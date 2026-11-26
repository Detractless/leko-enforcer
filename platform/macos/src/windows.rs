//! Narrow CoreGraphics visible-window query. Never reads window titles so macOS never prompts for Screen Recording.

use crate::applications::{running, Application};

#[derive(Debug, Clone, PartialEq)]
pub struct WindowMeta {
    pub owner_pid: i32,
    pub layer: i32,
    pub alpha: f64,
    pub width: f64,
    pub height: f64,
    pub is_onscreen: bool,
}

pub fn should_count_window(w: &WindowMeta) -> bool {
    w.owner_pid > 0
        && w.is_onscreen
        && w.layer == 0
        && w.alpha > 0.0
        && w.width >= 50.0
        && w.height >= 50.0
}

pub fn visible_bundles_for_windows<F>(
    windows: &[WindowMeta],
    running_fn: F,
) -> Result<Vec<String>, String>
where
    F: FnOnce() -> Result<Vec<Application>, String>,
{
    let apps = running_fn()?;
    let mut result: Vec<String> = Vec::new();
    for window in windows {
        if !should_count_window(window) {
            continue;
        }
        for app in &apps {
            if app.pid != window.owner_pid {
                continue;
            }
            if let Some(bundle) = app.bundle.as_deref().filter(|b| !b.is_empty()) {
                if !result.iter().any(|seen| seen.eq_ignore_ascii_case(bundle)) {
                    result.push(bundle.to_string());
                }
            }
        }
    }
    Ok(result)
}

#[cfg(target_os = "macos")]
mod platform {
    use super::WindowMeta;
    use std::ffi::c_void;

    const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
    const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
    const K_CG_NULL_WINDOW_ID: u32 = 0;
    const K_CF_NUMBER_SINT32_TYPE: isize = 3;
    const K_CF_NUMBER_FLOAT64_TYPE: isize = 6;

    #[repr(C)]
    #[derive(Default)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct CGRect {
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        static kCGWindowOwnerPID: *const c_void;
        static kCGWindowLayer: *const c_void;
        static kCGWindowAlpha: *const c_void;
        static kCGWindowBounds: *const c_void;
        static kCGWindowIsOnscreen: *const c_void;
        fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> *const c_void;
        fn CGRectMakeWithDictionaryRepresentation(dict: *const c_void, rect: *mut CGRect) -> u8;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, the_type: isize, value_ptr: *mut c_void) -> u8;
        fn CFBooleanGetValue(boolean: *const c_void) -> u8;
    }

    unsafe fn dict_i32(dict: *const c_void, key: *const c_void) -> Option<i32> {
        let raw = CFDictionaryGetValue(dict, key);
        if raw.is_null() {
            return None;
        }
        let mut out: i32 = 0;
        if CFNumberGetValue(raw, K_CF_NUMBER_SINT32_TYPE, (&mut out as *mut i32).cast()) != 0 {
            Some(out)
        } else {
            None
        }
    }

    unsafe fn dict_f64(dict: *const c_void, key: *const c_void) -> Option<f64> {
        let raw = CFDictionaryGetValue(dict, key);
        if raw.is_null() {
            return None;
        }
        let mut out: f64 = 0.0;
        if CFNumberGetValue(raw, K_CF_NUMBER_FLOAT64_TYPE, (&mut out as *mut f64).cast()) != 0 {
            Some(out)
        } else {
            None
        }
    }

    pub fn list_on_screen_windows() -> Result<Vec<WindowMeta>, String> {
        unsafe {
            let list = CGWindowListCopyWindowInfo(
                K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
                K_CG_NULL_WINDOW_ID,
            );
            if list.is_null() {
                return Ok(Vec::new());
            }
            let count = CFArrayGetCount(list);
            let mut windows = Vec::with_capacity(count.max(0) as usize);
            for idx in 0..count {
                let dict = CFArrayGetValueAtIndex(list, idx);
                if dict.is_null() {
                    continue;
                }
                let owner_pid = dict_i32(dict, kCGWindowOwnerPID).unwrap_or(0);
                let layer = dict_i32(dict, kCGWindowLayer).unwrap_or(-1);
                let alpha = dict_f64(dict, kCGWindowAlpha).unwrap_or(0.0);
                let onscreen_raw = CFDictionaryGetValue(dict, kCGWindowIsOnscreen);
                let is_onscreen = if onscreen_raw.is_null() {
                    true
                } else {
                    CFBooleanGetValue(onscreen_raw) != 0
                };
                let bounds_raw = CFDictionaryGetValue(dict, kCGWindowBounds);
                let mut rect = CGRect::default();
                let has_bounds = !bounds_raw.is_null()
                    && CGRectMakeWithDictionaryRepresentation(bounds_raw, &mut rect) != 0;
                windows.push(WindowMeta {
                    owner_pid,
                    layer,
                    alpha,
                    width: if has_bounds { rect.size.width } else { 0.0 },
                    height: if has_bounds { rect.size.height } else { 0.0 },
                    is_onscreen,
                });
            }
            CFRelease(list);
            Ok(windows)
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::WindowMeta;
    pub fn list_on_screen_windows() -> Result<Vec<WindowMeta>, String> {
        Ok(Vec::new())
    }
}

pub fn visible_apps() -> Result<Vec<String>, String> {
    let windows = platform::list_on_screen_windows()?;
    visible_bundles_for_windows(&windows, running)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(pid: i32, layer: i32, alpha: f64, width: f64, height: f64, is_onscreen: bool) -> WindowMeta {
        WindowMeta { owner_pid: pid, layer, alpha, width, height, is_onscreen }
    }

    #[test]
    fn filters_invisible_tiny_and_non_normal_windows_and_deduplicates_bundles() {
        let windows = vec![
            meta(10, 0, 1.0, 800.0, 600.0, true),
            meta(10, 0, 1.0, 400.0, 300.0, true),
            meta(11, 25, 1.0, 800.0, 600.0, true),
            meta(12, 0, 0.0, 800.0, 600.0, true),
            meta(13, 0, 1.0, 40.0, 600.0, true),
            meta(14, 0, 1.0, 800.0, 49.0, true),
            meta(15, 0, 1.0, 800.0, 600.0, false),
            meta(16, 0, 0.9, 600.0, 400.0, true),
        ];
        let apps = vec![
            Application { pid: 10, bundle: Some("com.apple.Safari".into()) },
            Application { pid: 11, bundle: Some("com.example.StatusBar".into()) },
            Application { pid: 12, bundle: Some("com.example.HiddenAlpha".into()) },
            Application { pid: 13, bundle: Some("com.example.TooNarrow".into()) },
            Application { pid: 14, bundle: Some("com.example.TooShort".into()) },
            Application { pid: 15, bundle: Some("com.example.Offscreen".into()) },
            Application { pid: 16, bundle: Some("COM.APPLE.SAFARI".into()) },
        ];
        let visible = visible_bundles_for_windows(&windows, || Ok(apps.clone())).unwrap();
        assert_eq!(visible, vec!["com.apple.Safari".to_string()]);
    }
}
