import Foundation
import AppKit

let arguments = CommandLine.arguments

if arguments.contains("--headless") {
    var machine = PillStateMachine()
    while let line = readLine() {
        guard let data = line.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let event = object["event"] as? String else {
            continue
        }
        let now = (object["now_ms"] as? Double ?? 0) / 1000
        let duration = (object["duration_ms"] as? Double ?? 0) / 1000
        let snapshot = machine.apply(event: event, now: now, duration: duration)
        let reply: [String: Any] = [
            "state": snapshot.state,
            "text": snapshot.text,
            "width": snapshot.width,
            "timer": snapshot.timer,
        ]
        guard let output = try? JSONSerialization.data(withJSONObject: reply),
              let text = String(data: output, encoding: .utf8) else {
            continue
        }
        print(text)
    }
} else {
    NSApplication.shared.setActivationPolicy(.accessory)
    let config = EventBridge.loadConfig()
    let controller = PillController(config: config)
    Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in
        controller.tick()
    }
    NSApplication.shared.run()
}
