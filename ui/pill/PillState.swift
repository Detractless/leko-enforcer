import Foundation

enum PillState: String {
    case idle
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
    let timer: String
}

struct PillStateMachine {
    var state: PillState = .idle
    var timer: String = ""
    var blockedUntil: TimeInterval = 0

    mutating func apply(event: String, now: TimeInterval, duration: TimeInterval) -> PillSnapshot {
        switch event {
        case "blocked":
            state = .blocked
            blockedUntil = now + 2
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
            state = .idle
        case "timer_start":
            timer = PillStateMachine.clock(duration)
        case "timer_stop":
            timer = ""
        default:
            break
        }
        if state == .blocked && now >= blockedUntil {
            state = .idle
        }
        if state != .speaking && timer.isEmpty {
            timer = ""
        }
        return snapshot()
    }

    func snapshot() -> PillSnapshot {
        let text: String
        let width: Int
        switch state {
        case .blocked:
            text = "Blocked"
            width = 90
        case .errorState:
            text = "Error"
            width = 72
        case .listening, .held, .thinking, .speaking:
            text = ""
            width = 72
        case .idle:
            text = ""
            width = 54
        }
        let name = state == .errorState ? "error" : state.rawValue
        return PillSnapshot(state: name, text: text, width: width, timer: timer)
    }

    static func clock(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds))
        return String(format: "%02d:%02d", total / 60, total % 60)
    }
}
