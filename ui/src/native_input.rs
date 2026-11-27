#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorPoint {
    pub x: f64,
    pub y: f64,
}

pub trait CursorProvider {
    fn position(&self) -> Option<CursorPoint>;
}

#[derive(Debug, Default)]
pub struct TestCursor {
    pub point: Option<CursorPoint>,
}

impl CursorProvider for TestCursor {
    fn position(&self) -> Option<CursorPoint> {
        self.point
    }
}

#[cfg(target_os = "macos")]
pub struct SystemCursor {
    pub screen_height: f64,
}

#[cfg(target_os = "macos")]
impl SystemCursor {
    pub fn new(screen_height: f64) -> Self {
        SystemCursor { screen_height }
    }
}

#[cfg(target_os = "macos")]
impl CursorProvider for SystemCursor {
    fn position(&self) -> Option<CursorPoint> {
        use objc2_app_kit::NSEvent;
        let point = unsafe { NSEvent::mouseLocation() };
        Some(CursorPoint {
            x: point.x,
            y: self.screen_height - point.y,
        })
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default)]
pub struct SystemCursor;

#[cfg(not(target_os = "macos"))]
impl SystemCursor {
    pub fn new(_screen_height: f64) -> Self {
        SystemCursor
    }
}

#[cfg(not(target_os = "macos"))]
impl CursorProvider for SystemCursor {
    fn position(&self) -> Option<CursorPoint> {
        None
    }
}

#[cfg(target_os = "macos")]
pub fn pressed_mouse_buttons() -> usize {
    unsafe { objc2_app_kit::NSEvent::pressedMouseButtons() }
}

#[cfg(not(target_os = "macos"))]
pub fn pressed_mouse_buttons() -> usize { 0 }

#[cfg(target_os = "macos")]
pub unsafe fn set_ignores_mouse(window: *mut std::ffi::c_void, ignore: bool) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let object = window as *mut AnyObject;
    let _: () = msg_send![object, setIgnoresMouseEvents: ignore];
}

#[cfg(not(target_os = "macos"))]
pub unsafe fn set_ignores_mouse(_window: *mut std::ffi::c_void, _ignore: bool) {}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct NativePoint { x: f64, y: f64 }

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct NativeSize { width: f64, height: f64 }

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy)]
struct NativeRect { origin: NativePoint, size: NativeSize }

#[cfg(target_os = "macos")]
unsafe impl objc2::Encode for NativePoint {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}

#[cfg(target_os = "macos")]
unsafe impl objc2::Encode for NativeSize {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}

#[cfg(target_os = "macos")]
unsafe impl objc2::Encode for NativeRect {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGRect", &[NativePoint::ENCODING, NativeSize::ENCODING]);
}

#[cfg(target_os = "macos")]
pub struct NonActivatingPanel {
    pub window: *mut std::ffi::c_void,
    pub content_view: *mut std::ffi::c_void,
}

