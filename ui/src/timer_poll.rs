use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimerUpdate {
    Start(String),
    Stop,
}

pub fn now_epoch_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub fn read_config(app_support: &Path) -> (String, String, bool) {
    let path = app_support.join("pill-config.json");
    let text = fs::read_to_string(path).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let theme = value
        .get("theme")
        .and_then(|v| v.as_str())
        .unwrap_or("dark")
        .to_string();
    let anchor = value
        .get("anchor")
        .and_then(|v| v.as_str())
        .unwrap_or("bottom-center")
        .to_string();
    let quiet = value
        .get("quietTimer")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    (theme, anchor, quiet)
}

pub fn epoch_from_rfc3339(text: &str) -> Option<f64> {
    let digits: Vec<i64> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<i64>().ok())
        .collect();
    if digits.len() < 6 {
        return None;
    }
    let year = digits[0];
    let month = digits[1] as u64;
    let day = digits[2] as u64;
    let adjusted = if month <= 2 { year - 1 } else { year };
    let era = if adjusted >= 0 { adjusted } else { adjusted - 399 } / 400;
    let yoe = adjusted - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let seconds = digits[3] * 3600 + digits[4] * 60 + digits[5];
    Some((days * 86400 + seconds) as f64)
}

pub fn pass_seconds(app_support: &Path, now_epoch: f64) -> Option<u64> {
    let path = app_support.join("rules.pass-state.json");
    let text = fs::read_to_string(path).unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let passes = value.get("passes")?.as_object()?;
    let mut best: f64 = 0.0;
    for pass in passes.values() {
        if let Some(earned) = pass.get("earned_at").and_then(|v| v.as_str()) {
            if let Some(started) = epoch_from_rfc3339(earned) {
                best = best.max(60.0 - (now_epoch - started));
            }
        }
    }
    if best > 0.0 {
        Some(best as u64)
    } else {
        None
    }
}

pub fn usage_seconds(app_support: &Path, now_epoch: f64) -> Option<u64> {
    let path = app_support.join("pill-events.json");
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let secs = value.get("usage_timer_seconds")?.as_u64()?;
    if let Some(at_str) = value.get("at").and_then(|v| v.as_str()) {
        if let Some(at_epoch) = epoch_from_rfc3339(at_str) {
            if now_epoch - at_epoch > 15.0 {
                return None;
            }
        }
    }
    Some(secs)
}

pub fn poll_active_seconds(app_support: &Path, now_epoch: f64) -> Option<u64> {
    let pass = pass_seconds(app_support, now_epoch);
    let usage = usage_seconds(app_support, now_epoch);
    match (pass, usage) {
        (Some(p), Some(u)) => Some(p.min(u)),
        (Some(p), None) => Some(p),
        (None, Some(u)) => Some(u),
        (None, None) => None,
    }
}

pub fn format_mm_ss(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

pub fn poll_step(
    quiet_timer: bool,
    was_active: &mut bool,
    app_support: &Path,
    now_epoch: f64,
) -> Option<TimerUpdate> {
    if !quiet_timer {
        if *was_active {
            *was_active = false;
            return Some(TimerUpdate::Stop);
        }
        return None;
    }
    match poll_active_seconds(app_support, now_epoch) {
        Some(secs) => {
            *was_active = true;
            Some(TimerUpdate::Start(format_mm_ss(secs)))
        }
        None if *was_active => {
            *was_active = false;
            Some(TimerUpdate::Stop)
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poll_step_tracks_usage_and_emits_stop() {
        let dir = std::env::temp_dir().join(format!("leko-timer-poll-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let (_, _, default_quiet) = read_config(&dir);
        assert!(!default_quiet);

        fs::write(dir.join("pill-config.json"), r#"{"quietTimer":true}"#).unwrap();
        let (_, _, quiet) = read_config(&dir);
        assert!(quiet);

        let now = epoch_from_rfc3339("2026-01-01T12:00:05Z").unwrap();
        fs::write(
            dir.join("pill-events.json"),
            r#"{"version":1,"event":"tick","usage_timer_seconds":252,"at":"2026-01-01T12:00:00Z"}"#,
        )
        .unwrap();

        let mut active = false;
        assert_eq!(
            poll_step(quiet, &mut active, &dir, now),
            Some(TimerUpdate::Start("04:12".to_string()))
        );
        assert!(active);

        fs::write(
            dir.join("pill-events.json"),
            r#"{"version":1,"event":"tick","usage_timer_seconds":null,"at":"2026-01-01T12:00:06Z"}"#,
        )
        .unwrap();
        assert_eq!(
            poll_step(quiet, &mut active, &dir, now + 1.0),
            Some(TimerUpdate::Stop)
        );
        assert!(!active);
        assert_eq!(poll_step(quiet, &mut active, &dir, now + 2.0), None);

        fs::remove_dir_all(dir).unwrap();
    }
}
