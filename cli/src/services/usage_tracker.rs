use std::path::{Path, PathBuf};

use leko_core::clock::Moment;
use leko_core::domain::{is_time_active, pass_active, period_key, UsageState};
use leko_core::models::{Pass, PassRequirement, Period, Target, UsageLimit};
use leko_core::services::{
    advance_rule_usage, budget_period_key, domain_state, domain_usage,
    load_usage_state, pass_state_path, record_pass, reset_engagement, save_pass_state,
    save_usage_state, usage_state_path, PassStateFile, UsageStateFile,
};
use leko_core::TechnologyManagementRule;

use crate::arguments::CliArgs;
use crate::commands::refused;
use crate::services::os_executor::ForegroundActivity;

pub fn usage_state_path_for(args: &CliArgs, store: &Path) -> PathBuf {
    args.flags
        .get("usage-state")
        .map(PathBuf::from)
        .unwrap_or_else(|| usage_state_path(store))
}

pub fn pass_state_path_for(args: &CliArgs, store: &Path) -> PathBuf {
    args.flags
        .get("pass-state")
        .map(PathBuf::from)
        .unwrap_or_else(|| pass_state_path(store))
}

fn site_matches(host: &str, patterns: &[String]) -> bool {
    let host = host.trim().to_ascii_lowercase();
    let stripped = host.strip_prefix("www.").unwrap_or(&host);
    patterns.iter().any(|raw| {
        let pat = raw.trim().to_ascii_lowercase();
        let pat_stripped = pat.strip_prefix("www.").unwrap_or(&pat);
        host == pat
            || stripped == pat_stripped
            || host.ends_with(&format!(".{}", pat_stripped))
    })
}

fn bundle_matches(bundle: &str, list: &[String]) -> bool {
    list.iter().any(|item| item.eq_ignore_ascii_case(bundle))
}

pub fn target_matches(target: &Target, activity: Option<&ForegroundActivity>) -> bool {
    let Some(act) = activity else {
        return false;
    };
    match target {
        Target::SpecificApps { apps } => act
            .bundle_id
            .as_deref()
            .map(|b| bundle_matches(b, apps))
            .unwrap_or(false),
        Target::AllowOnlyApps { allowed_apps } => act
            .bundle_id
            .as_deref()
            .map(|b| !bundle_matches(b, allowed_apps))
            .unwrap_or(false),
        Target::SpecificWebsites { sites } => act
            .site
            .as_deref()
            .map(|s| site_matches(s, sites))
            .unwrap_or(false),
        Target::AllowOnlyWebsites { allowed_sites, .. } => act
            .site
            .as_deref()
            .map(|s| !site_matches(s, allowed_sites))
            .unwrap_or(false),
        Target::TopicDetectedWebsites { .. } => act.site.is_some(),
        Target::WholeDevice { .. } => false,
    }
}

pub fn engagement_matches(pass: &Pass, activity: Option<&ForegroundActivity>) -> bool {
    let Some(act) = activity else {
        return false;
    };
    let Pass::EarnPass {
        requirement: PassRequirement::EngagementPass { targets, .. },
        ..
    } = pass
    else {
        return false;
    };
    let app_hit = act
        .bundle_id
        .as_deref()
        .map(|b| bundle_matches(b, targets))
        .unwrap_or(false);
    let site_hit = act
        .site
        .as_deref()
        .map(|s| site_matches(s, targets))
        .unwrap_or(false);
    app_hit || site_hit
}

pub fn resolve_status_usage(
    args: &CliArgs,
    store: &Path,
    rule: &TechnologyManagementRule,
    moment: &Moment,
) -> Result<(UsageState, u64, bool), (String, i32)> {
    let path = usage_state_path_for(args, store);
    let file = match load_usage_state(&path) {
        Ok(loaded) => loaded,
        Err(_) if args.flags.get("usage-state").is_none() => UsageStateFile::default(),
        Err(error) => return Err(refused(error.to_string(), 1)),
    };
    let (mut usage, mut active_seconds, mut session_open) = domain_usage(&file, &rule.id);

    let parse_flag = |name: &str| -> Result<Option<u32>, (String, i32)> {
        match args.flags.get(name) {
            None => Ok(None),
            Some(v) => v
                .parse::<u32>()
                .map(Some)
                .map_err(|_| refused(format!("The flag --{} needs a whole number.", name), 2)),
        }
    };
    let used_override = parse_flag("used-minutes")?;
    let sessions_override = parse_flag("sessions")?;
    if let Some(m) = used_override {
        usage.used_minutes = m;
        active_seconds = (m as u64).saturating_mul(60);
    }
    if let Some(s) = sessions_override {
        usage.sessions = s;
        session_open = false;
    }
    if let Some(key) = args.flags.get("usage-period-key") {
        usage.period_key = key.clone();
    } else if (used_override.is_some() || sessions_override.is_some()) && usage.period_key.is_empty()
    {
        usage.period_key = budget_period_key(rule, moment);
    }
    Ok((usage, active_seconds, session_open))
}

