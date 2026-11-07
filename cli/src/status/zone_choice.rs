//! Chooses the time zone a status answer is judged in: the one named with `--zone`, otherwise the
//! computer's zone handed in by `main.rs`. UTC is never assumed: a zone that is unknown or missing
//! is a sentence asking for `--zone`, because a wrong zone changes every outside-hours answer. The
//! computer's zone is an argument; nothing is read from the environment here.

use crate::session::session_report::shown;
use leko_core::config_time::zone_time::ZoneTime;

const ASK_FOR_ZONE: &str = "Name the zone with --zone, for example --zone America/New_York.";

/// The zone name to print and the zone itself, or the sentence saying why there is none.
pub fn choose_zone(flag: Option<&str>, computer: Option<&str>) -> Result<(String, ZoneTime), String> {
    match (flag, computer) {
        (Some(name), _) => ZoneTime::new(name).map(|zone| (name.to_string(), zone)),
        (None, Some(name)) => ZoneTime::new(name)
            .map(|zone| (name.to_string(), zone))
            .map_err(|_| format!("This computer's time zone {} is not one that can be used, so nothing was judged. {ASK_FOR_ZONE}", shown(name))),
        (None, None) => Err(format!("This computer's time zone could not be found, so nothing was judged. {ASK_FOR_ZONE}")),
    }
}
