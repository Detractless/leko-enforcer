//! Narrow AppKit bridge. Each operation owns an autorelease pool and checks identity again before quitting.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub pid: i32,
    pub bundle: Option<String>,
}

#[cfg(target_os = "macos")]
mod platform {
    use crate::objc::{objc_autoreleasePoolPop, objc_autoreleasePoolPush, objc_getClass, objc_msgSend, sel_registerName, Id, Sel};
    use super::Application;
    use std::ffi::{c_char, CStr};
    use std::ptr;

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopDefaultMode: Id;
        fn CFRunLoopRunInMode(mode: Id, seconds: f64, return_after_source_handled: u8) -> i32;
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
    unsafe fn boolean(receiver: Id, name: &'static [u8]) -> i8 {
        let call: unsafe extern "C" fn(Id, Sel) -> i8 = std::mem::transmute(objc_msgSend as *const ());
        call(receiver, selector(name))
    }
    unsafe fn drain_run_loop() {
        for _ in 0..16 {
            if CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1) != 4 {
                break;
            }
        }
        CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.005, 0);
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
            drain_run_loop();
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

    fn workspace_frontmost() -> Result<Option<Application>, String> {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let workspace = id(class(b"NSWorkspace\0"), b"sharedWorkspace\0");
            if workspace.is_null() {
                objc_autoreleasePoolPop(pool);
                return Err("Could not read workspace from macOS.".into());
            }
            drain_run_loop();
            let app = id(workspace, b"frontmostApplication\0");
            let found = if app.is_null() {
                None
            } else {
                Some(Application {
                    pid: integer(app, b"processIdentifier\0") as i32,
                    bundle: bundle(app),
                })
            };
            objc_autoreleasePoolPop(pool);
            Ok(found)
        }
    }

    fn active_running_app() -> Result<Option<Application>, String> {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let workspace = id(class(b"NSWorkspace\0"), b"sharedWorkspace\0");
            if workspace.is_null() {
                objc_autoreleasePoolPop(pool);
                return Err("Could not read workspace from macOS.".into());
            }
            let apps = id(workspace, b"runningApplications\0");
            if apps.is_null() {
                objc_autoreleasePoolPop(pool);
                return Ok(None);
            }
            let count = integer(apps, b"count\0") as usize;
            let object: unsafe extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let mut found = None;
            for index in 0..count {
                let app = object(apps, selector(b"objectAtIndex:\0"), index);
                if !app.is_null() && boolean(app, b"isActive\0") != 0 {
                    if let Some(bundle_id) = bundle(app) {
                        found = Some(Application {
                            pid: integer(app, b"processIdentifier\0") as i32,
                            bundle: Some(bundle_id),
                        });
                        break;
                    }
                }
            }
            objc_autoreleasePoolPop(pool);
            Ok(found)
        }
    }

    fn lsappinfo_frontmost() -> Option<Application> {
        let front = std::process::Command::new("lsappinfo")
            .arg("front")
            .output()
            .ok()?;
        if !front.status.success() {
            return None;
        }
        let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
        if asn.is_empty() {
            return None;
        }
        let info = std::process::Command::new("lsappinfo")
            .args(["info", "-only", "pid", "-only", "bundleID", &asn])
            .output()
            .ok()?;
        if !info.status.success() {
            return None;
        }
        super::parse_lsappinfo(&String::from_utf8_lossy(&info.stdout))
    }

    pub fn frontmost() -> Result<Option<Application>, String> {
        super::resolve_frontmost(workspace_frontmost, active_running_app, lsappinfo_frontmost)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Application;
    pub fn running() -> Result<Vec<Application>, String> { Err("App enforcement requires macOS.".into()) }
    pub fn terminate(_: &Application) -> Result<(), String> { Err("App enforcement requires macOS.".into()) }
    pub fn frontmost() -> Result<Option<Application>, String> { Ok(None) }
}

pub fn parse_lsappinfo(text: &str) -> Option<Application> {
    let mut pid: i32 = 0;
    let mut bundle: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"pid\"=") {
            if let Ok(parsed) = rest.trim().parse::<i32>() {
                pid = parsed;
            }
        } else if let Some(rest) = trimmed.strip_prefix("\"CFBundleIdentifier\"=") {
            let value = rest.trim().trim_matches('"').trim();
            if !value.is_empty() && !value.eq_ignore_ascii_case("[NULL]") {
                bundle = Some(value.to_string());
            }
        }
    }
    bundle.map(|b| Application { pid, bundle: Some(b) })
}

pub fn resolve_frontmost<W, A, L>(
    workspace_fn: W,
    active_fn: A,
    lsappinfo_fn: L,
) -> Result<Option<Application>, String>
where
    W: FnOnce() -> Result<Option<Application>, String>,
    A: FnOnce() -> Result<Option<Application>, String>,
    L: FnOnce() -> Option<Application>,
{
    if let Some(app) = lsappinfo_fn().filter(|a| a.bundle.as_deref().is_some_and(|b| !b.is_empty())) {
        return Ok(Some(app));
    }
    if let Some(app) = workspace_fn()?.filter(|a| a.bundle.as_deref().is_some_and(|b| !b.is_empty())) {
        return Ok(Some(app));
    }
    Ok(active_fn()?.filter(|a| a.bundle.as_deref().is_some_and(|b| !b.is_empty())))
}

pub use platform::{frontmost, running, terminate};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lsappinfo_extracts_pid_and_bundle_id() {
        let parsed = parse_lsappinfo("\"pid\"=2283\n\"CFBundleIdentifier\"=\"com.apple.calculator\"\n").unwrap();
        assert_eq!(parsed.pid, 2283);
        assert_eq!(parsed.bundle.as_deref(), Some("com.apple.calculator"));
        assert_eq!(parse_lsappinfo("\"pid\"=99\n\"CFBundleIdentifier\"=[NULL]\n"), None);
    }

    #[test]
    fn frontmost_fallback_chain_recovers_from_stale_or_nil_workspace() {
        let stale = Application { pid: 1, bundle: Some("com.stale.App".into()) };
        let calc = Application { pid: 2, bundle: Some("com.apple.calculator".into()) };
        let res = resolve_frontmost(
            || Ok(Some(stale.clone())),
            || Ok(None),
            || Some(calc.clone()),
        ).unwrap();
        assert_eq!(res, Some(calc.clone()));

        let ws_only = resolve_frontmost(
            || Ok(Some(calc.clone())),
            || Ok(None),
            || None,
        ).unwrap();
        assert_eq!(ws_only, Some(calc.clone()));

        let active_fallback = resolve_frontmost(
            || Ok(None),
            || Ok(Some(calc.clone())),
            || None,
        ).unwrap();
        assert_eq!(active_fallback, Some(calc));
    }
}
