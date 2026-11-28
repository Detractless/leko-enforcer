use std::path::Path;

type Rect = (f64, f64, f64, f64);

pub fn clamp(rect: Rect, screen: Rect) -> Rect {
    let (x, y, w, h) = rect;
    let (sx, sy, sw, sh) = screen;
    (x.clamp(sx, (sx + sw - w).max(sx)), y.clamp(sy, (sy + sh - h).max(sy)), w, h)
}

pub fn clamp_displays(rect: Rect, screens: &[Rect]) -> Rect {
    screens.iter().map(|screen| clamp(rect, *screen)).min_by(|a, b| {
        let dist = |r: &Rect| (r.0 - rect.0).powi(2) + (r.1 - rect.1).powi(2);
        dist(a).total_cmp(&dist(b))
    }).unwrap_or(rect)
}

pub fn restore_displays(path: &Path, fallback: Rect, screens: &[Rect]) -> Rect {
    let position = std::fs::read_to_string(path).ok()
        .and_then(|s| serde_json::from_str::<(f64, f64)>(&s).ok())
        .filter(|(x, y)| x.is_finite() && y.is_finite());
    clamp_displays(position.map_or(fallback, |(x, y)| (x, y, fallback.2, fallback.3)), screens)
}

pub fn card_frame(slot: Rect, size: Rect, screen: Rect) -> (Rect, &'static str) {
    let right = slot.0 + slot.2 / 2.0 >= screen.0 + screen.2 / 2.0;
    let top = slot.1 + slot.3 / 2.0 < screen.1 + screen.3 / 2.0;
    let anchor = match (top, right) { (true, true) => "top-right", (true, false) => "top-left", (false, true) => "bottom-right", (false, false) => "bottom-left" };
    let x = if right { slot.0 + slot.2 - size.2 } else { slot.0 };
    let y = if top { slot.1 - 12.0 } else { slot.1 + slot.3 + 12.0 - size.3 };
    ((x, y, size.2, size.3), anchor)
}

