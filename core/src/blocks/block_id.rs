//! The id of a new block, made from its name by one fixed rule: ASCII letters and digits lowercased,
//! every run of anything else one dash, dashes cut from both ends. The same name always gives the
//! same id, so the draft can show the id before anything is saved. A name that is empty, too long,
//! holds a control character, or has no ASCII letter or digit is refused with a sentence.

use crate::config::strict_fields::utf16_len;

/// The longest name the readers accept, counted in UTF-16 units as the phone counts.
pub const MAX_NAME_UNITS: i64 = 60;

/// Nothing when `name` can be written into a block file, else the sentence saying why not.
pub fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("A block needs a name.".to_string());
    }
    if utf16_len(name) > MAX_NAME_UNITS {
        return Err(format!("A block's name is at most {MAX_NAME_UNITS} characters, not {}.", utf16_len(name)));
    }
    if name.chars().any(char::is_control) {
        return Err("A block's name cannot hold a control character.".to_string());
    }
    Ok(())
}

/// The id `name` makes, or the sentence saying why it makes none. The id is never longer than the
/// name, so it stays within the 64 characters an id may hold.
pub fn id_from_name(name: &str) -> Result<String, String> {
    check_name(name)?;
    let (mut id, mut gap) = (String::new(), false);
    for letter in name.chars() {
        if letter.is_ascii_alphanumeric() {
            if gap && !id.is_empty() {
                id.push('-');
            }
            gap = false;
            id.push(letter.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    if id.is_empty() {
        return Err("A block's name needs at least one letter from A to Z or one digit, to make an id from.".to_string());
    }
    Ok(id)
}
