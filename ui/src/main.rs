mod card; mod native_input; mod move_mode; mod state; mod timer_poll;
use card::{merge_html, pill_slot_rect, resize_panel, window_frame, CardState, COMPACT_H, COMPACT_W};
use native_input::{
    pressed_mouse_buttons, set_ignores_mouse, CursorPoint, InputEvent, NativeInput, SystemCursor,
    TestCursor, CursorProvider, };
#[cfg(target_os = "macos")]
use native_input::{
    create_nonactivating_panel, frontmost_pid, order_front_regardless, panel_rect,
    restore_previous_app,
};
use state::Machine;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(not(target_os = "macos"))]
use tao::window::WindowBuilder;
use timer_poll::{now_epoch_seconds, poll_step, read_config, TimerUpdate};
use wry::WebViewBuilder;
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS, MonitorHandleExtMacOS};
const HTML: &str = include_str!("web/pill.html");
const CSS: &str = include_str!("web/pill.css");
const CARD_CSS: &str = include_str!("web/card.css");
const JS: &str = include_str!("web/pill.js");
const TOKENS: &str = include_str!("web/tokens.css");
enum Message {
    Event(String),
    Timer(String),
    TimerStop,
    PollTick,
}
fn app_support() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Library/Application Support/Leko")
}
fn write_action(action: &str) {
    let dir = app_support(); let _ = fs::create_dir_all(&dir); let path = dir.join("pill-actions.json"); let body = format!("{{\"action\":\"{}\"}}", action); let temp = dir.join(".pill-actions.tmp"); let _ = fs::write(&temp, &body); let _ = fs::rename(temp, &path);
    println!("{}", body);
    let _ = io::stdout().flush();
}
fn headless() {
    let quiet = std::env::args().any(|arg| arg == "--quiet-timer"); let mut machine = Machine::new(quiet);
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue }; let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if value.get("event").and_then(|v| v.as_str()).is_none() {
            continue;
        }
        let now = value.get("now_ms").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0; let duration = value.get("duration_ms").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0; let snap = machine.apply_value(&value, now, duration);
        let out = serde_json::json!({
            "state": snap.state, "text": snap.text, "width": snap.width,
            "height": snap.height, "timer": snap.timer, "bars": snap.bars,
            "dash": snap.dash, "card_active": snap.card_active, "card_text": snap.card_text,
            "card_columns": snap.card_columns, "card_rows": snap.card_rows,
            "card_width": snap.card_width, "card_height": snap.card_height,
        });
        println!("{}", out);
    }
}
fn timer_probe() {
    let mut was_active = false;
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue }; let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default(); let Some(dir) = value.get("dir").and_then(|v| v.as_str()) else { continue }; let dir_path = PathBuf::from(dir);
        if let Some(events) = value.get("events") {
            let _ = fs::write(dir_path.join("pill-events.json"), events.to_string());
        }
        let now = value.get("now_epoch").and_then(|v| v.as_f64()).unwrap_or_else(now_epoch_seconds); let (_, _, quiet) = read_config(&dir_path);
        let (update, timer) = match poll_step(quiet, &mut was_active, &dir_path, now) {
            Some(TimerUpdate::Start(text)) => ("timer_start", text),
            Some(TimerUpdate::Stop) => ("timer_stop", String::new()),
            None => ("none", String::new()),
        };
        println!("{}", serde_json::json!({"quiet": quiet, "update": update, "timer": timer}));
    }
}
fn input_probe() {
    let mut input = NativeInput::new((100.0, 100.0, 160.0, 56.0), TestCursor::default());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue }; let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        input.provider.point = value.get("x").and_then(|v| v.as_f64())
            .zip(value.get("y").and_then(|v| v.as_f64()))
            .map(|(x, y)| CursorPoint { x, y });
        let name = match input.poll() {
            InputEvent::Enter => "enter",
            InputEvent::Leave => "leave",
            InputEvent::None => "none",
        };
        println!("{}", serde_json::json!({
            "event": name,
            "inside": input.inside,
            "ignore_mouse": input.ignores_mouse,
            "cadence_ms": input.cadence_ms(),
        }));
    }
}
fn monitor_rect_logical(monitor: Option<&tao::monitor::MonitorHandle>) -> (f64, f64, f64, f64) {
    let Some(m) = monitor else {
        return (0.0, 0.0, 1440.0, 900.0);
    };
    #[cfg(target_os = "macos")] { return move_mode::display_bounds(m.native_id()); }
    #[cfg(not(target_os = "macos"))] { let scale = m.scale_factor(); let pos = m.position(); let size = m.size(); (pos.x as f64 / scale, pos.y as f64 / scale, size.width as f64 / scale, size.height as f64 / scale) }
}
fn main() {
    if std::env::args().any(|arg| arg == "--headless") {
        headless();
        return;
    }
    if std::env::args().any(|arg| arg == "--input-probe") {
        input_probe();
        return;
    }
    if std::env::args().any(|arg| arg == "--timer-probe") {
        timer_probe();
        return;
    }
    let (theme, mut anchor, quiet) = read_config(&app_support());
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_activate_ignoring_other_apps(false);
    }
    let proxy = event_loop.create_proxy(); let monitor = event_loop.primary_monitor(); let monitor_rect = monitor_rect_logical(monitor.as_ref()); let screen_height = monitor_rect.3; let position_path = app_support().join("pill-position.json"); let screens: Vec<_> = event_loop.available_monitors().map(|m| monitor_rect_logical(Some(&m))).collect(); let initial_rect = move_mode::restore_displays(&position_path, window_frame(&anchor, monitor_rect, COMPACT_W, COMPACT_H), &screens); let html = merge_html(HTML, TOKENS, CSS, CARD_CSS, JS); let handler_proxy = proxy.clone();
    let wv_builder = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(&html)
        .with_initialization_script(&format!(
            "document.body.dataset.theme='{}';document.body.dataset.anchor='{}';window.__leko_config={{quietTimer:{}}};",
            theme, anchor, quiet
        ))
        .with_ipc_handler(move |request| {
            let raw = request.body(); let first: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
            let value = first.as_str()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .unwrap_or(first);
            if let Some(action) = value.get("action").and_then(|v| v.as_str()) {
                if !action.starts_with("card_") && !action.starts_with("move_") {
                    write_action(action);
                }
                let _ = handler_proxy.send_event(Message::Event(value.to_string()));
            }
        });
    #[cfg(target_os = "macos")]
    let (native_window, webview, rect) = {
        let prev_pid = unsafe { frontmost_pid() };
        let panel = unsafe {
            create_nonactivating_panel(initial_rect.0, initial_rect.1, COMPACT_W, COMPACT_H, screen_height)
        };
        let webview = wv_builder
            .with_accept_first_mouse(true)
            .build(&panel)
            .expect("build webview");
        unsafe {
            order_front_regardless(panel.window);
            restore_previous_app(prev_pid);
        }
        let rect = unsafe { panel_rect(panel.window, screen_height) };
        (panel.window as usize, webview, rect)
    };
    #[cfg(not(target_os = "macos"))]
    let (native_window, webview, rect) = {
        let window = WindowBuilder::new()
            .with_title("Leko Pill")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_resizable(false)
            .with_focused(false)
            .with_inner_size(tao::dpi::LogicalSize::new(COMPACT_W, COMPACT_H))
            .with_position(tao::dpi::LogicalPosition::new(initial_rect.0, initial_rect.1))
            .build(&event_loop)
            .expect("build pill window");
        let webview = wv_builder.build(&window).expect("build webview");
        (0usize, webview, initial_rect)
    };
    #[cfg(not(target_os = "macos"))]
    let _window_keep = native_window;
    let mut card_state = CardState::default();
    let mut input = NativeInput::new(rect, SystemCursor::new(screen_height));
    input.pill_rect = pill_slot_rect(&anchor, rect);
    let delay = Arc::new(AtomicU64::new(input.cadence_ms()));
    let cursor_proxy = proxy.clone();
    let cursor_delay = delay.clone();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(cursor_delay.load(Ordering::Relaxed)));
        let _ = cursor_proxy.send_event(Message::PollTick);
    });
    let input_proxy = proxy.clone();
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { continue };
            let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
            let _ = input_proxy.send_event(Message::Event(value.to_string()));
        }
    });
    let poll_proxy = proxy.clone();
    thread::spawn(move || {
        let mut timer_active = false;
        loop {
            let dir = app_support();
            if let Ok(text) = fs::read_to_string(dir.join("pill-events.json")) {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if value.get("event").and_then(|v| v.as_str()) == Some("blocked") {
                    let _ = poll_proxy.send_event(Message::Event("{\"state\":\"blocked\"}".to_string()));
                }
            }
            let (_, _, quiet_now) = read_config(&dir);
            match poll_step(quiet_now, &mut timer_active, &dir, now_epoch_seconds()) {
                Some(TimerUpdate::Start(text)) => { let _ = poll_proxy.send_event(Message::Timer(text)); }
                Some(TimerUpdate::Stop) => { let _ = poll_proxy.send_event(Message::TimerStop); }
                None => {}
            }
            thread::sleep(Duration::from_millis(250));
        }
    });
    #[cfg(target_os = "macos")]
    let shortcut = move_mode::KeyboardShortcut::register().map_err(|e| eprintln!("Move shortcut unavailable: {e}")).ok();
    #[cfg(target_os = "macos")]
    let mut keyboard_start: Option<((f64, f64, f64, f64), i32)> = None;
    let mut moving = false;
    let mut move_origin: Option<(CursorPoint, (f64, f64, f64, f64))> = None;
    let mut last_card_cursor: Option<(i32, i32)> = None;
    unsafe { set_ignores_mouse(native_window as *mut std::ffi::c_void, true); }
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::UserEvent(Message::PollTick) = &event {
            #[cfg(target_os = "macos")]
            if shortcut.as_ref().is_some_and(|key| key.pressed()) { let _ = webview.evaluate_script("window.__leko_keyboard_move();"); }
            let screens: Vec<_> = target.available_monitors().map(|m| monitor_rect_logical(Some(&m))).collect();
            if move_origin.is_none() { let slot = move_mode::clamp_displays(input.pill_rect, &screens); let next = (input.rect.0 + slot.0 - input.pill_rect.0, input.rect.1 + slot.1 - input.pill_rect.1, input.rect.2, input.rect.3); if next != input.rect { unsafe { resize_panel(native_window as *mut std::ffi::c_void, next, screen_height); } input.rect = next; input.pill_rect = pill_slot_rect(&anchor, next); } }
            if let Some((start, rect)) = move_origin {
                if let Some(cursor) = input.provider.position() {
                    let next = move_mode::clamp_displays((rect.0 + cursor.x - start.x, rect.1 + cursor.y - start.y, rect.2, rect.3), &screens);
                    unsafe { resize_panel(native_window as *mut std::ffi::c_void, next, screen_height); }
                    input.rect = next;
                    input.pill_rect = pill_slot_rect(&anchor, next);
                }
            }
            let event_kind = input.poll();
            let left_down = (pressed_mouse_buttons() & 1) != 0;
            let drag_released = input.update_drag(left_down); if !left_down { move_origin = None; }
            let outside_clicked = input.poll_outside_click(left_down);
            let pointer = native_window as *mut std::ffi::c_void;
            unsafe { set_ignores_mouse(pointer, input.ignores_mouse); }
            delay.store(input.cadence_ms(), Ordering::Relaxed);
            let script = match event_kind {
                InputEvent::Enter => "window.__leko_on_event({state:'hover_on'});",
                InputEvent::Leave => "window.__leko_on_event({state:'hover_off'});",
                InputEvent::None => "",
            };
            if !script.is_empty() {
                let _ = webview.evaluate_script(script);
            }
            let next_cursor = input.local_cursor().map(|(x, y)| (x.round() as i32, y.round() as i32));
            if next_cursor != last_card_cursor {
                last_card_cursor = next_cursor;
                let (cx, cy) = next_cursor.unwrap_or((-1, -1));
                let _ = webview.evaluate_script(&format!(
                    "window.__leko_on_event({{state:'card_cursor',x:{},y:{}}});", cx, cy
                ));
            }
            if drag_released {
                let _ = webview.evaluate_script("window.__leko_on_event({state:'force_release'});");
            }
            if outside_clicked {
                let _ = webview.evaluate_script("window.__leko_on_event({state:'outside_click'});");
            }
            return;
        }
        match event {
            Event::UserEvent(Message::Event(raw)) => {
                let value: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
                if let Some(action) = value.get("action").and_then(|v| v.as_str()) {
                    #[cfg(target_os = "macos")]
                    if action == "move_keyboard_start" {
                        moving = true;
                        keyboard_start = Some((input.rect, unsafe { frontmost_pid() }));
                        move_mode::allow_key_focus(true); unsafe { move_mode::focus_panel(native_window as *mut std::ffi::c_void); }
                        let _ = webview.evaluate_script("document.getElementById(\"pill\").focus();");
                    } else if let Some(key) = action.strip_prefix("move_key_") {
                        if let Some((start, pid)) = keyboard_start {
                            let (dx, dy) = match key { "left" => (-10.0, 0.0), "right" => (10.0, 0.0), "up" => (0.0, -10.0), "down" => (0.0, 10.0), _ => (0.0, 0.0) };
                            let rect = if key == "cancel" || key == "interrupt" { start } else { (input.rect.0 + dx, input.rect.1 + dy, input.rect.2, input.rect.3) };
                            let screens: Vec<_> = target.available_monitors().map(|m| monitor_rect_logical(Some(&m))).collect(); let rect = move_mode::clamp_displays(rect, &screens);
                            unsafe { resize_panel(native_window as *mut std::ffi::c_void, rect, screen_height); }
                            input.rect = rect; input.pill_rect = pill_slot_rect(&anchor, rect);
                            if key == "cancel" || key == "commit" || key == "interrupt" {
                                if key == "commit" { let _ = move_mode::save(&position_path, (input.pill_rect.0, input.pill_rect.1 - 12.0, COMPACT_W, COMPACT_H)); }
                                keyboard_start = None; moving = false; move_mode::allow_key_focus(false); if key != "interrupt" { unsafe { restore_previous_app(pid); } }
                            }
                        }
                    }
                    if action == "move_toggle" {
                        moving = !moving;
                        if !moving { move_origin = None; let _ = move_mode::save(&position_path, (input.pill_rect.0, input.pill_rect.1 - 12.0, COMPACT_W, COMPACT_H)); }
                    } else if action == "move_start" && moving {
                        move_origin = input.provider.position().map(|point| (point, input.rect));
                    } else if action == "move_end" { move_origin = None; }
                    if action == "ptt_start" {
                        input.drag_active = true;
                        input.ignores_mouse = false;
                        unsafe { set_ignores_mouse(native_window as *mut std::ffi::c_void, false); }
                    } else if action == "ptt_end" || action == "ptt_cancel" {
                        input.drag_active = false;
                    }
                }
                if let Some(frame) = card_state.handle_event(&value, &anchor, monitor_rect) {
                    let screens: Vec<_> = target.available_monitors().map(|m| monitor_rect_logical(Some(&m))).collect(); let screen = screens.iter().min_by(|a,b| { let dist = |r: &&(f64,f64,f64,f64)| { let c = move_mode::clamp_displays(input.rect, &[**r]); (c.0-input.rect.0).powi(2)+(c.1-input.rect.1).powi(2) }; dist(a).total_cmp(&dist(b)) }).copied().unwrap_or(monitor_rect); let (new_rect, next_anchor) = move_mode::card_frame(input.pill_rect, frame, screen); anchor = next_anchor.to_string(); let _ = webview.evaluate_script(&format!("document.body.dataset.anchor=\"{}\";", anchor));
                    let ptr = native_window as *mut std::ffi::c_void;
                    unsafe { resize_panel(ptr, new_rect, screen_height); }
                    let _ = webview.set_bounds(wry::Rect {
                        position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                        size: wry::dpi::LogicalSize::new(new_rect.2, new_rect.3).into(),
                    });
                    input.rect = new_rect;
                    input.pill_rect = pill_slot_rect(&anchor, new_rect);
                    input.card_active = card_state.active;
                    let _ = input.poll();
                    unsafe { set_ignores_mouse(ptr, input.ignores_mouse); }
                    delay.store(input.cadence_ms(), Ordering::Relaxed);
                }
                if value.get("action").is_none() {
                    let _ = webview.evaluate_script(&format!("window.__leko_on_event({});", raw));
                }
            }
            Event::UserEvent(Message::Timer(text)) => {
                let _ = webview.evaluate_script(&format!(
                    "window.__leko_on_event({{state:'timer_start',timer:'{}'}});", text
                ));
            }
            Event::UserEvent(Message::TimerStop) => {
                let _ = webview.evaluate_script("window.__leko_on_event({state:'timer_stop'});");
            }
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
