use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::error::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug)]
pub struct Moment {
    pub utc: DateTime<Utc>,
    pub zone: Tz,
}

impl Moment {
    pub fn parse(utc_text: &str, zone: Tz) -> CoreResult<Self> {
        let utc = DateTime::parse_from_rfc3339(utc_text)
            .map_err(|error| CoreError::Clock(error.to_string()))?
            .with_timezone(&Utc);
        Ok(Moment { utc, zone })
    }
}

pub trait Clock {
    fn moment(&self) -> Moment;
}

#[derive(Clone, Copy, Debug)]
pub struct FixedClock {
    pub moment: Moment,
}

impl FixedClock {
    pub fn new(moment: Moment) -> Self {
        FixedClock { moment }
    }
}

impl Clock for FixedClock {
    fn moment(&self) -> Moment {
        self.moment
    }
}
