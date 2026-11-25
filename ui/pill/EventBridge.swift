import Foundation

struct PillConfig: Codable {
    var theme: String
    var anchor: String
    var quietTimer: Bool

    init() {
        theme = "dark"
        anchor = "bottom-center"
        quietTimer = false
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        theme = try values.decodeIfPresent(String.self, forKey: .theme) ?? "dark"
        anchor = try values.decodeIfPresent(String.self, forKey: .anchor) ?? "bottom-center"
        quietTimer = try values.decodeIfPresent(Bool.self, forKey: .quietTimer) ?? false
    }
}

enum EventBridge {
    static var appSupport: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Leko")
    }

    static func loadConfig() -> PillConfig {
        let url = appSupport.appendingPathComponent("pill-config.json")
        guard let data = try? Data(contentsOf: url),
              let config = try? JSONDecoder().decode(PillConfig.self, from: data) else {
            return PillConfig()
        }
        return config
    }

    static func readEvents() -> [String] {
        let url = appSupport.appendingPathComponent("pill-events.json")
        guard let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let event = object["event"] as? String else {
            return []
        }
        return [event]
    }

    static func readPassTimer() -> String {
        let url = appSupport.appendingPathComponent("rules.pass-state.json")
        guard let data = try? Data(contentsOf: url),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let passes = object["passes"] as? [String: Any] else {
            return ""
        }
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        for (_, raw) in passes {
            guard let pass = raw as? [String: Any],
                  let earned = pass["earned_at"] as? String,
                  let date = formatter.date(from: earned) else {
                continue
            }
            let remaining = 60 - Date().timeIntervalSince(date)
            if remaining > 0 {
                return PillStateMachine.clock(remaining)
            }
        }
        return ""
    }

    static func writeAction(_ action: String) {
        let url = appSupport.appendingPathComponent("pill-actions.json")
        guard let data = try? JSONSerialization.data(withJSONObject: ["action": action]) else {
            return
        }
        let temp = url.deletingLastPathComponent().appendingPathComponent(".pill-actions.tmp")
        try? data.write(to: temp)
        _ = try? FileManager.default.replaceItemAt(url, withItemAt: temp)
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([0x0a]))
    }
}