pub fn update_tick(
    usage_file: &mut UsageStateFile,
    pass_file: &mut PassStateFile,
    pass_path: &Path,
    rules: &[TechnologyManagementRule],
    moment: &Moment,
    activity: Option<&ForegroundActivity>,
    debounce_seconds: u64,
    max_gap_seconds: u64,
) -> bool {
    let mut pass_changed = false;
    let window_key = period_key(Period::Daily, moment);
    for rule in rules {
        let p_state = domain_state(pass_file, &rule.id);
        let p_active = pass_active(&rule.pass, &p_state, moment);
        let t_active = target_matches(&rule.target, activity);
        let e_active = engagement_matches(&rule.pass, activity);
        let outcome = advance_rule_usage(
            usage_file,
            rule,
            moment,
            t_active,
            e_active,
            p_active,
            debounce_seconds,
            max_gap_seconds,
        );
        if outcome.grant_engagement_pass {
            if record_pass(pass_file, rule, moment, window_key.clone()).is_ok() {
                reset_engagement(usage_file, &rule.id);
                pass_changed = true;
            }
        }
    }
    if pass_changed {
        let _ = save_pass_state(pass_path, pass_file);
    }
    pass_changed
}

pub fn save_usage_safe(path: &Path, usage: &UsageStateFile) -> Result<(), String> {
    save_usage_state(path, usage).map_err(|error| error.to_string())
}

pub fn active_time_limit_remaining(
    rules: &[TechnologyManagementRule],
    usage_file: &UsageStateFile,
    pass_file: &PassStateFile,
    activity: Option<&ForegroundActivity>,
    moment: &Moment,
) -> Option<u64> {
    let mut best: Option<u64> = None;
    for rule in rules {
        if !rule.enabled || !is_time_active(&rule.schedule.time_based, moment) {
            continue;
        }
        let Some(UsageLimit::TimeLimit { minutes, period }) = &rule.schedule.usage_based else {
            continue;
        };
        let p_state = domain_state(pass_file, &rule.id);
        if pass_active(&rule.pass, &p_state, moment) {
            continue;
        }
        if !target_matches(&rule.target, activity) {
            continue;
        }
        let (u_state, active_seconds, _) = domain_usage(usage_file, &rule.id);
        let key = period_key(*period, moment);
        let used = if u_state.period_key == key {
            active_seconds
        } else {
            0
        };
        let total = (*minutes as u64).saturating_mul(60);
        if used < total {
            let remaining = total - used;
            best = Some(best.map(|b| b.min(remaining)).unwrap_or(remaining));
        }
    }
    best
}

pub fn write_pill_state(
    args: &CliArgs,
    blocked: Option<(&str, &str)>,
    timer_seconds: Option<u64>,
    at: &str,
) {
    let Some(path) = args.flags.get("pill-events") else {
        return;
    };
    let value = if let Some((kind, target)) = blocked {
        serde_json::json!({
            "version": 1,
            "event": "blocked",
            "kind": kind,
            "target": target,
            "usage_timer_seconds": serde_json::Value::Null,
            "at": at,
        })
    } else {
        serde_json::json!({
            "version": 1,
            "event": "tick",
            "usage_timer_seconds": timer_seconds,
            "at": at,
        })
    };
    let Ok(bytes) = serde_json::to_vec(&value) else {
        return;
    };
    let path = PathBuf::from(path);
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, bytes).is_ok() {
        let _ = std::fs::rename(temp, path);
    }
}

pub fn normalize_domain(raw: &str) -> Option<String> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(&trimmed);
    let host_port = without_scheme.split('/').next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("").trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

pub fn parse_site_from_heartbeats(bytes: &[u8], bundle_id: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let entries = value.as_object()?;
    for (key, entry) in entries {
        let key_matches = key.contains(bundle_id)
            || entry
                .get("bundle_id")
                .and_then(|v| v.as_str())
                .map(|b| b.eq_ignore_ascii_case(bundle_id))
                .unwrap_or(false);
        if !key_matches {
            continue;
        }
        for field in ["active_site", "site", "domain", "url"] {
            if let Some(raw) = entry.get(field).and_then(|v| v.as_str()) {
                if let Some(domain) = normalize_domain(raw) {
                    return Some(domain);
                }
            }
        }
    }
    None
}

pub fn parse_site_from_eval_log(text: &str) -> Option<String> {
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        for field in ["domain", "active_site", "site", "url"] {
            if let Some(raw) = entry.get(field).and_then(|v| v.as_str()) {
                if let Some(domain) = normalize_domain(raw) {
                    return Some(domain);
                }
            }
        }
    }
    None
}
