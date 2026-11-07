#![allow(dead_code)]
use crate::run_capture::{run_in_zone, Outcome};
use crate::scratch_folder::Scratch;
use serde_json::{json, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const NOW: i64 = 1_793_629_200_000;
pub fn fixture(lock_hours: Option<i64>) -> (Scratch, PathBuf) {
    let folder = Scratch::new("edit-block");
    let mut block = crate::settings_builder::block("one", json!({"apps": ["com.one", "com.two"]}), json!([crate::settings_builder::always("a", &[])]));
    if let Some(hours) = lock_hours { block["lock"] = json!({"level": "wait", "hours": hours}); }
    let file = crate::settings_builder::write_settings(folder.path(), "settings.zip", &[block]);
    (folder, file)
}
pub fn command(file: &Path, command: &str, args: &[&str], now: i64) -> Outcome {
    let mut words = vec![OsString::from(command), file.as_os_str().to_os_string()];
    words.extend(args.iter().map(OsString::from));
    run_in_zone(words, now, Some("UTC"))
}
pub fn block(file: &Path) -> Value {
    let zip = leko_core::open_zip(&std::fs::read(file).unwrap()).unwrap();
    serde_json::from_slice(&zip.files.iter().find(|(name, _)| name == "blocks/one.json").unwrap().1).unwrap()
}
pub fn waiting(file: &Path) -> PathBuf { PathBuf::from(format!("{}.waiting.json", file.display())) }
