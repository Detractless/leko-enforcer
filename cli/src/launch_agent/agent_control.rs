//! Bounded calls to launchctl in the current user's GUI domain.
use std::path::Path;
use std::process::Command;

#[derive(Debug, PartialEq, Eq)]
pub enum Service { Absent, Loaded { path: String, pid: Option<u32> } }

pub trait Backend {
    fn status(&mut self, label: &str) -> Result<Service, String>;
    fn bootstrap(&mut self, path: &Path) -> Result<(), String>;
    fn bootout(&mut self, label: &str) -> Result<(), String>;
}

pub struct Launchctl { domain: String }
impl Launchctl {
    pub fn current() -> Result<Self, String> { Ok(Self { domain: current_domain()? }) }
}
impl Backend for Launchctl {
    fn status(&mut self, label: &str) -> Result<Service, String> { status(&self.domain, label) }
    fn bootstrap(&mut self, path: &Path) -> Result<(), String> { bootstrap(&self.domain, path) }
    fn bootout(&mut self, label: &str) -> Result<(), String> { bootout(&self.domain, label) }
}

pub fn current_domain() -> Result<String, String> {
    let output = Command::new("/usr/bin/id").arg("-u").output().map_err(|e| format!("Could not read the current user: {e}"))?;
    let uid = String::from_utf8(output.stdout).map_err(|_| "The current user ID was not text.")?;
    let uid = uid.trim().parse::<u32>().map_err(|_| "The current user ID was invalid.")?;
    if !output.status.success() || uid == 0 { return Err("A logged-in, non-root user is required.".into()); }
    Ok(format!("gui/{uid}"))
}

fn call(args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("/bin/launchctl").args(args).output().map_err(|e| format!("Could not run launchctl: {e}"))
}

pub fn status(domain: &str, label: &str) -> Result<Service, String> {
    let target = format!("{domain}/{label}");
    let output = call(&["print", &target])?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        if output.status.code() == Some(113) && message.contains("Could not find service") {
            let domain_output = call(&["print", domain])?;
            return if domain_output.status.success() { Ok(Service::Absent) } else { Err("The user's GUI launchd domain is unavailable.".into()) };
        }
        return Err(format!("Could not inspect the agent: {}", message.trim()));
    }
    parse_status(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse_status(output: &str) -> Result<Service, String> {
    let value = |name: &str| output.lines().find_map(|line| line.trim().strip_prefix(name).map(str::trim));
    let path = value("path = ").ok_or("launchctl did not report the agent path.")?.to_string();
    let state = value("state = ").ok_or("launchctl did not report the agent state.")?;
    let pid = value("pid = ").map(|p| p.parse::<u32>().map_err(|_| "launchctl reported an invalid process ID.")).transpose()?;
    if state == "running" && pid.is_none() { return Err("launchctl reported running without a process ID.".into()); }
    if state != "running" && pid.is_some() { return Err("launchctl reported an inconsistent process state.".into()); }
    Ok(Service::Loaded { path, pid })
}

pub fn bootstrap(domain: &str, path: &Path) -> Result<(), String> {
    let path = path.to_str().ok_or("The plist path is not valid text.")?;
    let output = call(&["bootstrap", domain, path])?;
    if output.status.success() { Ok(()) } else { Err(format!("Could not register the agent: {}", String::from_utf8_lossy(&output.stderr).trim())) }
}

pub fn bootout(domain: &str, label: &str) -> Result<(), String> {
    let output = call(&["bootout", &format!("{domain}/{label}")])?;
    if output.status.success() { Ok(()) } else { Err(format!("Could not unload the agent: {}", String::from_utf8_lossy(&output.stderr).trim())) }
}
