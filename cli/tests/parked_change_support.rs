//! The helper modules every parked-change command test uses: run the library in this process, keep
//! what it wrote, own a scratch folder, and build the settings zips those commands read.
#[path = "delay_runs/mod.rs"]
pub mod delay_runs;
#[path = "../../core/tests/delay_sets/mod.rs"]
pub mod delay_sets;
#[path = "run_capture/mod.rs"]
pub mod run_capture;
#[path = "scratch_folder/mod.rs"]
pub mod scratch_folder;
#[path = "../../core/tests/zip_writer/mod.rs"]
pub mod zip_writer;
