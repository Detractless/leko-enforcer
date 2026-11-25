use chrono_tz::Tz;
use leko_core::clock::Moment;
use leko_core::Clock;
use leko_core::io::SystemClock;

use crate::arguments::CliArgs;

pub struct CliClock {
    pub moment: Moment,
}

impl CliClock {
    pub fn from_args(args: &CliArgs) -> Result<Self, String> {
        let zone_name = args.flags.get("zone").map(String::as_str).unwrap_or("UTC");
        let zone: Tz = zone_name
            .parse()
            .map_err(|_| format!("The timezone {} is not known.", zone_name))?;
        let moment = match args.flags.get("at") {
            Some(value) => Moment::parse(value, zone)
                .map_err(|error| format!("The timestamp {} is not valid: {}", value, error))?,
            None => SystemClock::new(zone).moment(),
        };
        Ok(CliClock { moment })
    }

    pub fn now_ms(&self) -> i64 {
        self.moment.utc.timestamp_millis()
    }

    pub fn timestamp(&self) -> String {
        self.moment.utc.to_rfc3339()
    }
}
