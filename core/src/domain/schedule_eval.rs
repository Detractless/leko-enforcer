use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::clock::Moment;
use crate::models::{TimeBasedSchedule, Window};

fn resolve_local(naive: NaiveDateTime, zone: Tz) -> Option<DateTime<Utc>> {
    for offset in 0..120 {
        let candidate = naive + Duration::minutes(offset);
        match zone.from_local_datetime(&candidate) {
            LocalResult::Single(value) => return Some(value.with_timezone(&Utc)),
            LocalResult::Ambiguous(earliest, _) => return Some(earliest.with_timezone(&Utc)),
            LocalResult::None => continue,
        }
    }
    None
}

fn window_instants(
    date: NaiveDate,
    window: &Window,
    zone: Tz,
) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    let start = resolve_local(midnight + Duration::minutes(window.start_minute as i64), zone)?;
    let end = resolve_local(midnight + Duration::minutes(window.end_minute as i64), zone)?;
    Some((start, end))
}

pub fn routine_windows(
    days: &[u32],
    windows: &[Window],
    moment: &Moment,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let today = moment.utc.with_timezone(&moment.zone).date_naive();
    let mut found = Vec::new();
    for offset in -1i64..=0 {
        let date = today + Duration::days(offset);
        let weekday = date.weekday().num_days_from_monday() + 1;
        if !days.contains(&weekday) {
            continue;
        }
        for window in windows {
            if let Some(pair) = window_instants(date, window, moment.zone) {
                found.push(pair);
            }
        }
    }
    found
}

pub fn is_time_active(schedule: &TimeBasedSchedule, moment: &Moment) -> bool {
    match schedule {
        TimeBasedSchedule::AlwaysOnSchedule => true,
        TimeBasedSchedule::OneTimeSchedule { start_at, end_at } => {
            let start = Moment::parse(start_at, moment.zone).map(|value| value.utc);
            let end = Moment::parse(end_at, moment.zone).map(|value| value.utc);
            match (start, end) {
                (Ok(start), Ok(end)) => moment.utc >= start && moment.utc < end,
                _ => false,
            }
        }
        TimeBasedSchedule::RoutineSchedule { days, windows } => {
            routine_windows(days, windows, moment)
                .iter()
                .any(|(start, end)| moment.utc >= *start && moment.utc < *end)
        }
    }
}
