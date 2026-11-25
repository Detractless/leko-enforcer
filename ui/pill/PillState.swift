import Foundation

enum PillState: String {
    case idle
    case timer
    case blocked
    case listening
    case held
    case thinking
    case speaking
    case errorState
}

struct PillSnapshot: Codable {
    let state: String
    let text: String
    let width: Int
    let height: Int
    let timer: String
    let bars: Int
    let dash: Bool
}

struct PillStateMachine {
    var state: PillState = .idle
    var timer: String = ""
    var blockedUntil: TimeInterval = 0
    var quietTimer: Bool
    var hovering: Bool = false

    init(quietTimer: Bool = false) {
        self.quietTimer = quietTimer
    }

    mutating func apply(event: String, now: TimeInterval, duration: TimeInterval) -> PillSnapshot {
        switch event {
        case "blocked":
            state = .blocked
            blockedUntil = now + 2.2
        case "ptt_start":
            state = .held
        case "ptt_end":
            state = .listening
        case "speaking":
            state = .speaking
        case "thinking":
            state = .thinking
        case "error":
            state = .errorState
        case "idle":
            state = restState()
        case "timer_start":
            timer = PillStateMachine.clock(duration)
            if quietTimer {
                state = .timer
            }
        case "timer_stop":
            timer = ""
            state = .idle
        case "hover_on":
            hovering = true
        case "hover_off":
            hovering = false
        default:
            break
        }
        if state == .blocked && now >= blockedUntil {
            state = restState()
        }
        return snapshot()
    }

    mutating func rest() -> PillSnapshot {
        state = restState()
        return snapshot()
    }

    private func restState() -> PillState {
        quietTimer && !timer.isEmpty ? .timer : .idle
    }

    func snapshot() -> PillSnapshot {
        let text: String
        let width: Int
        let height: Int
        let bars: Int
        let dash: Bool
        switch state {
        case .blocked:
            text = "Blocked"
            width = 92
            height = 28
            bars = 0
            dash = false
        case .errorState:
            text = "Error"
            width = 92
            height = 28
            bars = 0
            dash = false
        case .timer:
            text = timer
            width = 68
            height = 24
            bars = 0
            dash = false
        case .listening, .held:
            text = ""
            width = 112
            height = 30
            bars = 5
            dash = false
        case .thinking, .speaking:
            text = "MAEVE"
            width = 112
            height = 30
            bars = 3
            dash = false
        case .idle:
            text = ""
            width = hovering ? 62 : 54
            height = hovering ? 18 : 16
            bars = 0
            dash = true
        }
        let name = state == .errorState ? "error" : state.rawValue
        return PillSnapshot(state: name, text: text, width: width, height: height,
                            timer: timer, bars: bars, dash: dash)
    }

    static func clock(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds))
        return String(format: "%02d:%02d", total / 60, total % 60)
    }
}
