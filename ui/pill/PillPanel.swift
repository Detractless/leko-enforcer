import AppKit
import QuartzCore

final class CapsuleView: NSView {
    var onPress: ((Bool) -> Void)?
    var onRelease: ((TimeInterval) -> Void)?
    var onHover: ((Bool) -> Void)?
    private var pressedAt: Date?

    override var acceptsFirstResponder: Bool { true }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas {
            removeTrackingArea(area)
        }
        let area = NSTrackingArea(rect: bounds, options: [.mouseEnteredAndExited, .activeAlways],
                                  owner: self, userInfo: nil)
        addTrackingArea(area)
    }

    override func mouseEntered(with event: NSEvent) {
        onHover?(true)
    }

    override func mouseExited(with event: NSEvent) {
        onHover?(false)
    }

    override func mouseDown(with event: NSEvent) {
        pressedAt = Date()
        onPress?(true)
    }

    override func mouseUp(with event: NSEvent) {
        let held = pressedAt.map { Date().timeIntervalSince($0) } ?? 0
        pressedAt = nil
        onPress?(false)
        onRelease?(held)
    }
}

final class NonActivatingPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

final class PillPanel {
    private let panel: NonActivatingPanel
    private let capsule: CapsuleView
    private let dash: NSView
    private let label: NSTextField
    private var bars: [NSView] = []
    private let theme: PillTheme
    private var anchor: String
    private var current: PillSnapshot
    private var pulseAt: TimeInterval = 0

    var onPress: ((Bool) -> Void)?
    var onRelease: ((TimeInterval) -> Void)?
    var onHover: ((Bool) -> Void)?

    init(snapshot: PillSnapshot, config: PillConfig) {
        current = snapshot
        theme = PillTheme.make(config.theme)
        anchor = config.anchor
        let rect = NSRect(x: 0, y: 0, width: snapshot.width, height: snapshot.height)
        panel = NonActivatingPanel(contentRect: rect, styleMask: [.borderless, .nonactivatingPanel],
                                   backing: .buffered, defer: false)
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.hidesOnDeactivate = false
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.contentView?.wantsLayer = true
        panel.contentView?.layer?.masksToBounds = false
        panel.contentView?.layer?.shadowColor = theme.shadow
        panel.contentView?.layer?.shadowOpacity = 1
        panel.contentView?.layer?.shadowRadius = 4
        panel.contentView?.layer?.shadowOffset = CGSize(width: 0, height: -1)

        capsule = CapsuleView(frame: rect)
        capsule.wantsLayer = true
        capsule.autoresizingMask = [.width, .height]
        capsule.layer?.masksToBounds = true
        capsule.layer?.borderWidth = 1
        panel.contentView?.addSubview(capsule)

        dash = NSView(frame: NSRect(x: 0, y: 0, width: 12, height: 2))
        dash.wantsLayer = true
        dash.layer?.cornerRadius = 1
        dash.layer?.backgroundColor = theme.ink.withAlphaComponent(0.35).cgColor
        capsule.addSubview(dash)

        label = NSTextField(frame: .zero)
        label.isBezeled = false
        label.drawsBackground = false
        label.isEditable = false
        label.isSelectable = false
        label.alignment = .center
        label.stringValue = ""
        capsule.addSubview(label)

        for _ in 0..<5 {
            let bar = NSView(frame: .zero)
            bar.wantsLayer = true
            bar.layer?.cornerRadius = 1
            bar.layer?.backgroundColor = theme.ink.cgColor
            capsule.addSubview(bar)
            bars.append(bar)
        }

        capsule.onPress = { [weak self] down in self?.onPress?(down) }
        capsule.onRelease = { [weak self] held in self?.onRelease?(held) }
        capsule.onHover = { [weak self] hovering in self?.onHover?(hovering) }

        panel.setFrame(rect, display: true)
        panel.orderFrontRegardless()
        apply(snapshot, animated: false)
        place()
    }

