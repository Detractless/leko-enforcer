//! macOS platform adapters. No policy decisions live here; the core decides.

pub mod applications;
pub mod device_lock;
#[cfg(target_os = "macos")]
pub mod objc;
pub mod windows;

pub use leko_core::models::specs::DeviceAction;

pub use applications::{frontmost, running, terminate, Application};
pub use device_lock::perform as perform_device_action;
pub use windows::visible_apps;
