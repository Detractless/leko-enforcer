//! The program's clock, time zone and operating-system entry point.
use leko_cli::session::run_command::EXIT_REFUSED;
use std::{ffi::OsString, io::Write, time::{SystemTime, UNIX_EPOCH}};

fn now_ms() -> Result<i64, String> {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| "The clock is set before 1970, so no session was made.".to_string())?;
    i64::try_from(since.as_millis()).map_err(|_| "The clock is too far in the future to read, so no session was made.".to_string())
}

fn computer_zone() -> Option<String> {
    if let Some(name) = std::env::var_os("TZ").filter(|name| !name.is_empty()) {
        return Some(name.to_string_lossy().trim_start_matches(':').to_string());
    }
    let target = std::fs::read_link("/etc/localtime").ok()?;
    target.to_str()?.split_once("zoneinfo/").map(|(_, name)| name.to_string())
}
fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "settings") { std::process::exit(leko_cli::settings_store::run(&args[1..], std::env::var_os("HOME").as_deref(), &mut std::io::stdout(), &mut std::io::stderr())); }
    if args.first().is_some_and(|arg| arg == "agent") { std::process::exit(leko_cli::launch_agent::agent_command::run(&args[1..], std::env::var_os("HOME").as_deref(), computer_zone().as_deref(), &mut std::io::stdout(), &mut std::io::stderr())); }
    if args.first().is_some_and(|arg| arg == "enforce") {
        if args.len() != 2 || args[1].to_str().is_none() {
            eprintln!("Usage: leko enforce FILE.zip"); std::process::exit(2);
        }
        let started = std::time::Instant::now(); let code = leko_cli::enforcement::enforcement_loop::run(
            args[1].to_str().unwrap(), now_ms, computer_zone,
            leko_cli::enforcement::macos_applications::running, leko_cli::enforcement::macos_applications::terminate, std::process::id() as i32,
            || started.elapsed().as_millis().min(u64::MAX as u128) as u64, |ms| std::thread::sleep(std::time::Duration::from_millis(ms)),
            leko_cli::enforcement::macos_device_lock::perform, &mut std::io::stderr(),
        );
        std::process::exit(code);
    }
    let code = match now_ms() {
        Ok(now) => leko_cli::run(&args, now, computer_zone().as_deref(), &mut std::io::stdout(), &mut std::io::stderr()),
        Err(sentence) => {
            let _ = writeln!(std::io::stderr(), "{sentence}"); EXIT_REFUSED
        }
    };
    std::process::exit(code);
}
