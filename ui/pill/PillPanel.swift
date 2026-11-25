import AppKit

final class PillPanel: NSObject {
    private let panel: NSPanel
    private let label: NSTextField
    private let bars: [NSView]
    private let button: NSButton
    var onActivate: (() -> Void)?

    init(snapshot: PillSnapshot, config: PillConfig) {
        let dark = config.theme != "light"
        let rect = NSRect(x: 0, y: 0, width: snapshot.width, height: 16)
        panel = NSPanel(contentRect: rect, styleMask: [.borderless, .nonactivatingPanel],
                        backing: .buffered, defer: false)
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.isOpaque = false
        panel.backgroundColor = .clear

        label = NSTextField(frame: NSRect(x: 2, y: 1, width: rect.width - 4, height: 14))
        label.isBezeled = false
        label.drawsBackground = false
        label.isEditable = false
        label.isSelectable = false
        label.alignment = .center
        label.font = NSFont.systemFont(ofSize: 10, weight: .medium)
        label.textColor = dark ? .white : .black
        label.stringValue = snapshot.text
        panel.contentView?.addSubview(label)

        var created: [NSView] = []
        for index in 0..<3 {
            let bar = NSView(frame: NSRect(x: 8 + index * 10, y: 5, width: 4, height: 6))
            bar.wantsLayer = true
            bar.layer?.backgroundColor = (dark ? NSColor.white : NSColor.black).cgColor
            created.append(bar)
            panel.contentView?.addSubview(bar)
        }
        bars = created

        button = NSButton(frame: rect)
        button.isBordered = false
        button.title = ""
        button.wantsLayer = true
        button.layer?.backgroundColor = NSColor.clear.cgColor
        panel.contentView?.addSubview(button)

        super.init()
        button.target = self
        button.action = #selector(pillTapped)
        panel.setFrame(rect, display: true)
        panel.orderFrontRegardless()
    }

    @objc private func pillTapped() {
        onActivate?()
    }

    func apply(_ snapshot: PillSnapshot) {
        label.stringValue = snapshot.text.isEmpty ? snapshot.timer : snapshot.text
        let listening = snapshot.state == "listening" || snapshot.state == "held"
        for (index, bar) in bars.enumerated() {
            bar.isHidden = !listening
            bar.frame.size.height = listening ? CGFloat(3 + index * 3) : 6
        }
        var frame = panel.frame
        frame.size.width = CGFloat(snapshot.width)
        panel.setFrame(frame, display: true)
    }

    func place(anchor: String) {
        guard let screen = NSScreen.main else { return }
        let frame = screen.visibleFrame
        let size = panel.frame.size
        let x: CGFloat
        let y: CGFloat
        switch anchor {
        case "bottom-right":
            x = frame.maxX - size.width - 12
            y = frame.minY + 12
        case "top-right":
            x = frame.maxX - size.width - 12
            y = frame.maxY - size.height - 12
        case "bottom-left":
            x = frame.minX + 12
            y = frame.minY + 12
        default:
            x = frame.midX - size.width / 2
            y = frame.minY + 12
        }
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }
}

final class PillController {
    private var machine = PillStateMachine()
    private let panel: PillPanel
    private let config: PillConfig
    private var lastEvent = ""

    init(config: PillConfig) {
        self.config = config
        panel = PillPanel(snapshot: PillStateMachine().snapshot(), config: config)
        panel.onActivate = { [weak self] in
            guard let self else { return }
            EventBridge.writeAction(self.machine.state == .held ? "ptt_end" : "ptt_start")
        }
        panel.place(anchor: config.anchor)
    }

    func tick() {
        for event in EventBridge.readEvents() where event != lastEvent {
            lastEvent = event
            let snapshot = machine.apply(event: event, now: Date().timeIntervalSince1970,
                                         duration: 60)
            panel.apply(snapshot)
        }
    }
}
