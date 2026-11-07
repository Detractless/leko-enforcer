#![allow(dead_code)]
use leko_cli::launch_agent::agent_control::{Backend, Service};
use std::path::Path;

#[derive(Default)]
pub struct Mock { pub path: Option<String>, pub pid: Option<u32>, pub starts: usize, pub stops: usize, pub fail_start: bool }
impl Backend for Mock {
    fn status(&mut self, _: &str) -> Result<Service, String> {
        Ok(match &self.path { Some(path) => Service::Loaded { path: path.clone(), pid: self.pid }, None => Service::Absent })
    }
    fn bootstrap(&mut self, path: &Path) -> Result<(), String> {
        self.starts += 1;
        if self.fail_start { return Err("launchd refused registration".into()); }
        self.path = Some(path.to_str().unwrap().into()); self.pid = Some(123); Ok(())
    }
    fn bootout(&mut self, _: &str) -> Result<(), String> { self.stops += 1; self.path = None; self.pid = None; Ok(()) }
}
