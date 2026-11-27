#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardState {
    pub active: bool,
    pub speaking: bool,
    pub hovered: bool,
    pub text: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub collapse_at: Option<f64>,
}

impl CardState {
    pub fn open(&mut self, text: String, columns: Vec<String>, rows: Vec<Vec<String>>) {
        self.active = true;
        self.hovered = false;
        self.text = text;
        self.columns = columns;
        self.rows = rows;
        self.collapse_at = None;
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.speaking = false;
        self.hovered = false;
        self.text.clear();
        self.columns.clear();
        self.rows.clear();
        self.collapse_at = None;
    }

    pub fn apply_reply(&mut self, value: &serde_json::Value) {
        let (text, columns, rows) = parse_card_payload(value);
        self.open(text, columns, rows);
    }

    pub fn handle_event(
        &mut self,
        value: &serde_json::Value,
        base_rect: (f64, f64, f64, f64),
    ) -> Option<(f64, f64, f64, f64)> {
        let kind = value
            .get("state")
            .or_else(|| value.get("type"))
            .or_else(|| value.get("action"))
            .or_else(|| value.get("event"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match kind {
            "reply_card" => {
                self.apply_reply(value);
                let (w, h) = self.desired_size();
                Some(card_rect(base_rect, w as f64, h as f64))
            }
            "blocked" | "error" | "card_dismiss" | "escape" | "outside_click" => {
                if self.active {
                    self.clear();
                    Some(base_rect)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn desired_size(&self) -> (u32, u32) {
        if !self.active {
            return (160, 56);
        }
        let text_lines = ((self.text.len() as u32) + 41) / 42;
        let body_h = 34 + text_lines.clamp(1, 5) * 18;
        let table_h = if self.rows.is_empty() {
            0
        } else {
            28 + (self.rows.len().min(6) as u32) * 24
        };
        let pill_h = 46;
        (360, (pill_h + body_h + table_h).clamp(130, 320))
    }
}

pub fn parse_card_payload(value: &serde_json::Value) -> (String, Vec<String>, Vec<Vec<String>>) {
    let text = value.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let columns = value
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let rows = value
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|row| row.as_array())
                .map(|cells| cells.iter().map(|c| c.as_str().unwrap_or("").to_string()).collect())
                .collect()
        })
        .unwrap_or_default();
    (text, columns, rows)
}

pub fn card_rect(base_rect: (f64, f64, f64, f64), width: f64, height: f64) -> (f64, f64, f64, f64) {
    let center_x = base_rect.0 + base_rect.2 / 2.0;
    let bottom_top = base_rect.1 + base_rect.3;
    let top = (bottom_top - height).max(16.0);
    let left = (center_x - width / 2.0).max(16.0);
    (left, top, width, height)
}

pub fn merge_html(html: &str, tokens: &str, css: &str, card_css: &str, js: &str) -> String {
    html.replace(
        "<title>Leko Pill</title>",
        &format!("<title>Leko Pill</title><style>{}</style><style>{}</style><style>{}</style>", tokens, css, card_css),
    )
    .replace("</body>", &format!("<script>{}</script></body>", js))
}

#[cfg(target_os = "macos")]
pub unsafe fn resize_panel(
    window: *mut std::ffi::c_void,
    base_rect: (f64, f64, f64, f64),
    width: f64,
    height: f64,
    screen_height: f64,
) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    #[repr(C)]
    struct Point {
        x: f64,
        y: f64,
    }
    #[repr(C)]
    struct Size {
        width: f64,
        height: f64,
    }
    #[repr(C)]
    struct Rect {
        origin: Point,
        size: Size,
    }
    unsafe impl objc2::Encode for Point {
        const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl objc2::Encode for Size {
        const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl objc2::Encode for Rect {
        const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGRect", &[Point::ENCODING, Size::ENCODING]);
    }

    if window.is_null() {
        return;
    }
    let center_x = base_rect.0 + base_rect.2 / 2.0;
    let bottom_top = base_rect.1 + base_rect.3;
    let top = (bottom_top - height).max(16.0);
    let left = (center_x - width / 2.0).max(16.0);
    let frame = Rect {
        origin: Point { x: left, y: screen_height - top - height },
        size: Size { width, height },
    };
    let object = window as *mut AnyObject;
    let _: () = msg_send![object, setFrame: frame display: true animate: false];
}

#[cfg(not(target_os = "macos"))]
pub unsafe fn resize_panel(
    _window: *mut std::ffi::c_void,
    _base_rect: (f64, f64, f64, f64),
    _width: f64,
    _height: f64,
    _screen_height: f64,
) {
}
