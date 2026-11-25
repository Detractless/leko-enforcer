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
            "height": snapshot.height,
            "timer": snapshot.timer,
            "bars": snapshot.bars,
            "dash": snapshot.dash,
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

    let input = FileHandle.standardInput
    Thread.detachNewThread {
        var buffer = ""
        while true {
            let data = input.availableData
            if data.isEmpty {
                break
            }
            buffer += String(data: data, encoding: .utf8) ?? ""
            while let newline = buffer.firstIndex(of: "\n") {
                let line = String(buffer[buffer.startIndex..<newline])
                buffer.removeSubrange(buffer.startIndex...newline)
                guard let object = try? JSONSerialization.jsonObject(with: Data(line.utf8))
                        as? [String: Any] else {
                    continue
                }
                let state = object["state"] as? String ?? object["type"] as? String ?? ""
                DispatchQueue.main.async {
                    controller.applyExternal(state: state)
                }
            }
        }
    }

    Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { _ in
        controller.tick()
    }
    NSApplication.shared.run()
}
