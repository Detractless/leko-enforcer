import AppKit

struct PillTheme {
    let fill: NSColor
    let hoverFill: NSColor
    let border: NSColor
    let ink: NSColor
    let mutedInk: NSColor
    let shadow: CGColor

    static func make(_ name: String) -> PillTheme {
        if name == "light" {
            return PillTheme(
                fill: PillTheme.color(250),
                hoverFill: PillTheme.color(255),
                border: PillTheme.color(204),
                ink: PillTheme.color(20),
                mutedInk: PillTheme.color(110),
                shadow: PillTheme.shadow(0.14)
            )
        }
        return PillTheme(
            fill: PillTheme.color(24),
            hoverFill: PillTheme.color(34),
            border: PillTheme.color(48),
            ink: PillTheme.color(242),
            mutedInk: PillTheme.color(142),
            shadow: PillTheme.shadow(0.55)
        )
    }

    private static func color(_ value: Int) -> NSColor {
        let amount = CGFloat(value) / 255
        return NSColor(calibratedRed: amount, green: amount, blue: amount, alpha: 1)
    }

    private static func shadow(_ alpha: Float) -> CGColor {
        CGColor(red: 0, green: 0, blue: 0, alpha: CGFloat(alpha))
    }
}