#[cfg(target_os = "macos")]
impl wry::raw_window_handle::HasWindowHandle for NonActivatingPanel {
    fn window_handle(&self) -> Result<wry::raw_window_handle::WindowHandle<'_>, wry::raw_window_handle::HandleError> {
        let ptr = std::ptr::NonNull::new(self.content_view)
            .ok_or(wry::raw_window_handle::HandleError::Unavailable)?;
        let raw = wry::raw_window_handle::RawWindowHandle::AppKit(
            wry::raw_window_handle::AppKitWindowHandle::new(ptr),
        );
        Ok(unsafe { wry::raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

#[cfg(target_os = "macos")]
fn panel_class() -> &'static objc2::runtime::AnyClass {
    use objc2::declare::ClassBuilder;
    use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
    use objc2::sel;
    use std::sync::OnceLock;

    static PANEL_CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    PANEL_CLASS.get_or_init(|| {
        unsafe extern "C" fn ret_no(_this: *mut AnyObject, _cmd: Sel) -> Bool { Bool::NO }
        let superclass = AnyClass::get("NSPanel").expect("NSPanel class");
        let mut builder = ClassBuilder::new("LekoNonActivatingPanel", superclass)
            .expect("register LekoNonActivatingPanel");
        unsafe {
            builder.add_method(sel!(canBecomeKeyWindow), ret_no as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool);
            builder.add_method(sel!(canBecomeMainWindow), ret_no as unsafe extern "C" fn(*mut AnyObject, Sel) -> Bool);
        }
        builder.register()
    })
}

#[cfg(target_os = "macos")]
pub unsafe fn create_nonactivating_panel(
    x: f64, y: f64, width: f64, height: f64, screen_height: f64,
) -> NonActivatingPanel {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};

    let rect = NativeRect {
        origin: NativePoint { x, y: screen_height - (y + height) },
        size: NativeSize { width, height },
    };
    let cls = panel_class();
    let alloc: *mut AnyObject = msg_send![cls, alloc];
    // NSWindowStyleMaskNonactivatingPanel (1 << 7) | borderless (0) = 128; NSBackingStoreBuffered = 2
    let panel: *mut AnyObject = msg_send![
        alloc, initWithContentRect: rect, styleMask: 128usize, backing: 2usize, defer: Bool::NO
    ];
    let clear: *mut AnyObject = msg_send![AnyClass::get("NSColor").unwrap(), clearColor];
    let _: () = msg_send![panel, setLevel: 25isize];
    let _: () = msg_send![panel, setOpaque: Bool::NO];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: Bool::NO];
    let _: () = msg_send![panel, setHidesOnDeactivate: Bool::NO];
    let _: () = msg_send![panel, setAcceptsMouseMovedEvents: Bool::YES];
    let _: () = msg_send![panel, setIgnoresMouseEvents: Bool::YES];
    // CanJoinAllSpaces (1) | Stationary (16) | IgnoresCycle (64) | FullScreenAuxiliary (256)
    let _: () = msg_send![panel, setCollectionBehavior: 337usize];
    let content_view: *mut AnyObject = msg_send![panel, contentView];
    NonActivatingPanel { window: panel as *mut std::ffi::c_void, content_view: content_view as *mut std::ffi::c_void }
}

#[cfg(target_os = "macos")]
pub unsafe fn order_front_regardless(window: *mut std::ffi::c_void) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let _: () = msg_send![window as *mut AnyObject, orderFrontRegardless];
}

#[cfg(target_os = "macos")]
pub unsafe fn panel_rect(window: *mut std::ffi::c_void, screen_height: f64) -> (f64, f64, f64, f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let frame: NativeRect = msg_send![window as *mut AnyObject, frame];
    (frame.origin.x, screen_height - (frame.origin.y + frame.size.height), frame.size.width, frame.size.height)
}

#[cfg(target_os = "macos")]
pub unsafe fn frontmost_pid() -> i32 {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    let ws: *mut AnyObject = msg_send![AnyClass::get("NSWorkspace").unwrap(), sharedWorkspace];
    let app: *mut AnyObject = msg_send![ws, frontmostApplication];
    if app.is_null() { 0 } else { msg_send![app, processIdentifier] }
}

