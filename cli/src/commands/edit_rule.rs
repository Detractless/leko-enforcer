use std::io::Write;

use leko_core::domain::{allow_edit, ChangeDirection, EditDecision, ProposedEdit, SealProof};
use leko_core::models::{QueuedEdit, Seal};
use leko_core::TechnologyManagementRule;

use crate::arguments::CliArgs;
use crate::clock::CliClock;
use crate::commands::{emit_json, emit_text, load, refused, save, CommandResult};
use crate::services::output_json;

fn direction_of(args: &CliArgs) -> Result<ChangeDirection, (String, i32)> {
    let default = if args.command == "delete-rule" { "delete" } else { "weaken" };
    match args.flags.get("direction").map(String::as_str).unwrap_or(default) {
        "tighten" => Ok(ChangeDirection::Tighten),
        "weaken" => Ok(ChangeDirection::Weaken),
        "pause" => Ok(ChangeDirection::Pause),
        "delete" => Ok(ChangeDirection::Delete),
        other => Err(refused(format!("Unknown edit direction {}.", other), 2)),
    }
}

fn proof_of(args: &CliArgs) -> SealProof {
    SealProof {
        typing_ok: args.flags.contains_key("proof-typing"),
        task_ok: args.flags.contains_key("proof-task"),
        password_ok: args.flags.contains_key("proof-password"),
        verifier_satisfied: args.flags.contains_key("proof-verifier"),
        edit_window_ok: false,
    }
}

fn report(args: &CliArgs, message: &str, out: &mut dyn Write) -> CommandResult {
    if args.flags.contains_key("json") {
        emit_json(output_json::message(message), out)
    } else {
        emit_text(message, out)
    }
}

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    if !crate::services::addon::check_pinned(args).map_err(|message| refused(message, 1))? {
        return Err(refused("the custom verifier was not satisfied", 1));
    }
    let id = args
        .flags
        .get("id")
        .ok_or_else(|| refused("The flag --id needs a value.", 2))?;
    let direction = direction_of(args)?;
    let clock = CliClock::from_args(args).map_err(|error| refused(error, 1))?;
    let field = args.flags.get("field").cloned().unwrap_or_else(|| "rule".to_string());
    let value = args.flags.get("value").cloned().unwrap_or_else(|| "true".to_string());
    let proposal = ProposedEdit { field: field.clone(), direction };
    let mut store = load(args)?;
    let index = store
        .rules
        .iter()
        .position(|rule| rule.id == *id)
        .ok_or_else(|| refused(format!("No rule has the id {}.", id), 1))?;

    if args.command == "cancel-edit" {
        if let Seal::QueuedDelaySeal { pending_edits, .. } = &mut store.rules[index].seal {
            for edit in pending_edits.iter_mut() {
                edit.cancelled = true;
            }
        } else {
            return Err(refused("The rule has no queued delay seal.", 1));
        }
        save(args, &store)?;
        return report(args, "Queued edit cancelled.", out);
    }

    let decision = allow_edit(&store.rules[index], &proposal, &clock.moment, &proof_of(args));
    match decision {
        EditDecision::Blocked => Err(refused("The seal blocks this change.", 1)),
        EditDecision::Queued => {
            if let Seal::QueuedDelaySeal { pending_edits, .. } = &mut store.rules[index].seal {
                pending_edits.push(QueuedEdit {
                    key: field,
                    value,
                    requested_at: clock.timestamp(),
                    cancelled: false,
                });
            } else {
                return Err(refused("The rule has no queued delay seal.", 1));
            }
            save(args, &store)?;
            report(args, "Edit queued.", out)
        }
        EditDecision::Allowed => {
            match direction {
                ChangeDirection::Delete => {
                    store.rules.remove(index);
                    save(args, &store)?;
                    return report(args, "Rule deleted.", out);
                }
                ChangeDirection::Pause => {
                    store.rules[index].enabled = false;
                    store.rules[index].updated_at = clock.timestamp();
                    save(args, &store)?;
                    return report(args, "Rule paused.", out);
                }
                ChangeDirection::Tighten | ChangeDirection::Weaken => {
                    let mut object = serde_json::to_value(&store.rules[index])
                        .map_err(|error| refused(error.to_string(), 1))?;
                    let parsed = serde_json::from_str::<serde_json::Value>(&value)
                        .unwrap_or_else(|_| serde_json::Value::String(value.clone()));
                    object[&field] = parsed;
                    let updated: TechnologyManagementRule = serde_json::from_value(object)
                        .map_err(|error| refused(format!("Invalid edit: {}", error), 1))?;
                    updated.validate().map_err(|error| refused(error.to_string(), 1))?;
                    store.rules[index] = updated;
                    save(args, &store)?;
                    return report(args, "Rule edited.", out);
                }
            }
        }
    }
}
