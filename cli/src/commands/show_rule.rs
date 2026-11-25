use std::io::Write;

use crate::arguments::CliArgs;
use crate::commands::{emit_json, find_rule, load, refused, CommandResult};
use crate::services::{output_human, output_json};

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let store = load(args)?;
    let rule = find_rule(&store, id)?;
    if args.flags.contains_key("json") {
        emit_json(output_json::rule(rule), out)
    } else {
        output_human::rule(rule, out).map_err(|message| refused(message, 1))
    }
}
