//! Narrow AppKit bridge. Each operation owns an autorelease pool and checks identity again before quitting.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub pid: i32,
    pub bundle: Option<String>,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Application;
    use std::ffi::{c_char, c_void, CStr};
    use std::ptr;

    type Id = *mut c_void;
    type Sel = *const c_void;

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopDefaultMode: Id;
        fn CFRunLoopRunInMode(mode: Id, seconds: f64, return_after_source_handled: u8) -> i32;
    }
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> Id;
        fn objc_autoreleasePoolPop(pool: Id);
    }

    unsafe fn selector(name: &'static [u8]) -> Sel { sel_registerName(name.as_ptr().cast()) }
    unsafe fn class(name: &'static [u8]) -> Id { objc_getClass(name.as_ptr().cast()) }
    unsafe fn id(receiver: Id, name: &'static [u8]) -> Id {
        let call: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
        call(receiver, selector(name))
    }
    unsafe fn integer(receiver: Id, name: &'static [u8]) -> isize {
        let call: unsafe extern "C" fn(Id, Sel) -> isize = std::mem::transmute(objc_msgSend as *const ());
        call(receiver, selector(name))
    }
    unsafe fn bundle(app: Id) -> Option<String> {
        let value = id(app, b"bundleIdentifier\0");
        if value.is_null() { return None; }
        let bytes = id(value, b"UTF8String\0") as *const c_char;
        if bytes.is_null() { return None; }
        let value = CStr::from_ptr(bytes).to_string_lossy().into_owned();
        if value.is_empty() { None } else { Some(value) }
    }

    pub fn running() -> Result<Vec<Application>, String> {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let workspace = id(class(b"NSWorkspace\0"), b"sharedWorkspace\0");
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.001, 0);
            let apps = id(workspace, b"runningApplications\0");
            if workspace.is_null() || apps.is_null() {
                objc_autoreleasePoolPop(pool);
                return Err("Could not read running applications from macOS.".into());
            }
            let count = integer(apps, b"count\0") as usize;
            let object: unsafe extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let mut result = Vec::with_capacity(count);
            for index in 0..count {
                let app = object(apps, selector(b"objectAtIndex:\0"), index);
                result.push(Application { pid: integer(app, b"processIdentifier\0") as i32, bundle: bundle(app) });
            }
            objc_autoreleasePoolPop(pool);
            Ok(result)
        }
    }

    pub fn terminate(app: &Application) -> Result<(), String> {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let lookup: unsafe extern "C" fn(Id, Sel, i32) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let current = lookup(class(b"NSRunningApplication\0"), selector(b"runningApplicationWithProcessIdentifier:\0"), app.pid);
            let result = if current == ptr::null_mut() || bundle(current) != app.bundle {
                Err("application exited or its identity changed".into())
            } else {
                let call: unsafe extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(objc_msgSend as *const ());
                if call(current, selector(b"terminate\0")) != 0 { Ok(()) } else { Err("macOS refused the termination request".into()) }
            };
            objc_autoreleasePoolPop(pool);
            result
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Application;
    pub fn running() -> Result<Vec<Application>, String> { Err("App enforcement requires macOS.".into()) }
    pub fn terminate(_: &Application) -> Result<(), String> { Err("App enforcement requires macOS.".into()) }
}

pub use platform::{running, terminate};
