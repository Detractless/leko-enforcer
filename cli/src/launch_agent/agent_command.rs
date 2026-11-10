//! Preview and explicit installation, inspection, and removal of the user's LaunchAgent.
use super::agent_control::{Backend, Launchctl, Service};
use super::agent_plist::{self, FILE_NAME, LABEL};
use crate::settings_store;
use crate::status::settings_file::read_settings;
use crate::status::zone_choice::choose_zone;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

const USAGE: &str = "Usage: leko agent install --executable PATH [--settings PATH] [--write] | status | uninstall [--write]";

fn paths(home: &Path) -> Result<(PathBuf, PathBuf), String> {
    if !home.is_absolute() { return Err("The home folder must be an absolute path.".into()); }
    let home = home.canonicalize().map_err(|e| format!("Could not read the home folder: {e}"))?;
    Ok((home.join("Library/LaunchAgents").join(FILE_NAME), home.join("Library/Logs/Leko/agent.log")))
}

fn read_owned(path: &Path, log: &Path) -> Result<Option<String>, String> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Could not inspect the agent plist: {e}")),
        Ok(meta) if !meta.file_type().is_file() || meta.len() > 65536 => Err("The agent plist is not a regular, reasonably sized file.".into()),
        Ok(_) => {
            let contents = std::fs::read_to_string(path).map_err(|e| format!("Could not read the agent plist: {e}"))?;
            if agent_plist::owned(&contents, log) { Ok(Some(contents)) } else { Err("The existing plist is not a recognized Leko agent; it was left untouched.".into()) }
        }
    }
}

fn same_service(service: &Service, path: &Path) -> Result<(), String> {
    if let Service::Loaded { path: reported, .. } = service {
        if Path::new(reported) != path { return Err("A different service uses the Leko label; it was left untouched.".into()); }
    }
    Ok(())
}

fn check_executable(path: &Path) -> Result<(), String> {
    if !path.is_absolute() { return Err("The executable path must be absolute.".into()); }
    let facts = std::fs::metadata(path).map_err(|e| format!("Could not read the executable: {e}"))?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if !facts.is_file() || facts.permissions().mode() & 0o111 == 0 { return Err("The executable is not a runnable file.".into()); }
        let allowed = std::process::Command::new("/bin/test").arg("-x").arg(path).status().map_err(|e| format!("Could not check executable access: {e}"))?;
        if !allowed.success() { return Err("The executable is not runnable by the current user.".into()); }
    }
    #[cfg(not(unix))] if !facts.is_file() { return Err("The executable is not a file.".into()); }
    Ok(())
}

fn install(args: &[String], home: &Path, zone: Option<&str>, out: &mut dyn Write, backend: &mut dyn Backend) -> Result<(), String> {
    let (mut exe, mut settings, mut write) = (None, None, false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--write" if !write => write = true,
            "--executable" | "--settings" => {
                let option = &args[i]; i += 1;
                let value = args.get(i).filter(|s| !s.starts_with('-')).ok_or_else(|| format!("{option} needs a path."))?;
                let target = if option == "--executable" { &mut exe } else { &mut settings };
                if target.replace(PathBuf::from(value)).is_some() { return Err(format!("{option} was repeated.")); }
            }
            _ => return Err(USAGE.into()),
        }
        i += 1;
    }
    let exe = exe.ok_or("An executable path is required.")?;
    let settings = match settings {
        Some(path) => path,
        None => settings_store::default_path(Some(home.as_os_str()))?,
    };
    check_executable(&exe)?;
    if !settings.is_absolute() { return Err("The settings path must be absolute.".into()); }
    read_settings(settings.to_str().ok_or("The settings path is not valid text.")?)?;
    choose_zone(None, zone)?;
    let (plist, log) = paths(home)?;
    let proposed = agent_plist::render(LABEL, &exe, &settings, &log)?;
    let existing = read_owned(&plist, &log)?;
    if existing.as_ref().is_some_and(|old| old != &proposed) { return Err("A different Leko agent is already installed; remove it first.".into()); }
    if !write {
        writeln!(out, "Would install {} with this configuration:\n{proposed}Nothing was changed.", plist.display()).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let service = backend.status(LABEL)?;
    same_service(&service, &plist)?;
    if existing.is_none() && service != Service::Absent { return Err("A service already uses the label without an owned plist; nothing was changed.".into()); }
    std::fs::create_dir_all(plist.parent().unwrap()).map_err(|e| format!("Could not create the LaunchAgents folder: {e}"))?;
    std::fs::create_dir_all(log.parent().unwrap()).map_err(|e| format!("Could not create the log folder: {e}"))?;
    if existing.is_none() {
        use std::fs::OpenOptions;
        let mut file = OpenOptions::new().write(true).create_new(true).open(&plist).map_err(|e| format!("Could not create the agent plist: {e}"))?;
        file.write_all(proposed.as_bytes()).and_then(|_| file.sync_all()).map_err(|e| format!("Could not save the agent plist: {e}"))?;
    }
    if service == Service::Absent { backend.bootstrap(&plist)?; }
    match backend.status(LABEL)? {
        Service::Loaded { path, pid } if Path::new(&path) == plist => writeln!(out, "Agent registered; {}.", if pid.is_some() { "running" } else { "not currently running" }).map_err(|e| e.to_string()),
        _ => Err("Registration did not produce the expected service; check the plist before retrying.".into()),
    }
}

