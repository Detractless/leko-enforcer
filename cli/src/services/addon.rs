use std::fs;
use std::io::Write;
use std::path::PathBuf;

use leko_core::io::snapshot::SnapshotStore;
use leko_core::models::CustomVerifierSpec;
use leko_core::services::{
    parse_recipe, pin_bundle, verify_pin, AddonRunner, NonceGuard, NonceIssuer, ProcessRunner,
    RecipeRunner, Verification, VerifierHook,
};
use serde_json::{json, Value};

use crate::arguments::CliArgs;
use crate::commands::{emit_json, emit_text, refused, CommandResult};

const USAGE: &str = "Usage: leko pin-addon --addon-id ID --sha256 HASH --addon-source DIR [--snapshot-root DIR] [--config JSON] | list-addons --snapshot-root DIR | verify-addon --addon-id ID --sha256 HASH --snapshot-root DIR";

fn flag(args: &CliArgs, name: &str) -> Result<String, (String, i32)> {
    args.flags.get(name).cloned().ok_or_else(|| refused(format!("The flag --{} needs a value.", name), 2))
}

fn config_of(args: &CliArgs) -> Result<Value, (String, i32)> {
    match args.flags.get("config") {
        None => Ok(json!({})),
        Some(text) => serde_json::from_str(text)
            .map_err(|_| refused("The config flag needs JSON.".to_string(), 2)),
    }
}

fn snapshot_root(args: &CliArgs) -> Result<PathBuf, (String, i32)> {
    Ok(PathBuf::from(
        args.flags.get("snapshot-root").cloned().unwrap_or_else(|| "leko-snapshots".to_string()),
    ))
}

fn pin(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let addon_id = flag(args, "addon-id")?;
    let sha256 = flag(args, "sha256")?;
    let source = PathBuf::from(flag(args, "addon-source")?);
    let root = snapshot_root(args)?;
    let spec = CustomVerifierSpec { addon_id: addon_id.clone(), sha256, config: config_of(args)? };
    let target = pin_bundle(&source, &root, &spec)
        .map_err(|error| refused(error.to_string(), 1))?;
    if args.flags.contains_key("json") {
        emit_json(json!({"addon_id": addon_id, "snapshot": target.display().to_string()}), out)
    } else {
        emit_text(&format!("Pinned add-on {} at {}.", addon_id, target.display()), out)
    }
}

fn list(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let root = snapshot_root(args)?;
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    names.sort();
    if args.flags.contains_key("json") {
        emit_json(json!({"addons": names}), out)
    } else if names.is_empty() {
        emit_text("No add-ons are pinned.", out)
    } else {
        for name in names {
            emit_text(&name, out)?;
        }
        Ok(())
    }
}

fn verify(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    let addon_id = flag(args, "addon-id")?;
    let sha256 = flag(args, "sha256")?;
    let root = snapshot_root(args)?;
    let spec = CustomVerifierSpec { addon_id: addon_id.clone(), sha256, config: config_of(args)? };
    let ok = verify_pin(&root, &spec);
    if args.flags.contains_key("json") {
        emit_json(json!({"addon_id": addon_id, "verified": ok}), out)
    } else if ok {
        emit_text(&format!("Add-on {} matches its pin.", addon_id), out)
    } else {
        Err(refused(format!("Add-on {} does not match its pin.", addon_id), 1))
    }
}

pub fn check_pinned(args: &CliArgs) -> Result<bool, String> {
    let Some(addon_id) = args.flags.get("addon-id") else {
        return Ok(true);
    };
    let sha256 = flag(args, "sha256").map_err(|error| error.0)?;
    let mut config = config_of(args).map_err(|error| error.0)?;
    if let Some(path) = args.flags.get("executable") {
        config["executable"] = json!(path);
    }
    let source = PathBuf::from(
        args.flags.get("addon-source").cloned().unwrap_or_else(|| ".".to_string()),
    );
    let root = snapshot_root(args).map_err(|error| error.0)?;
    let spec = CustomVerifierSpec { addon_id: addon_id.clone(), sha256, config };
    let mut snapshots = SnapshotStore::with_quarantine(&source, &root);
    snapshots.pin(&spec).map_err(|error| error.to_string())?;
    let runner: Option<Box<dyn AddonRunner>> = if let Some(text) = args.flags.get("recipe") {
        Some(Box::new(RecipeRunner {
            recipe: parse_recipe(text).map_err(|error| error.to_string())?,
        }))
    } else if args.flags.contains_key("executable") {
        Some(Box::new(ProcessRunner::default()))
    } else {
        None
    };
    let mut guard = NonceGuard::default();
    let mut hook = VerifierHook::new(&snapshots, &mut guard);
    let nonce = NonceIssuer::new().issue();
    Ok(hook.evaluate(&spec, runner.as_deref(), &nonce) == Verification::Satisfied)
}