    func apply(_ snapshot: PillSnapshot, animated: Bool = true) {
        current = snapshot
        let size = NSSize(width: snapshot.width, height: snapshot.height)
        let frame = NSRect(origin: panel.frame.origin, size: size)
        capsule.layer?.cornerRadius = size.height / 2
        capsule.layer?.backgroundColor = theme.fill.cgColor
        capsule.layer?.borderColor = theme.border.cgColor
        capsule.layer?.backgroundColor = theme.fill.cgColor

        dash.isHidden = !snapshot.dash
        dash.frame = NSRect(x: (size.width - 12) / 2, y: (size.height - 2) / 2, width: 12, height: 2)

        label.isHidden = snapshot.text.isEmpty
        label.stringValue = snapshot.text.uppercased() == "MAEVE" ? "MAEVE" : snapshot.text
        let mono = snapshot.state == "timer" || snapshot.text == "MAEVE"
        label.font = NSFont.systemFont(ofSize: mono ? 10 : 11, weight: .medium)
        label.textColor = snapshot.text == "MAEVE" ? theme.mutedInk : theme.ink
        if snapshot.text == "MAEVE" {
            label.frame = NSRect(x: 8, y: 8, width: 50, height: 14)
            label.alignment = .left
        } else {
            label.frame = NSRect(x: 4, y: (size.height - 14) / 2, width: size.width - 8, height: 14)
            label.alignment = .center
        }

        let visible = snapshot.bars
        let gap: CGFloat = 3
        let totalWidth = CGFloat(visible) * 2 + CGFloat(max(0, visible - 1)) * gap
        let startX = visible == 3 ? 64 : (size.width - totalWidth) / 2
        for (index, bar) in bars.enumerated() {
            bar.isHidden = index >= visible
            bar.frame = NSRect(x: startX + CGFloat(index) * (2 + gap),
                               y: (size.height - 13) / 2, width: 2, height: 13)
        }

        if animated {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.24
                context.timingFunction = CAMediaTimingFunction(name: .easeOut)
                panel.animator().setFrame(frame, display: true)
            }
        } else {
            panel.setFrame(frame, display: true)
        }
        place()
    }

    func animate(_ now: TimeInterval) {
        pulseAt = now
        let speaking = current.state == "speaking" || current.state == "thinking"
        for (index, bar) in bars.enumerated() where !bar.isHidden {
            let cycle = speaking ? 1.15 : 0.75
            let phase = sin(now * 2 * .pi / cycle + Double(index) * 0.9)
            let low: CGFloat = speaking ? 4 : 4
            let high: CGFloat = speaking ? 10 : 13
            let height = low + (high - low) * CGFloat((phase + 1) / 2)
            let alpha: CGFloat = speaking ? 0.7 : 0.45 + 0.55 * CGFloat((phase + 1) / 2)
            bar.layer?.backgroundColor = theme.ink.withAlphaComponent(alpha).cgColor
            var frame = bar.frame
            frame.size.height = height
            frame.origin.y = (panel.frame.size.height - height) / 2
            bar.frame = frame
        }
    }

    func place() {
        guard let screen = NSScreen.main else { return }
        let frame = screen.visibleFrame
        let size = panel.frame.size
        let pad: CGFloat = 20
        let x: CGFloat
        let y: CGFloat
        switch anchor {
        case "bottom-right":
            x = frame.maxX - size.width - pad
            y = frame.minY + pad
        case "top-right":
            x = frame.maxX - size.width - pad
            y = frame.maxY - size.height - pad
        case "bottom-left":
            x = frame.minX + pad
            y = frame.minY + pad
        default:
            x = frame.midX - size.width / 2
            y = frame.minY + pad
        }
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    func setHoverFill(_ hovering: Bool) {
        capsule.layer?.backgroundColor = (hovering ? theme.hoverFill : theme.fill).cgColor
    }
}
