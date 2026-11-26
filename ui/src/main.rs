mod native_input;
mod state;
mod timer_poll;

use native_input::{
    set_ignores_mouse, CursorPoint, InputEvent, NativeInput, SystemCursor, TestCursor,
};
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
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

const HTML: &str = include_str!("web/pill.html");
const CSS: &str = include_str!("web/pill.css");
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
    let dir = app_support();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("pill-actions.json");
    let body = format!("{{\"action\":\"{}\"}}", action);
    let temp = dir.join(".pill-actions.tmp");
    let _ = fs::write(&temp, &body);
    let _ = fs::rename(temp, &path);
    println!("{}", body);
    let _ = io::stdout().flush();
}

fn headless() {
    let quiet = std::env::args().any(|arg| arg == "--quiet-timer");
    let mut machine = Machine::new(quiet);
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue };
        let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        let Some(event) = value.get("event").and_then(|v| v.as_str()) else { continue };
        let now = value.get("now_ms").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0;
        let duration = value.get("duration_ms").and_then(|v| v.as_f64()).unwrap_or(0.0) / 1000.0;
        let snap = machine.apply(event, now, duration);
        let out = serde_json::json!({
            "state": snap.state, "text": snap.text, "width": snap.width,
            "height": snap.height, "timer": snap.timer, "bars": snap.bars,
            "dash": snap.dash,
        });
        println!("{}", out);
    }
}

fn timer_probe() {
    let mut was_active = false;
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue };
        let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        let Some(dir) = value.get("dir").and_then(|v| v.as_str()) else { continue };
        let dir_path = PathBuf::from(dir);
        if let Some(events) = value.get("events") {
            let _ = fs::write(dir_path.join("pill-events.json"), events.to_string());
        }
        let now = value.get("now_epoch").and_then(|v| v.as_f64()).unwrap_or_else(now_epoch_seconds);
        let (_, _, quiet) = read_config(&dir_path);
        let (update, timer) = match poll_step(quiet, &mut was_active, &dir_path, now) {
            Some(TimerUpdate::Start(text)) => ("timer_start", text),
            Some(TimerUpdate::Stop) => ("timer_stop", String::new()),
            None => ("none", String::new()),
        };
        let out = serde_json::json!({"quiet": quiet, "update": update, "timer": timer});
        println!("{}", out);
    }
}

fn input_probe() {
    let mut input = NativeInput::new((100.0, 100.0, 160.0, 56.0), TestCursor::default());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { continue };
        let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        input.provider.point = value.get("x").and_then(|v| v.as_f64())
            .zip(value.get("y").and_then(|v| v.as_f64()))
            .map(|(x, y)| CursorPoint { x, y });
        let event = input.poll();
        let name = match event {
            InputEvent::Enter => "enter",
            InputEvent::Leave => "leave",
            InputEvent::None => "none",
        };
        let out = serde_json::json!({
            "event": name,
            "inside": input.inside,
            "ignore_mouse": input.ignores_mouse,
            "cadence_ms": input.cadence_ms(),
        });
        println!("{}", out);
    }
}

fn anchor_position(anchor: &str, monitor: &tao::monitor::MonitorHandle) -> tao::dpi::LogicalPosition<i32> {
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let width = 160.0;
    let height = 56.0;
    let left = monitor.position().x as f64 / scale;
    let top = monitor.position().y as f64 / scale;
    let max_x = left + size.width as f64 / scale - width;
    let max_y = top + size.height as f64 / scale - height;
    let pad = 16.0;
    let (x, y) = match anchor {
        "bottom-right" => (max_x - pad, max_y - pad),
        "top-right" => (max_x - pad, top + pad),
        "bottom-left" => (left + pad, max_y - pad),
        _ => (left + (size.width as f64 / scale - width) / 2.0, max_y - pad),
    };
    tao::dpi::LogicalPosition::new(x as i32, y as i32)
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

    let (theme, anchor, quiet) = read_config(&app_support());
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<Message>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_activate_ignoring_other_apps(false);
    }
    let proxy = event_loop.create_proxy();
    let monitor = event_loop.primary_monitor();
    let position = monitor.as_ref().map(|m| anchor_position(&anchor, m));
    let screen_height = monitor
        .as_ref()
        .map(|item| item.size().height as f64 / item.scale_factor())
        .unwrap_or(900.0);

    let html = HTML
        .replace("<title>Leko Pill</title>",
            &format!("<title>Leko Pill</title><style>{}</style><style>{}</style>", TOKENS, CSS))
        .replace("</body>", &format!("<script>{}</script></body>", JS));
    let handler_proxy = proxy.clone();
    let wv_builder = WebViewBuilder::new()
        .with_transparent(true)
        .with_html(&html)
        .with_initialization_script(&format!(
            "document.body.dataset.theme='{}';window.__leko_config={{quietTimer:{}}};",
            theme, quiet
        ))
        .with_ipc_handler(move |request| {
            let raw = request.body();
            let first: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
            let value = first.as_str()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .unwrap_or(first);
            if let Some(action) = value.get("action").and_then(|v| v.as_str()) {
                write_action(action);
                let _ = handler_proxy.send_event(Message::Event(action.to_string()));
            }
        });

    #[cfg(target_os = "macos")]
    let (native_window, webview, rect) = {
        let prev_pid = unsafe { frontmost_pid() };
        let (x, y) = position.map(|p| (p.x as f64, p.y as f64)).unwrap_or((0.0, 0.0));
        let panel = unsafe { create_nonactivating_panel(x, y, 160.0, 56.0, screen_height) };
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
        let mut builder = WindowBuilder::new()
            .with_title("Leko Pill")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top(true)
            .with_resizable(false)
            .with_focused(false)
            .with_inner_size(tao::dpi::LogicalSize::new(160.0, 56.0));
        if let Some(pos) = position {
            builder = builder.with_position(pos);
        }
        let window = builder.build(&event_loop).expect("build pill window");
        let webview = wv_builder.build(&window).expect("build webview");
        let rect = position
            .map(|point| (point.x as f64, point.y as f64, 160.0, 56.0))
            .unwrap_or((0.0, 0.0, 160.0, 56.0));
        (0usize, webview, rect)
    };
    #[cfg(not(target_os = "macos"))]
    let _window_keep = native_window;

    let mut input = NativeInput::new(rect, SystemCursor::new(screen_height));
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
            let path = dir.join("pill-events.json");
            if let Ok(text) = fs::read_to_string(&path) {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                if value.get("event").and_then(|v| v.as_str()) == Some("blocked") {
                    let _ = poll_proxy.send_event(Message::Event(
                        "{\"state\":\"blocked\"}".to_string()
                    ));
                }
            }
            let (_, _, quiet_now) = read_config(&dir);
            match poll_step(quiet_now, &mut timer_active, &dir, now_epoch_seconds()) {
                Some(TimerUpdate::Start(text)) => {
                    let _ = poll_proxy.send_event(Message::Timer(text));
                }
                Some(TimerUpdate::Stop) => {
                    let _ = poll_proxy.send_event(Message::TimerStop);
                }
                None => {}
            }
            thread::sleep(Duration::from_millis(250));
        }
    });

    unsafe { set_ignores_mouse(native_window as *mut std::ffi::c_void, true); }
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::UserEvent(Message::PollTick) = &event {
            let event_kind = input.poll();
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
            return;
        }
        match event {
            Event::UserEvent(Message::Event(raw)) => {
                let _ = webview.evaluate_script(&format!("window.__leko_on_event({});", raw));
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
