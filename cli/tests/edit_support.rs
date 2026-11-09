//! The helper modules every edit-command test uses: run the command, own a scratch folder, build
//! the settings zips the edit reads, and capture what it wrote.
#[path = "edit_runs/mod.rs"]
pub mod edit_runs;
#[path = "run_capture/mod.rs"]
pub mod run_capture;
#[path = "scratch_folder/mod.rs"]
pub mod scratch_folder;
#[path = "settings_builder/mod.rs"]
pub mod settings_builder;
#[path = "delay_runs/mod.rs"]
pub mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
pub mod delay_sets;
#[path = "../../core/tests/zip_writer/mod.rs"]
pub mod zip_writer;
