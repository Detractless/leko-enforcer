//! Low-level test modules for building settings, capturing command output, and owning scratch folders.
#![allow(dead_code)]

#[path = "run_capture/mod.rs"]
pub mod run_capture;
#[path = "scratch_folder/mod.rs"]
pub mod scratch_folder;
#[path = "settings_builder/mod.rs"]
pub mod settings_builder;
