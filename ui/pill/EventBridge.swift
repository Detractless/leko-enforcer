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

    static func writeAction(_ action: String) {
        let url = appSupport.appendingPathComponent("pill-actions.json")
        guard let data = try? JSONSerialization.data(withJSONObject: ["action": action]) else {
            return
        }
        let temp = url.deletingLastPathComponent().appendingPathComponent(".pill-actions.tmp")
        try? data.write(to: temp)
        _ = try? FileManager.default.replaceItemAt(url, withItemAt: temp)
    }
}
