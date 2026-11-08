//! Narrow bridge to the two system requests a device block makes: lock the screen, or end the
//! session (log out or shut down). A missing private symbol or a refused Apple event is a sentence,
//! never a panic. The screen lock uses the system's own login framework, as Cold Turkey does; the
//! session requests are Apple events sent to the system process.

use leko_core::models::specs::DeviceAction;

#[cfg(target_os = "macos")]
mod platform {
    use super::DeviceAction;
    use std::ffi::{c_char, c_int, c_void, CString};
    use std::ptr;

    type Id = *mut c_void;
    type Sel = *const c_void;

    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
        fn objc_autoreleasePoolPush() -> Id;
        fn objc_autoreleasePoolPop(pool: Id);
    }

    extern "C" {
        fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    const RTLD_LAZY: c_int = 1;
    /// The private framework that holds the screen lock, as Cold Turkey finds it.
    const LOGIN_FRAMEWORK: &str = "/System/Library/PrivateFrameworks/login.framework/Versions/Current/login";
    const AEVT: u32 = 0x61657674; // 'aevt'
    const PSN: u32 = 0x70736e20; // 'psn '
    const LOG_OUT: u32 = 0x726c676f; // 'rlgo' (kAEReallyLogOut)
    const SHUT_DOWN: u32 = 0x73687574; // 'shut'

    unsafe fn selector(name: &'static [u8]) -> Sel { sel_registerName(name.as_ptr().cast()) }
    unsafe fn class(name: &'static [u8]) -> Id { objc_getClass(name.as_ptr().cast()) }

    /// Lock the screen through the login framework's own request. A framework or symbol that is not
    /// there is a sentence, so a macOS that moved it says so instead of crashing.
    fn lock() -> Result<(), String> {
        unsafe {
            let path = CString::new(LOGIN_FRAMEWORK).map_err(|_| "The screen lock path is not usable.".to_string())?;
            let handle = dlopen(path.as_ptr(), RTLD_LAZY);
            if handle.is_null() {
                return Err("the system's login framework could not be opened".into());
            }
            let name = CString::new("SACLockScreenImmediate").map_err(|_| "The screen lock symbol is not usable.".to_string())?;
            let symbol = dlsym(handle, name.as_ptr());
            if symbol.is_null() {
                return Err("the system's login framework has no screen lock".into());
            }
            let lock_screen: unsafe extern "C" fn() -> c_int = std::mem::transmute(symbol);
            lock_screen();
        }
        Ok(())
    }

    /// Send `event_id` to the system process, the way an application asks the session to end.
    fn apple_event(event_id: u32) -> Result<(), String> {
        unsafe {
            let pool = objc_autoreleasePoolPush();
            let result = send(event_id);
            objc_autoreleasePoolPop(pool);
            result
        }
    }

    unsafe fn send(event_id: u32) -> Result<(), String> {
        let mut psn: [u8; 8] = [0, 0, 0, 0, 1, 0, 0, 0];
        let make_target: unsafe extern "C" fn(Id, Sel, u32, *const c_void, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let target = make_target(class(b"NSAppleEventDescriptor\0"), selector(b"descriptorWithDescriptorType:bytes:length:\0"), PSN, psn.as_mut_ptr().cast(), 8);
        if target.is_null() {
            return Err("the system process could not be addressed".into());
        }
        let allocate: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let allocated = allocate(class(b"NSAppleEventDescriptor\0"), selector(b"alloc\0"));
        let init: unsafe extern "C" fn(Id, Sel, u32, u32, Id, i16, i32) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let event = init(allocated, selector(b"initWithEventClass:eventID:targetDescriptor:returnID:transactionID:\0"), AEVT, event_id, target, -1, 0);
        if event.is_null() {
            return Err("the session request could not be made".into());
        }
        let mut error: Id = ptr::null_mut();
        let send: unsafe extern "C" fn(Id, Sel, u32, f64, *mut Id) -> Id = std::mem::transmute(objc_msgSend as *const ());
        let sent = send(event, selector(b"sendEventWithOptions:timeout:error:\0"), 0x23, -1.0, &mut error);
        if sent.is_null() {
            return Err("macOS refused the session request".into());
        }
        Ok(())
    }

    pub fn perform(action: DeviceAction) -> Result<(), String> {
        match action {
            DeviceAction::Lock => lock(),
            DeviceAction::Logoff => apple_event(LOG_OUT),
            DeviceAction::Shutdown => apple_event(SHUT_DOWN),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::DeviceAction;
    pub fn perform(_: DeviceAction) -> Result<(), String> { Err("a device block needs macOS.".into()) }
}

pub use platform::perform;
