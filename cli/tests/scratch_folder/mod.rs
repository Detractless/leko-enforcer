//! A folder a test owns: made fresh, removed (even when read-only) when the test ends.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!("leko-cli-{label}-{}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::SeqCst)));
        std::fs::create_dir_all(&path).expect("a scratch folder");
        Scratch(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The names inside the folder, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0).expect("readable").map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[cfg(unix)]
    pub fn set_mode(&self, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(mode)).expect("permissions");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        self.set_mode(0o755);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
