//! What the command says about a session it made: one sentence for a person, one JSON line for a
//! script, and the text of a refusal. The JSON is built from the core's own rule and seal values, so
//! it cannot differ from them. `shown` writes an odd or long argument safely for a terminal.

use leko_core::sessions::timed_session::TimedSession;
use serde_json::json;

const SHOWN_CHARACTERS: usize = 60;

/// `text` cut to 60 characters, in quotes, with control characters written out, so a long or odd
/// argument cannot flood or steer the terminal.
pub fn shown(text: &str) -> String {
    let mut cut: String = text.chars().take(SHOWN_CHARACTERS).collect();
    if text.chars().nth(SHOWN_CHARACTERS).is_some() {
        cut.push_str("...");
    }
    format!("{cut:?}")
}

/// The sentence for a session of `minutes` that ends as `session` says; ends with a newline.
pub fn sentence(minutes: i64, session: &TimedSession) -> String {
    let unit = if minutes == 1 { "minute" } else { "minutes" };
    format!("Made a session of {minutes} {unit} that ends at {} (UTC). Nothing was saved or changed; add --json to see the rule and the seal.\n", session.ends_at)
}

/// One line of JSON: the rule and the seal, as the core made them; ends with a newline.
pub fn json_line(session: &TimedSession) -> String {
    format!("{}\n", json!({"rule": session.rule, "lock": session.lock}))
}

/// The core's sentence for a request it refuses, with the fact that nothing was made.
pub fn refusal(core_sentence: &str) -> String {
    format!("{core_sentence} Nothing was made.\n")
}

/// `word` written so that pasting it into a shell gives the same word: left alone when it holds only
/// safe characters, else in single quotes; a word with a control character is written out in escapes.
pub fn quoted(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|letter| letter.is_alphanumeric() || "_./:,@%+=-".contains(letter)) {
        return word.to_string();
    }
    if word.chars().any(char::is_control) {
        return format!("{word:?}");
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// `sentence` and the words that nothing was saved, as one line; a full stop is added when the
/// sentence ends without one.
pub fn refusal_line(sentence: &str) -> String {
    let stop = if sentence.ends_with(['.', '!', '?']) { "" } else { "." };
    format!("{sentence}{stop} Nothing was saved.\n")
}
