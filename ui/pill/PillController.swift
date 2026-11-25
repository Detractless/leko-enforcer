import Foundation

final class PillController {
    private var machine: PillStateMachine
    private let panel: PillPanel
    private let config: PillConfig
    private var lastEvent = ""
    private var localResponse: DispatchWorkItem?

    init(config: PillConfig) {
        self.config = config
        machine = PillStateMachine(quietTimer: config.quietTimer)
        panel = PillPanel(snapshot: machine.snapshot(), config: config)
        panel.onPress = { [weak self] down in
            guard let self, down else { return }
            self.localResponse?.cancel()
            self.applyEvent("ptt_start")
            EventBridge.writeAction("ptt_start")
        }
        panel.onRelease = { [weak self] held in
            guard let self else { return }
            EventBridge.writeAction("ptt_end")
            if held <= 0.2 {
                self.applyEvent("idle")
            } else {
                self.scheduleStandaloneResponse()
            }
        }
        panel.onHover = { [weak self] hovering in
            guard let self else { return }
            self.applyEvent(hovering ? "hover_on" : "hover_off")
            self.panel.setHoverFill(hovering)
        }
    }

    func applyEvent(_ event: String, duration: TimeInterval = 60) {
        let snapshot = machine.apply(event: event, now: Date().timeIntervalSince1970,
                                     duration: duration)
        panel.apply(snapshot)
    }

    func applyExternal(state: String) {
        localResponse?.cancel()
        if state == "blocked" {
            applyEvent("blocked")
            return
        }
        let mapped: String
        switch state {
        case "listening", "held", "thinking", "speaking", "error", "idle", "loading":
            mapped = state
        default:
            mapped = "idle"
        }
        applyEvent(mapped)
    }

    private func scheduleStandaloneResponse() {
        localResponse?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self else { return }
            self.applyEvent("speaking")
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { [weak self] in
                self?.applyEvent("idle")
            }
        }
        localResponse = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: work)
    }

    func tick() {
        for event in EventBridge.readEvents() where event != lastEvent {
            lastEvent = event
            applyEvent("blocked")
        }
        if config.quietTimer {
            let text = EventBridge.readPassTimer()
            let seconds = PillController.seconds(from: text)
            if seconds > 0 && machine.timer != text {
                applyEvent("timer_start", duration: seconds)
            } else if seconds == 0 && !machine.timer.isEmpty {
                applyEvent("timer_stop")
            }
        }
        panel.animate(Date().timeIntervalSince1970)
    }

    private static func seconds(from text: String) -> TimeInterval {
        let parts = text.split(separator: ":")
        guard parts.count == 2,
              let minutes = Double(parts[0]),
              let seconds = Double(parts[1]) else {
            return 0
        }
        return minutes * 60 + seconds
    }
}
