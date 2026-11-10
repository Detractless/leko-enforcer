use std::io::{BufRead, Write};
use std::thread;
use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Leko")
        .with_inner_size(LogicalSize::new(420.0, 400.0))
        .with_min_inner_size(LogicalSize::new(320.0, 380.0))
        .with_resizable(true)
        .build(&event_loop)?;

    let html_content = include_str!("../web/index.html");
    let token_content = include_str!("../web/styles/tokens.css");
    let css_content = include_str!("../web/styles/circle.css");
    let js_content = include_str!("../web/app.js");
    let controller_content = include_str!("../web/circle-controller.js");

    // Inlined self-contained HTML so binary has zero external file dependencies
    let full_html = html_content
        .replace(r#"<link rel="stylesheet" href="styles/tokens.css">"#, &format!("<style>{token_content}</style>"))
        .replace(r#"<link rel="stylesheet" href="styles/circle.css">"#, &format!("<style>{css_content}</style>"))
        .replace(r#"<script src="app.js"></script>"#, &format!("<script>{js_content}</script>"))
        .replace(r#"<script type="module" src="circle-controller.js"></script>"#, &format!(r#"<script type="module">{controller_content}</script>"#));

    let webview = WebViewBuilder::new()
        .with_html(full_html)
        .with_ipc_handler(move |request| {
            println!("{}", request.body());
            let _ = std::io::stdout().flush();
        })
        .build(&window)?;

    // Background thread reading JSON event lines from stdin
    thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(line) = line {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    let _ = proxy.send_event(trimmed.to_string());
                }
            }
        }
    });

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {},
            Event::UserEvent(json_line) => {
                let script = format!("window.__leko_on_event && window.__leko_on_event({});", json_line);
                let _ = webview.evaluate_script(&script);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