pub fn run(args: &CliArgs, out: &mut dyn Write) -> CommandResult {
    match args.command.as_str() {
        "pin-addon" => pin(args, out),
        "list-addons" => list(args, out),
        "verify-addon" => verify(args, out),
        _ => Err(refused(USAGE.to_string(), 2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arguments::parse;
    use leko_core::services::{digest_bytes, manifest_digest, BundleManifest};

    fn call(text: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = text.iter().map(|value| value.to_string()).collect();
        let parsed = parse(&args).unwrap();
        let mut out = Vec::new();
        let code = match run(&parsed, &mut out) {
            Ok(()) => 0,
            Err((_, code)) => code,
        };
        (code, String::from_utf8(out).unwrap(), String::new())
    }

    fn clean(path: &std::path::Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let child = entry.path();
                    if child.is_dir() {
                        let _ = fs::set_permissions(&child, fs::Permissions::from_mode(0o755));
                        clean(&child);
                    } else {
                        let _ = fs::set_permissions(&child, fs::Permissions::from_mode(0o644));
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(path);
    }

    fn bundle(label: &str) -> (PathBuf, PathBuf, String) {
        let base = std::env::temp_dir().join(format!("leko-cli-addon-{}-{}", std::process::id(), label));
        clean(&base);
        let source = base.join("source");
        fs::create_dir_all(&source).unwrap();
        let payload = b"payload";
        let config = json!({});
        let manifest = json!({
            "version": 1,
            "addon_id": "checker",
            "config_sha256": digest_bytes(&serde_json::to_vec(&config).unwrap()),
            "files": [{"name": "addon.bin", "sha256": digest_bytes(payload)}],
        });
        let parsed: BundleManifest = serde_json::from_value(manifest.clone()).unwrap();
        let digest = manifest_digest(&parsed);
        fs::write(source.join("addon.bin"), payload).unwrap();
        fs::write(source.join("manifest.json"), serde_json::to_vec(&manifest).unwrap()).unwrap();
        (source, base.join("snapshots"), digest)
    }

    #[test]
    fn canonical_addon_commands_and_stable_json() {
        let (source, root, digest) = bundle("json");
        let source_s = source.to_string_lossy();
        let root_s = root.to_string_lossy();
        let (code, out, _) = call(&["pin-addon", "--addon-id", "checker", "--sha256", &digest,
                                    "--addon-source", &source_s, "--snapshot-root", &root_s, "--json"]);
        assert_eq!(code, 0, "{}", out);
        let value: Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(value["addon_id"], "checker");

        let (code, out, _) = call(&["list-addons", "--snapshot-root", &root_s, "--json"]);
        assert_eq!(code, 0);
        let list: Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(list["addons"].as_array().unwrap().len(), 1);

        let (code, out, _) = call(&["verify-addon", "--addon-id", "checker", "--sha256", &digest,
                                    "--snapshot-root", &root_s, "--json"]);
        assert_eq!(code, 0, "{}", out);
        let verified: Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(verified["verified"], true);
        clean(source.parent().unwrap());
    }

    #[test]
    fn usage_and_streams_follow_the_cli_contract() {
        let (code, _, _) = call(&["pin-addon"]);
        assert_eq!(code, 2);
        let (code, _, _) = call(&["verify-addon", "--addon-id", "x"]);
        assert_eq!(code, 2);
    }

    #[test]
    fn repeated_and_empty_addon_requests_are_safe() {
        let (source, root, digest) = bundle("repeat");
        let source_s = source.to_string_lossy();
        let root_s = root.to_string_lossy();
        let command = ["pin-addon", "--addon-id", "checker", "--sha256", &digest,
                       "--addon-source", &source_s, "--snapshot-root", &root_s];
        let (first, _, _) = call(&command);
        let (second, _, _) = call(&command);
        assert_eq!(first, 0);
        assert_eq!(second, 0);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);

        let (code, out, _) = call(&["list-addons", "--snapshot-root",
                                    std::env::temp_dir().join(format!("leko-empty-{}", std::process::id()))
                                        .to_string_lossy().as_ref()]);
        assert_eq!(code, 0);
        assert!(out.contains("No add-ons"));
        clean(source.parent().unwrap());
    }
}
