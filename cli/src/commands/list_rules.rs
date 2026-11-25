use std::io::Write;

use crate::arguments::CliArgs;
use crate::commands::{emit_json, emit_text, load, refused, CommandResult};
use crate::services::{output_human, output_json};

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let store = load(args)?;
    let mut rules = store.rules.clone();
    rules.sort_by(|left, right| left.id.cmp(&right.id));

    if args.flags.contains_key("json") {
        return emit_json(output_json::rules(&rules), out);
    }
    if rules.is_empty() {
        return emit_text("No rules.", out);
    }
    writeln!(out, "Rules: {}", rules.len()).map_err(|error| refused(error.to_string(), 1))?;
    for rule in &rules {
        writeln!(
            out,
            "- {}: {} | Target: {} | Seal: {}",
            rule.id,
            rule.name,
            output_human::target_name(&rule.target),
            output_human::seal_name(&rule.seal)
        )
        .map_err(|error| refused(error.to_string(), 1))?;
    }
    Ok(())
}
