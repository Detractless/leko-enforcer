use std::collections::HashMap;

pub const EXIT_OK: i32 = 0;
pub const EXIT_REFUSED: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

pub const RULE_COMMANDS: &[&str] = &[
    "create-rule", "list-rules", "show-rule", "edit-rule", "delete-rule",
    "queue-edit", "cancel-edit", "request-pass", "check-status", "evaluate-rule",
];

pub const INFRA_COMMANDS: &[&str] = &["settings", "agent", "enforce"];

pub const ADDON_COMMANDS: &[&str] = &["pin-addon", "list-addons", "verify-addon"];

pub const LEGACY_COMMANDS: &[&str] = &[
    "add-block", "session", "switch-off", "switch-on", "status", "edit-block",
    "edit-schedule", "delay", "settle", "cancel", "waiting",
];

pub const LEGACY_FLAGS: &[&str] = &[
    "--open-hours", "--wall", "--write-zip", "--blocked-hours",
];

const BOOLEAN_FLAGS: &[&str] = &[
    "--json", "--write", "--queue", "--cancel", "--loop", "--once", "--dry-run",
    "--proof-typing", "--proof-task", "--proof-password", "--proof-verifier",
    "--engagement-pass", "--rest-of-day",
];

#[derive(Clone, Debug)]
pub struct CliArgs {
    pub command: String,
    pub subcommand: Option<String>,
    pub flags: HashMap<String, String>,
    pub positional: Vec<String>,
}

pub fn usage() -> String {
    let mut commands: Vec<&str> = RULE_COMMANDS.to_vec();
    commands.extend_from_slice(INFRA_COMMANDS);
    commands.extend_from_slice(ADDON_COMMANDS);
    format!("Usage: leko <{}> [flags]", commands.join("|"))
}

pub fn parse(args: &[String]) -> Result<CliArgs, String> {
    let mut tokens = args.iter();
    let command = tokens
        .next()
        .ok_or_else(|| usage())?
        .clone();
    if LEGACY_COMMANDS.contains(&command.as_str()) {
        return Err(format!("The legacy command {} was removed. {}", command, usage()));
    }
    if !RULE_COMMANDS.contains(&command.as_str())
        && !INFRA_COMMANDS.contains(&command.as_str())
        && !ADDON_COMMANDS.contains(&command.as_str())
    {
        return Err(format!("Unknown command {}. {}", command, usage()));
    }

    let mut subcommand = None;
    let mut flags = HashMap::new();
    let mut positional = Vec::new();
    let mut rest = tokens.peekable();
    if command == "agent" {
        if let Some(value) = rest.peek() {
            if !value.starts_with('-') {
                subcommand = Some((*value).clone());
                rest.next();
            }
        }
    }

    while let Some(token) = rest.next() {
        if token.starts_with("--") {
            if LEGACY_FLAGS.contains(&token.as_str()) {
                return Err(format!("The legacy flag {} was removed. {}", token, usage()));
            }
            let name = token.trim_start_matches('-').to_string();
            if BOOLEAN_FLAGS.contains(&token.as_str()) {
                if flags.insert(name, "true".to_string()).is_some() {
                    return Err(format!("The flag {} was repeated.", token));
                }
                continue;
            }
            let value = rest
                .next()
                .ok_or_else(|| format!("The flag {} needs a value.", token))?;
            if value.starts_with('-') {
                return Err(format!("The flag {} needs a value.", token));
            }
            if flags.insert(name, value.clone()).is_some() {
                return Err(format!("The flag {} was repeated.", token));
            }
        } else {
            positional.push(token.clone());
        }
    }

    Ok(CliArgs { command, subcommand, flags, positional })
}
