pub const COMPACT_W: f64 = 160.0;
pub const COMPACT_H: f64 = 56.0;
pub const EXPANDED_W: f64 = 380.0;
pub const EXPANDED_MIN_H: f64 = 260.0;
pub const CARD_MAX_W: f64 = 348.0;
pub const CARD_H_PAD: f64 = 12.0;
pub const EDGE_PAD: f64 = 16.0;
pub const CAPSULE_INSET: f64 = 12.0;
pub const SLOT_H: f64 = 32.0;

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
        anchor: &str,
        monitor_rect: (f64, f64, f64, f64),
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
                Some(window_frame(anchor, monitor_rect, w as f64, h as f64))
            }
            "blocked" | "error" | "card_dismiss" | "escape" | "outside_click" => {
                if self.active {
                    self.clear();
                    Some(window_frame(anchor, monitor_rect, COMPACT_W, COMPACT_H))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn desired_size(&self) -> (u32, u32) {
        if !self.active {
            return (COMPACT_W as u32, COMPACT_H as u32);
        }
        let text_lines = ((self.text.len() as u32) + 41) / 42;
        let body_h = 34 + text_lines.clamp(1, 6) * 18;
        let table_h = if self.rows.is_empty() {
            0
        } else {
            32 + (self.rows.len().min(6) as u32) * 24
        };
        let total = 56 + body_h + table_h;
        (EXPANDED_W as u32, total.clamp(EXPANDED_MIN_H as u32, 380))
    }
}

pub fn window_frame(
    anchor: &str,
    monitor_rect: (f64, f64, f64, f64),
    width: f64,
    height: f64,
) -> (f64, f64, f64, f64) {
    let (mon_x, mon_y, mon_w, mon_h) = monitor_rect;
    let x = match anchor {
        "bottom-right" | "top-right" => mon_x + mon_w - EDGE_PAD - width,
        "bottom-left" => mon_x + EDGE_PAD,
        _ => mon_x + (mon_w - width) / 2.0,
    };
    let y = match anchor {
        "top-right" => mon_y + EDGE_PAD,
        _ => mon_y + mon_h - EDGE_PAD - height,
    };
    (x, y, width, height)
}

pub fn pill_slot_rect(anchor: &str, win_rect: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let (wx, wy, ww, wh) = win_rect;
    let sx = match anchor {
        "bottom-right" | "top-right" => wx + ww - COMPACT_W,
        "bottom-left" => wx,
        _ => wx + (ww - COMPACT_W) / 2.0,
    };
    let sy = match anchor {
        "top-right" => wy + CAPSULE_INSET,
        _ => wy + wh - CAPSULE_INSET - SLOT_H,
    };
    (sx, sy, COMPACT_W, SLOT_H)
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
    target_rect: (f64, f64, f64, f64),
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
    let (left, top, width, height) = target_rect;
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
    _target_rect: (f64, f64, f64, f64),
    _screen_height: f64,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_and_expanded_keep_exact_pill_anchor_across_all_corners() {
        let monitor = (0.0, 0.0, 1440.0, 900.0);
        let mut card = CardState::default();
        card.open("Reply text".into(), vec!["A".into(), "B".into()], vec![vec!["1".into(), "2".into()]]);
        let (exp_w, exp_h) = card.desired_size();
        assert!(exp_w as f64 >= EXPANDED_W);
        assert!(exp_h as f64 >= EXPANDED_MIN_H);
        assert!((exp_w as f64) > CARD_MAX_W + 2.0 * CARD_H_PAD);

        for anchor in ["bottom-center", "bottom-right", "top-right", "bottom-left"] {
            let compact_win = window_frame(anchor, monitor, COMPACT_W, COMPACT_H);
            let expanded_win = window_frame(anchor, monitor, exp_w as f64, exp_h as f64);
            let compact_slot = pill_slot_rect(anchor, compact_win);
            let expanded_slot = pill_slot_rect(anchor, expanded_win);

            let compact_cx = compact_slot.0 + compact_slot.2 / 2.0;
            let expanded_cx = expanded_slot.0 + expanded_slot.2 / 2.0;
            let compact_by = compact_slot.1 + compact_slot.3;
            let expanded_by = expanded_slot.1 + expanded_slot.3;

            assert!((compact_cx - expanded_cx).abs() < 1e-6, "center X drifted for {}", anchor);
            assert!((compact_by - expanded_by).abs() < 1e-6, "bottom Y drifted for {}", anchor);
        }
    }
}
