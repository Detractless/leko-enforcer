use tao::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Leko")
        .with_inner_size(LogicalSize::new(420.0, 600.0))
        .with_resizable(true)
        .build(&event_loop)?;

    let html_content = include_str!("../web/index.html");
    let css_content = include_str!("../web/circle.css");
    let js_content = include_str!("../web/app.js");

    // Inlined self-contained HTML so the binary has zero external file path dependencies
    let full_html = html_content
        .replace(r#"<link rel="stylesheet" href="circle.css">"#, &format!("<style>{css_content}</style>"))
        .replace(r#"<script src="app.js"></script>"#, &format!("<script>{js_content}</script>"));

    let _webview = WebViewBuilder::new()
        .with_html(full_html)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {},
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
