pub mod check_status;
pub mod create_rule;
pub mod edit_rule;
pub mod evaluate_rule;
pub mod list_rules;
pub mod request_pass;
pub mod show_rule;

use std::io::Write;
use std::path::PathBuf;

use leko_core::store::NativeStore;
use leko_core::TechnologyManagementRule;
use serde_json::Value;

use crate::arguments::CliArgs;
use crate::services::infrastructure;

pub type CommandResult = Result<(), (String, i32)>;

pub fn refused(message: impl Into<String>, code: i32) -> (String, i32) {
    (message.into(), code)
}

pub fn store_path(args: &CliArgs) -> PathBuf {
    args.flags
        .get("store")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("leko-rules.json"))
}

pub fn load(args: &CliArgs) -> Result<NativeStore, (String, i32)> {
    leko_core::store::load(&store_path(args)).map_err(|error| refused(error.to_string(), 1))
}

pub fn save(args: &CliArgs, store: &NativeStore) -> CommandResult {
    leko_core::store::save(&store_path(args), store).map_err(|error| refused(error.to_string(), 1))
}

pub fn find_rule<'a>(
    store: &'a NativeStore,
    id: &str,
) -> Result<&'a TechnologyManagementRule, (String, i32)> {
    store
        .rules
        .iter()
        .find(|rule| rule.id == id)
        .ok_or_else(|| refused(format!("No rule has the id {}.", id), 1))
}

pub fn emit_json(value: Value, out: &mut dyn Write) -> CommandResult {
    writeln!(out, "{}", value).map_err(|error| refused(error.to_string(), 1))
}

pub fn emit_text(text: &str, out: &mut dyn Write) -> CommandResult {
    writeln!(out, "{}", text).map_err(|error| refused(error.to_string(), 1))
}

pub fn dispatch(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    match args.command.as_str() {
        "create-rule" => create_rule::run(args, out),
        "list-rules" => list_rules::run(args, out),
        "show-rule" => show_rule::run(args, out),
        "edit-rule" | "delete-rule" | "queue-edit" | "cancel-edit" => edit_rule::run(args, out),
        "request-pass" => request_pass::run(args, out),
        "check-status" => check_status::run(args, out),
        "evaluate-rule" => evaluate_rule::run(args, out),
        "settings" | "agent" | "enforce" => infrastructure::run(args, out),
        "pin-addon" | "list-addons" | "verify-addon" => crate::services::addon::run(args, out),
        _ => Err(refused("That command is not available.", 2)),
    }
}
