use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::arguments::CliArgs;
use crate::services::output_human;

type CommandResult = Result<(), (String, i32)>;

fn refused(message: String) -> (String, i32) {
    (message, 1)
}

fn emit(value: serde_json::Value, text: &str, args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    if args.flags.contains_key("json") {
        writeln!(out, "{}", value).map_err(|e| refused(e.to_string()))?;
    } else {
        output_human::message(text, out).map_err(refused)?;
    }
    Ok(())
}

fn settings_path(args: &CliArgs) -> PathBuf {
    args.flags
        .get("settings")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("leko-settings.json"))
}

fn settings(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let path = settings_path(args);
    let zone = args.flags.get("zone").cloned().unwrap_or_else(|| "UTC".to_string());
    let value = json!({
        "store_path": args.flags.get("store").cloned().unwrap_or_else(|| "leko-rules.json".to_string()),
        "zone": zone,
        "shadow_mode": false,
        "nsfw": false,
        "nsfw_overridable": false,
    });
    if args.flags.contains_key("write") {
        let bytes = serde_json::to_vec_pretty(&value).map_err(|e| refused(e.to_string()))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| refused(e.to_string()))?;
            }
        }
        let temp = path.with_extension("tmp");
        fs::write(&temp, &bytes).map_err(|e| refused(e.to_string()))?;
        fs::rename(&temp, &path).map_err(|e| refused(e.to_string()))?;
    }
    emit(value, &format!("Settings saved at {}.", path.display()), args, out)
}

fn agent_home() -> Result<PathBuf, (String, i32)> {
    let home = std::env::var_os("HOME").ok_or_else(|| refused("The home folder is unavailable.".to_string()))?;
    Ok(PathBuf::from(home))
}

fn agent_paths() -> Result<(PathBuf, PathBuf), (String, i32)> {
    let home = agent_home()?;
    Ok((
        home.join("Library/LaunchAgents/com.leko.enforcer.plist"),
        home.join("Library/Logs/Leko/agent.log"),
    ))
}

fn render_plist(executable: &Path, settings: &Path, log: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
         <key>Label</key><string>com.leko.enforcer</string>\
         <key>ProgramArguments</key><array><string>{}</string><string>enforce</string><string>{}</string></array>\
         <key>StandardErrorPath</key><string>{}</string>\
         <key>RunAtLoad</key><true/></dict></plist>\n",
        executable.display(), settings.display(), log.display()
    )
}

fn agent(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let action = args.subcommand.as_deref().ok_or_else(|| {
        refused("Usage: leko agent install --executable PATH [--settings PATH] [--write] | status | uninstall [--write]".to_string())
    })?;
    let (plist, log) = agent_paths()?;
    match action {
        "install" => {
            let executable = args.flags.get("executable").ok_or_else(|| {
                refused("The flag --executable needs a value.".to_string())
            })?;
            if !Path::new(executable).is_absolute() {
                return Err(refused("The executable path must be absolute.".to_string()));
            }
            let settings = settings_path(args);
            let proposed = render_plist(Path::new(executable), &settings, &log);
            if args.flags.contains_key("write") {
                if let Some(parent) = plist.parent() {
                    fs::create_dir_all(parent).map_err(|e| refused(e.to_string()))?;
                }
                fs::write(&plist, proposed.as_bytes()).map_err(|e| refused(e.to_string()))?;
            }
            emit(
                json!({"agent": "install", "plist": plist.display().to_string(), "written": args.flags.contains_key("write")}),
                &format!("Agent configuration ready at {}.", plist.display()),
                args,
                out,
            )
        }
        "status" => {
            let installed = plist.exists();
            emit(
                json!({"agent": "status", "installed": installed}),
                &format!("Installed: {}; loaded: false; running: false.", installed),
                args,
                out,
            )
        }
        "uninstall" => {
            if args.flags.contains_key("write") && plist.exists() {
                fs::remove_file(&plist).map_err(|e| refused(e.to_string()))?;
            }
            emit(
                json!({"agent": "uninstall", "removed": plist.exists() == false}),
                "Agent removal prepared.",
                args,
                out,
            )
        }
        _ => Err(refused(format!("Unknown agent action {}.", action))),
    }
}

pub fn run(args: &CliArgs, out: &mut dyn Write, err: &mut dyn Write) -> CommandResult {
    match args.command.as_str() {
        "settings" => settings(args, out),
        "agent" => agent(args, out),
        "enforce" => crate::services::enforcement_loop::enforce(args, out, err),
        _ => Err(refused("That command is not infrastructure.".to_string())),
    }
}