fn status(home: &Path, out: &mut dyn Write, backend: &mut dyn Backend) -> Result<(), String> {
    let (plist, log) = paths(home)?;
    let installed = read_owned(&plist, &log)?.is_some();
    let service = backend.status(LABEL)?;
    same_service(&service, &plist)?;
    match service {
        Service::Absent => writeln!(out, "Installed: {installed}; loaded: false; running: false."),
        Service::Loaded { pid, .. } => writeln!(out, "Installed: {installed}; loaded: true; running: {}.", pid.is_some()),
    }.map_err(|e| e.to_string())
}

fn uninstall(args: &[String], home: &Path, out: &mut dyn Write, backend: &mut dyn Backend) -> Result<(), String> {
    let write = match args { [] => false, [flag] if flag == "--write" => true, _ => return Err(USAGE.into()) };
    let (plist, log) = paths(home)?;
    let existing = read_owned(&plist, &log)?;
    if existing.is_none() { return Err("No recognized Leko agent plist was found; nothing was changed.".into()); }
    if !write { return writeln!(out, "Would unload {LABEL} and remove {}; settings remain unchanged. Nothing was changed.", plist.display()).map_err(|e| e.to_string()); }
    let service = backend.status(LABEL)?;
    same_service(&service, &plist)?;
    if service != Service::Absent { backend.bootout(LABEL)?; }
    if read_owned(&plist, &log)? != existing { return Err("The plist changed during removal; it was left in place.".into()); }
    std::fs::remove_file(&plist).map_err(|e| format!("The agent was unloaded but its plist could not be removed: {e}"))?;
    writeln!(out, "Agent removed; settings remain unchanged.").map_err(|e| e.to_string())
}

pub fn run(args: &[OsString], home: Option<&std::ffi::OsStr>, zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut backend = match Launchctl::current() {
        Ok(backend) => backend,
        Err(problem) => { let _ = writeln!(err, "{problem}"); return 1; }
    };
    run_with_backend(args, home, zone, out, err, &mut backend)
}

pub fn run_with_backend(args: &[OsString], home: Option<&std::ffi::OsStr>, zone: Option<&str>, out: &mut dyn Write, err: &mut dyn Write, backend: &mut dyn Backend) -> i32 {
    let result = (|| {
        let home = Path::new(home.ok_or("The home folder is unavailable.")?);
        let words: Vec<String> = args.iter().map(|arg| arg.to_str().map(str::to_string).ok_or("Arguments must be valid text.")).collect::<Result<_, _>>()?;
        match words.split_first() {
            Some((action, rest)) if action == "install" => install(rest, home, zone, out, backend),
            Some((action, rest)) if action == "status" && rest.is_empty() => status(home, out, backend),
            Some((action, rest)) if action == "uninstall" => uninstall(rest, home, out, backend),
            _ => Err(USAGE.into()),
        }
    })();
    match result { Ok(()) => 0, Err(problem) => { let _ = writeln!(err, "{problem}"); 1 } }
}
