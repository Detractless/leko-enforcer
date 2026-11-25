use chrono::{DateTime, Datelike, Duration, LocalResult, TimeZone, Utc};

use crate::clock::Moment;
use crate::models::{Pass, PassCap, PassLength};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassState {
    pub earned_at: Option<String>,
    pub used_in_window: u32,
    pub window_key: String,
}

fn expiry(earned: DateTime<Utc>, length: &PassLength, moment: &Moment) -> Option<DateTime<Utc>> {
    match length {
        PassLength::TimedPass { minutes } => Some(earned + Duration::minutes(*minutes as i64)),
        PassLength::RestOfDayPass => {
            let local = earned.with_timezone(&moment.zone);
            let next_midnight = (local.date_naive() + Duration::days(1)).and_hms_opt(0, 0, 0)?;
            match moment.zone.from_local_datetime(&next_midnight) {
                LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
                LocalResult::Ambiguous(earliest, _) => Some(earliest.with_timezone(&Utc)),
                LocalResult::None => None,
            }
        }
    }
}

pub fn pass_active(pass: &Pass, state: &PassState, moment: &Moment) -> bool {
    let Pass::EarnPass { length, cap, .. } = pass else {
        return false;
    };
    let Some(earned_text) = &state.earned_at else {
        return false;
    };
    let Ok(earned) = Moment::parse(earned_text, moment.zone) else {
        return false;
    };
    let Some(end) = expiry(earned.utc, length, moment) else {
        return false;
    };
    if moment.utc < earned.utc || moment.utc >= end {
        return false;
    }
    match cap {
        PassCap::UnlimitedPasses => true,
        PassCap::LimitedPasses { max_per_window } => {
            let local = moment.utc.with_timezone(&moment.zone);
            let key = format!("{:04}-{:02}-{:02}", local.year(), local.month(), local.day());
            let used = if state.window_key == key {
                state.used_in_window
            } else {
                0
            };
            used < *max_per_window
        }
    }
}