pub fn save(path: &Path, rect: Rect) -> std::io::Result<()> {
    let body = serde_json::to_vec(&(rect.0, rect.1))?;
    let parent = path.parent().ok_or_else(|| std::io::Error::other("missing parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, body)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn movement_clamps_and_anchors_card() {
        let compact = clamp((-100.0, 850.0, 160.0, 56.0), (0.0, 0.0, 1000.0, 900.0));
        assert_eq!(compact, (0.0, 844.0, 160.0, 56.0));
        let slot = (compact.0, compact.1 + 12.0, 160.0, 32.0);
        let (card, _) = card_frame(slot, (0.0, 0.0, 380.0, 260.0), (0.0, 0.0, 1000.0, 900.0));
        assert_eq!(slot.0, card.0);
        assert_eq!(compact.1 + compact.3, card.1 + card.3);
    }
    #[test]
    fn interrupted_drag_recovers_input() {
        use crate::native_input::{CursorPoint, NativeInput, TestCursor};
        let mut input = NativeInput::new((0.0, 0.0, 160.0, 56.0), TestCursor { point: Some(CursorPoint { x: 20.0, y: 20.0 }) });
        input.poll();
        assert!(!input.update_drag(true));
        input.provider.point = Some(CursorPoint { x: 500.0, y: 500.0 });
        input.poll();
        assert!(input.update_drag(false));
        assert!(input.ignores_mouse);
    }
    #[test]
    fn secondary_displays_and_removal_keep_pill_visible() {
        let screens = [(0.0, 0.0, 1000.0, 900.0), (-800.0, 0.0, 800.0, 900.0)];
        let secondary = (-700.0, 200.0, 160.0, 56.0);
        assert_eq!(clamp_displays(secondary, &screens), secondary);
        assert_eq!(clamp_displays(secondary, &screens[..1]).0, 0.0);
    }
    #[test]
    fn card_opens_toward_available_screen_space() {
        let screen = (0.0, 0.0, 1000.0, 900.0);
        let slot = (0.0, 12.0, 160.0, 32.0);
        let (frame, anchor) = card_frame(slot, (0.0, 0.0, 380.0, 260.0), screen);
        assert_eq!(anchor, "top-left");
        assert_eq!(frame, (0.0, 0.0, 380.0, 260.0));
    }
    #[test]
    fn saved_position_restores_on_available_display() {
        let path = std::env::temp_dir().join(format!("leko-move-test-{}.json", std::process::id()));
        save(&path, (3000.0, 500.0, 160.0, 56.0)).unwrap();
        assert_eq!(restore_displays(&path, (0.0, 0.0, 160.0, 56.0), &[(0.0, 0.0, 1000.0, 900.0)]), (840.0, 500.0, 160.0, 56.0));
        std::fs::remove_file(path).unwrap();
    }
}

#[cfg(target_os = "macos")]
mod shortcut {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    static PRESSED: AtomicBool = AtomicBool::new(false);
    const SIGNATURE: u32 = u32::from_be_bytes(*b"Leko");
    #[repr(C)]
    struct EventType { class: u32, kind: u32 }
    #[repr(C)]
    struct HotKeyId { signature: u32, id: u32 }
    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn GetApplicationEventTarget() -> *mut c_void;
        fn InstallEventHandler(target: *mut c_void, handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32, count: u32, events: *const EventType, data: *mut c_void, out: *mut *mut c_void) -> i32;
        fn RemoveEventHandler(handler: *mut c_void) -> i32;
        fn RegisterEventHotKey(code: u32, modifiers: u32, id: HotKeyId, target: *mut c_void, options: u32, out: *mut *mut c_void) -> i32;
        fn UnregisterEventHotKey(key: *mut c_void) -> i32;
        fn GetEventParameter(event: *mut c_void, name: u32, ty: u32, actual: *mut u32, size: u32, actual_size: *mut u32, data: *mut c_void) -> i32;
    }
    extern "C" fn handle(_: *mut c_void, event: *mut c_void, _: *mut c_void) -> i32 {
        let mut id = HotKeyId { signature: 0, id: 0 };
        let status = unsafe { GetEventParameter(event, u32::from_be_bytes(*b"----"), u32::from_be_bytes(*b"hkid"), std::ptr::null_mut(), std::mem::size_of::<HotKeyId>() as u32, std::ptr::null_mut(), &mut id as *mut _ as *mut c_void) };
        if status == 0 && id.signature == SIGNATURE && id.id == 1 { PRESSED.store(true, Ordering::SeqCst); 0 } else { -9874 }
    }
    pub struct KeyboardShortcut { handler: *mut c_void, key: *mut c_void }
    impl KeyboardShortcut {
        pub fn register() -> Result<Self, String> {
            let mut handler = std::ptr::null_mut(); let mut key = std::ptr::null_mut();
            let target = unsafe { GetApplicationEventTarget() };
            let event = EventType { class: u32::from_be_bytes(*b"keyb"), kind: 6 };
            let status = unsafe { InstallEventHandler(target, handle, 1, &event, std::ptr::null_mut(), &mut handler) };
            if status != 0 { return Err(format!("event handler error {status}")); }
            let status = unsafe { RegisterEventHotKey(46, (1 << 8) | (1 << 9) | (1 << 11), HotKeyId { signature: SIGNATURE, id: 1 }, target, 0, &mut key) };
            if status != 0 { unsafe { RemoveEventHandler(handler); } return Err(format!("shortcut registration error {status}")); }
            Ok(Self { handler, key })
        }
        pub fn pressed(&self) -> bool { PRESSED.swap(false, Ordering::SeqCst) }
    }
    impl Drop for KeyboardShortcut {
        fn drop(&mut self) { unsafe { UnregisterEventHotKey(self.key); RemoveEventHandler(self.handler); } }
    }
}
#[cfg(target_os = "macos")]
pub use shortcut::KeyboardShortcut;
#[cfg(target_os = "macos")]
static KEY_FOCUS_ALLOWED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(target_os = "macos")]
pub fn key_focus_allowed() -> bool { KEY_FOCUS_ALLOWED.load(std::sync::atomic::Ordering::SeqCst) }
#[cfg(target_os = "macos")]
pub fn allow_key_focus(allowed: bool) { KEY_FOCUS_ALLOWED.store(allowed, std::sync::atomic::Ordering::SeqCst); }
#[cfg(target_os = "macos")]
pub unsafe fn focus_panel(window: *mut std::ffi::c_void) {
    use objc2::{msg_send, runtime::{AnyClass, AnyObject, Bool}};
    let app: *mut AnyObject = msg_send![AnyClass::get("NSApplication").unwrap(), sharedApplication];
    let _: () = msg_send![app, activateIgnoringOtherApps: Bool::YES];
    let _: () = msg_send![window as *mut AnyObject, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
}

#[cfg(target_os = "macos")]
pub fn display_bounds(id: u32) -> (f64, f64, f64, f64) {
    #[repr(C)]
    struct Point { x: f64, y: f64 }
    #[repr(C)]
    struct Size { width: f64, height: f64 }
    #[repr(C)]
    struct Rect { origin: Point, size: Size }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" { fn CGDisplayBounds(display: u32) -> Rect; }
    let rect = unsafe { CGDisplayBounds(id) };
    (rect.origin.x, rect.origin.y, rect.size.width, rect.size.height)
}
