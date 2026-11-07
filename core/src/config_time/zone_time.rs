//! Local dates and minutes in one time zone, as java.time reads them: a local minute that does not
//! exist (the skipped hour of spring) moves forward by the length of the gap, and a local minute
//! that happens twice (the repeated hour of autumn) is its first pass, the one at the earlier
//! offset. Offsets come from the time zone database, so no zone rule is written here. Instants are
//! milliseconds since the epoch. Ported from `lib/zone-time.js`.

use chrono::{Datelike, Duration, NaiveDate, Offset, TimeZone};
use chrono_tz::Tz;

const SECOND_MS: i64 = 1000;
const MINUTE_MS: i64 = 60_000;
const DAY_MS: i64 = 86_400_000;
/// The weekday codes the settings files use, Monday first.
const WEEKDAY_CODES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
pub const MINUTES_PER_DAY: i64 = 1440;

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// The date written YYYY-MM-DD, or `None` when the text is not a real date.
    pub fn parse_iso(text: &str) -> Option<Date> {
        let mut parts = text.split('-');
        let (year, month, day) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
        if parts.next().is_some() {
            return None;
        }
        NaiveDate::from_ymd_opt(year, month, day).map(Date::from_naive)
    }

    /// The weekday as the settings files write it, "mon" to "sun".
    pub fn weekday(&self) -> &'static str {
        self.naive().map_or(WEEKDAY_CODES[3], |date| WEEKDAY_CODES[date.weekday().num_days_from_monday() as usize])
    }

    /// The ISO text of the date.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    fn from_naive(date: NaiveDate) -> Date {
        Date { year: date.year(), month: date.month(), day: date.day() }
    }

    fn naive(&self) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(self.year, self.month, self.day)
    }
}

/// One time zone of the database.
#[derive(Debug, Clone)]
pub struct ZoneTime {
    zone: Tz,
}

impl ZoneTime {
    /// The zone called `name`, or a sentence saying this computer does not know it.
    pub fn new(name: &str) -> Result<ZoneTime, String> {
        name.parse::<Tz>().map(|zone| ZoneTime { zone }).map_err(|_| format!("the time zone \"{name}\" is not one this computer knows"))
    }

    /// The zone's offset from UTC at instant `ms`, in milliseconds.
    pub fn offset_at(&self, ms: i64) -> i64 {
        let seconds = self.zone.timestamp_millis_opt(ms).single().map_or(0, |at| i64::from(at.offset().fix().local_minus_utc()));
        seconds * SECOND_MS
    }

    /// The calendar date at instant `ms`.
    pub fn date_at(&self, ms: i64) -> Date {
        let local = self.zone.timestamp_millis_opt(ms).single().map(|at| at.date_naive());
        local.map_or(Date { year: 1970, month: 1, day: 1 }, Date::from_naive)
    }

    /// The weekday at instant `ms`, "mon" to "sun".
    pub fn weekday_at(&self, ms: i64) -> &'static str {
        self.date_at(ms).weekday()
    }

    /// Minutes after local midnight at instant `ms`. In the repeated hour of autumn the same minutes
    /// are seen twice, as a wall clock shows them.
    pub fn minute_of_day_at(&self, ms: i64) -> i64 {
        ms.saturating_add(self.offset_at(ms)).div_euclid(MINUTE_MS).rem_euclid(MINUTES_PER_DAY)
    }

    /// Instant `ms` on this zone's wall clock as `YYYY-MM-DDTHH:MM:SS` and the offset in force then
    /// (`-05:00`, `+05:30`, `Z` for none); the part of a second is cut off. An instant the zone has no
    /// rule for is read as the zone code reads it everywhere else, 1970-01-01 at offset 0.
    pub fn local_text(&self, ms: i64) -> String {
        if self.zone.timestamp_millis_opt(ms).single().is_none() {
            return "1970-01-01T00:00:00Z".to_string();
        }
        let offset = self.offset_at(ms).div_euclid(SECOND_MS);
        let (minute, second) = (self.minute_of_day_at(ms), ms.saturating_add(offset * SECOND_MS).div_euclid(SECOND_MS).rem_euclid(60));
        let zone_text = if offset == 0 { "Z".to_string() } else { format!("{}{:02}:{:02}", if offset < 0 { '-' } else { '+' }, offset.abs() / 3600, offset.abs() % 3600 / 60) };
        format!("{}T{:02}:{:02}:{:02}{}", self.date_at(ms).iso(), minute / 60, minute % 60, second, zone_text)
    }

    /// The date `days` after `date`.
    pub fn plus_days(&self, date: Date, days: i64) -> Date {
        date.naive().and_then(|naive| naive.checked_add_signed(Duration::days(days))).map_or(date, Date::from_naive)
    }

    /// The instant of local `minute` after midnight on `date`; 1440 or more is the start of the next day.
    pub fn instant_of(&self, date: Date, minute: i64) -> i64 {
        let (day, local) = if minute >= MINUTES_PER_DAY { (self.plus_days(date, 1), 0) } else { (date, minute) };
        let naive = day.naive().and_then(|day| day.and_hms_opt((local / 60) as u32, (local % 60) as u32, 0)).map_or(0, |at| at.and_utc().timestamp_millis());
        let before = self.offset_at(naive - DAY_MS);
        let after = self.offset_at(naive + DAY_MS);
        let mut offsets = vec![before];
        if after != before {
            offsets.push(after);
        }
        let fits = offsets.iter().map(|offset| naive - offset).filter(|instant| self.offset_at(*instant) == naive - instant);
        fits.min().unwrap_or(naive - before)
    }
}
