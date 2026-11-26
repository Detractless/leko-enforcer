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
pub unsafe fn set_ignores_mouse(window: *mut std::ffi::c_void, ignore: bool) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let object = window as *mut AnyObject;
    let _: () = msg_send![object, setIgnoresMouseEvents: ignore];
}

#[cfg(not(target_os = "macos"))]
pub unsafe fn set_ignores_mouse(_window: *mut std::ffi::c_void, _ignore: bool) {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputEvent {
    Enter,
    Leave,
    None,
}

pub struct NativeInput<P: CursorProvider> {
    pub rect: (f64, f64, f64, f64),
    pub provider: P,
    pub inside: bool,
    pub ignores_mouse: bool,
}

impl<P: CursorProvider> NativeInput<P> {
    pub fn new(rect: (f64, f64, f64, f64), provider: P) -> Self {
        NativeInput { rect, provider, inside: false, ignores_mouse: true }
    }

    pub fn contains(&self, point: CursorPoint) -> bool {
        let (x, y, width, height) = self.rect;
        point.x >= x && point.x < x + width && point.y >= y && point.y < y + height
    }

    pub fn poll(&mut self) -> InputEvent {
        let Some(point) = self.provider.position() else {
            return InputEvent::None;
        };
        let now_inside = self.contains(point);
        if now_inside == self.inside {
            return InputEvent::None;
        }
        self.inside = now_inside;
        self.ignores_mouse = !now_inside;
        if now_inside {
            InputEvent::Enter
        } else {
            InputEvent::Leave
        }
    }

    pub fn cadence_ms(&self) -> u64 {
        cadence(self.inside)
    }

}

pub fn cadence(near: bool) -> u64 {
    if near { 16 } else { 90 }
}

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
}
