use chrono::Utc;
use chrono_tz::Tz;

use crate::clock::{Clock, Moment};

pub struct SystemClock {
    pub zone: Tz,
}

impl SystemClock {
    pub fn new(zone: Tz) -> Self {
        SystemClock { zone }
    }
}

impl Clock for SystemClock {
    fn moment(&self) -> Moment {
        Moment {
            utc: Utc::now(),
            zone: self.zone,
        }
    }
}