#[cfg(target_os = "macos")]
pub unsafe fn restore_previous_app(pid: i32) {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    if pid <= 0 || pid == std::process::id() as i32 {
        return;
    }
    let app: *mut AnyObject = msg_send![AnyClass::get("NSRunningApplication").unwrap(), runningApplicationWithProcessIdentifier: pid];
    if !app.is_null() {
        let _: Bool = msg_send![app, activateWithOptions: 1usize];
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent { Enter, Leave, None }

pub struct NativeInput<P: CursorProvider> {
    pub rect: (f64, f64, f64, f64),
    pub pill_rect: (f64, f64, f64, f64),
    pub card_active: bool,
    pub drag_active: bool,
    pub provider: P,
    pub inside: bool,
    pub near: bool,
    pub ignores_mouse: bool,
}

impl<P: CursorProvider> NativeInput<P> {
    pub fn new(rect: (f64, f64, f64, f64), provider: P) -> Self {
        NativeInput {
            rect,
            pill_rect: rect,
            card_active: false,
            drag_active: false,
            provider,
            inside: false,
            near: false,
            ignores_mouse: true,
        }
    }

    pub fn contains_rect(r: (f64, f64, f64, f64), point: CursorPoint) -> bool {
        let (x, y, w, h) = r;
        point.x >= x && point.x < x + w && point.y >= y && point.y < y + h
    }

    pub fn contains(&self, point: CursorPoint) -> bool {
        Self::contains_rect(self.pill_rect, point)
    }

    pub fn is_near(&self, point: CursorPoint) -> bool {
        let (x, y, w, h) = if self.card_active { self.rect } else { self.pill_rect };
        let pad = 64.0;
        point.x >= x - pad && point.x < x + w + pad && point.y >= y - pad && point.y < y + h + pad
    }

    pub fn interactive_at(&self, point: CursorPoint) -> bool {
        if self.drag_active || self.contains(point) {
            return true;
        }
        if !self.card_active || !Self::contains_rect(self.rect, point) {
            return false;
        }
        let in_pill_band = point.y >= self.pill_rect.1 && point.y < self.pill_rect.1 + self.pill_rect.3;
        !in_pill_band
    }

    pub fn poll(&mut self) -> InputEvent {
        let Some(point) = self.provider.position() else {
            return InputEvent::None;
        };
        self.near = self.is_near(point);
        let now_inside = self.contains(point);
        self.ignores_mouse = !self.interactive_at(point);
        if now_inside == self.inside {
            return InputEvent::None;
        }
        self.inside = now_inside;
        if now_inside { InputEvent::Enter } else { InputEvent::Leave }
    }

    pub fn update_drag(&mut self, pressed: bool) -> bool {
        if pressed && self.inside {
            self.drag_active = true;
            self.ignores_mouse = false;
            return false;
        }
        if self.drag_active && !pressed {
            self.drag_active = false;
            let interactive = self.provider.position().is_some_and(|p| self.interactive_at(p));
            self.ignores_mouse = !interactive;
            return true;
        }
        false
    }

    pub fn cadence_ms(&self) -> u64 { cadence(self.near) }
}

pub fn cadence(near: bool) -> u64 { if near { 16 } else { 90 } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injectable_provider_drives_hover() {
        let cursor = TestCursor::default();
        let mut input = NativeInput::new((0.0, 0.0, 160.0, 56.0), cursor);
        assert_eq!(input.poll(), InputEvent::None);
        input.provider.point = Some(CursorPoint { x: 80.0, y: 28.0 });
        assert_eq!(input.poll(), InputEvent::Enter);
        assert!(input.inside);
        assert!(!input.ignores_mouse);
        assert_eq!(input.cadence_ms(), 16);
    }

    #[test]
    fn leaving_restores_click_through() {
        let cursor = TestCursor { point: Some(CursorPoint { x: 20.0, y: 20.0 }) };
        let mut input = NativeInput::new((0.0, 0.0, 160.0, 56.0), cursor);
        assert_eq!(input.poll(), InputEvent::Enter);
        input.provider.point = Some(CursorPoint { x: 500.0, y: 500.0 });
        assert_eq!(input.poll(), InputEvent::Leave);
        assert!(input.ignores_mouse);
        assert_eq!(input.cadence_ms(), 90);
    }

    #[test]
    fn drag_keeps_mouse_active_until_release_and_uses_pill_rect() {
        let mut input = NativeInput::new((100.0, 100.0, 380.0, 260.0), TestCursor::default());
        input.pill_rect = (210.0, 316.0, 160.0, 32.0);
        input.card_active = true;
        input.provider.point = Some(CursorPoint { x: 290.0, y: 330.0 });
        assert_eq!(input.poll(), InputEvent::Enter);
        assert!(!input.update_drag(true));
        input.provider.point = Some(CursorPoint { x: 800.0, y: 800.0 });
        assert_eq!(input.poll(), InputEvent::Leave);
        assert!(!input.ignores_mouse);
        assert!(input.update_drag(false));
        assert!(input.ignores_mouse);
    }
}
