//! Writes what `leko add-block` says: the plain draft of the block (what it is called, what it
//! covers, when it closes, how a person gets in, what is not checked yet, the phone's warning, and
//! that nothing was saved), and the sentence after a save. Every name that came from the user goes
//! through `shown`, so an odd or long one cannot flood or steer the terminal. Built only from the
//! values the core returned; ends with a newline.

use crate::session::session_report::shown;
use leko_core::blocks::block_request::{BlockRequest, RuleChoice};
use leko_core::blocks::draft_block::{Draft, DEFAULT_WAIT_SECONDS};
use leko_core::models::specs::DeviceAction;

fn list(items: &[String]) -> String {
    items.iter().map(|item| shown(item)).collect::<Vec<_>>().join(", ")
}

fn covers_in_words(request: &BlockRequest) -> String {
    if let Some(action) = request.device {
        return match action {
            DeviceAction::Lock => "the whole computer: the screen is locked".to_string(),
            DeviceAction::Logoff => "the whole computer: the user is logged out".to_string(),
            DeviceAction::Shutdown => "the whole computer: it is shut down".to_string(),
        };
    }
    let mut parts = Vec::new();
    if !request.apps.is_empty() {
        parts.push(format!("{} {}", if request.apps.len() == 1 { "the app" } else { "the apps" }, list(&request.apps)));
    }
    if !request.sites.is_empty() {
        parts.push(format!("{} {}", if request.sites.len() == 1 { "the site" } else { "the sites" }, list(&request.sites)));
    }
    parts.join(" and ")
}

/// "Every day" or "On mon, tue", and the closing sentence for days the rule leaves alone.
fn days_in_words(request: &BlockRequest) -> (String, &'static str) {
    if request.days.is_empty() {
        ("Every day".to_string(), "")
    } else {
        (format!("On {}", request.days.join(", ")), " On the other days this block does not close anything.")
    }
}

fn rule_in_words(request: &BlockRequest, draft: &Draft) -> String {
    let (when, other_days) = days_in_words(request);
    match &request.rule {
        RuleChoice::Always => format!("{when} it is closed all day.{other_days}"),
        RuleChoice::OpenHours(stretches) => {
            let open: Vec<String> = stretches.iter().map(|(from, to)| format!("{from}-{to}")).collect();
            format!("{when} it is open only during {} and closed at all other times.{other_days}", open.join(", "))
        }
        RuleChoice::ForMinutes(minutes) => {
            let unit = if *minutes == 1 { "minute" } else { "minutes" };
            format!("It is closed for {minutes} {unit}, until {} (UTC).", draft.ends_at.as_deref().unwrap_or("the end"))
        }
    }
}

fn door_in_words(draft: &Draft) -> String {
    let (door, pass) = (&draft.block["door"], &draft.block["pass"]);
    match (door["seconds"].as_i64(), pass["minutes"].as_i64(), pass["perDay"].as_i64()) {
        (Some(seconds), Some(minutes), Some(per_day)) => {
            let standard = if seconds == DEFAULT_WAIT_SECONDS { " (the standard wait)" } else { "" };
            format!("To get in, a person waits {seconds} seconds{standard}, and then has a pass of {minutes} minutes, up to {per_day} times a day.")
        }
        (Some(seconds), _, _) => format!("To get in, a person waits {seconds} seconds."),
        _ => "There is no way in while it is closed.".to_string(),
    }
}

/// The plain draft of `draft`, made from `request`, and the command that saves it. `file` is the
/// settings file as typed; `phone_warning` is why the phone's Enforcer 0.3 would refuse the result.
pub fn draft_text(request: &BlockRequest, draft: &Draft, file: &str, phone_warning: Option<&str>, command: &str) -> String {
    let mut lines = vec![
        format!("Draft of a new block called {}, with the id {}, for {}.", shown(&request.name), shown(&draft.id), shown(file)),
        format!("It closes {}.", covers_in_words(request)),
        rule_in_words(request, draft),
        door_in_words(draft),
    ];
    if let Some(message) = &request.message {
        lines.push(format!("When someone tries to open it, it says {}.", shown(message)));
    }
    if request.device.is_none() {
        lines.push("Apps that are never blocked are not checked yet.".to_string());
    }
    if let Some(why) = phone_warning {
        lines.push(phone_warning_line(why));
    }
    lines.push(format!("Nothing was saved. To save it, run the same command with --write: {command} --write"));
    lines.join("\n") + "\n"
}

/// The warning line for the phone's reason `why` (the core's sentence, closed with a full stop).
fn phone_warning_line(why: &str) -> String {
    let stop = if why.ends_with(['.', '!', '?']) { "" } else { "." };
    format!("Warning: the phone's Enforcer 0.3 could not read this file, so it would not sync there. {why}{stop}")
}

/// The sentence after the block was saved into `file`.
pub fn saved_text(draft: &Draft, file: &str, phone_warning: Option<&str>) -> String {
    let id = draft.block["id"].as_str().unwrap_or(&draft.id);
    let mut text = format!("Saved the block {} as blocks/{id}.json in {}; every other file in it is as it was.\n", shown(&draft.id), shown(file));
    if let Some(why) = phone_warning {
        text.push_str(&phone_warning_line(why));
        text.push('\n');
    }
    text
}
