//! The site names of a block's coverage. A site name is lowercase ASCII (other scripts are written
//! as punycode): labels of letters, digits and "-" up to 63 characters, no "-" at either end, joined
//! by dots; at least two labels, the last not all digits (so an IP number is not a site); up to 253
//! characters in all. A list holds 1 to 1000 names.

use crate::config::strict_fields::{Read, StrictFields};

pub const MAX_SITES: i64 = 1000;
pub const MAX_SITE_LENGTH: i64 = 253;
const MIN_SITE_LENGTH: i64 = 3;
const MAX_LABEL: usize = 63;

fn is_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    let allowed = |c: &u8| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-';
    !bytes.is_empty() && bytes.len() <= MAX_LABEL && bytes.iter().all(allowed) && bytes[0] != b'-' && bytes[bytes.len() - 1] != b'-'
}

/// True when `name` is a site name in the form a settings file holds.
pub fn is_site(name: &str) -> bool {
    if name.len() > MAX_SITE_LENGTH as usize {
        return false;
    }
    let labels: Vec<&str> = name.split('.').collect();
    let last_is_number = labels.last().is_some_and(|last| last.bytes().all(|c| c.is_ascii_digit()));
    labels.len() >= 2 && labels.iter().all(|label| is_label(label)) && !last_is_number
}

/// The `sites` list of a covers object: 1 to 1000 site names, the first bad one refuses the file.
pub fn read_sites(fields: &mut StrictFields) -> Read<Vec<String>> {
    let sites = fields.texts("sites", 1, MAX_SITES, MIN_SITE_LENGTH, MAX_SITE_LENGTH, true)?.unwrap_or_default();
    for (index, name) in sites.iter().enumerate() {
        if !is_site(name) {
            let reason = "must be a site name in lowercase, such as example.com: no scheme, path, port or wildcard, at least two labels, the last not all digits";
            return fields.fail(&format!("sites[{index}]"), reason);
        }
    }
    Ok(sites)
}
