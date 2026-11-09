//! The few Objective-C runtime and autorelease-pool declarations the macOS bridges share.

#![cfg(target_os = "macos")]

use std::ffi::{c_char, c_void};

pub type Id = *mut c_void;
pub type Sel = *const c_void;

#[link(name = "objc")]
extern "C" {
    pub fn objc_getClass(name: *const c_char) -> Id;
    pub fn sel_registerName(name: *const c_char) -> Sel;
    pub fn objc_msgSend();
    pub fn objc_autoreleasePoolPush() -> Id;
    pub fn objc_autoreleasePoolPop(pool: Id);
}
