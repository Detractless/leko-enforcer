# Leko Mandatory-Enforcement Design (one-page sketch)

Behavioral design only. Inspired by patterns observed in Cold Turkey; no proprietary code, strings, assets, or implementation details are copied.

## Goal

Make the browser extension a required sensor, not an optional add-on. If a browser is open and its Leko extension is missing, disabled, or detached from the native host, Leko warns once and then closes that browser while an active block is locked. The voice loop remains the primary control surface; the UI stays a minimal circle.

## Components

| Component | Responsibility |
|---|---|
| `voice/` + `core/` (existing) | Speech → intent → tool call → CLI enforce → spoken confirmation |
| `EnforcementController` | Owns active blocks, grace timers, and the decision to warn or close |
| `ExtensionHandshake` | Per-browser probe + native messaging channel; reports healthy / missing / disabled / detached |
| `PlatformAdapter` | macOS now (process control, LaunchAgent); Android later (VPN + usage stats) |
| `Installer / Watchdog` | One-time privileged install, per-user self-healing agent, tamper detection |
| `ui/circle` | Shows one of three states: verifying, warning countdown, locked |

## Flow

1. Native host registers with each supported browser and exposes a small stdio channel to the extension.
2. On a 5 s tick, `ExtensionHandshake` reads the browser's own profile state to confirm the extension is installed, enabled, and allowed on all sites; it also checks for launch flags that disable extensions. Read errors are treated as **unknown**, never as healthy.
3. The extension pushes a compact state heartbeat (active block id, policy, expiry) and reports page/chunk evaluations back to the daemon.
4. If a browser is open and the extension is missing/disabled/detached while a block is live:
   - spoken warning + circle countdown (grace period, e.g. 60 s);
   - if still unhealthy after grace, `PlatformAdapter` terminates that browser process (SIGTERM, then SIGKILL only if needed);
   - the event is written to the session log.
5. Recovery: user installs/re-enables the extension, or speaks an explicit unlock phrase that goes through the normal backend-owned confirmation flow. No silent bypass.

## Installer and persistence

- macOS `.pkg` with pre/post-install scripts; root used only at install time to place the per-user LaunchAgent plist and native host manifests.
- KeepAlive LaunchAgent relaunches the user agent with `-agent`; the app itself never runs privileged.
- Where enterprise policy is available, force-install the extension; otherwise the first-run voice prompt guides a one-time manual install.
- Watchdog re-asserts the plist, manifests, and extension allowlist periodically; logs tamper events and re-locks rather than quietly repairing.
- Android: system VPN + usage stats provide OS-level enforcement so the extension is never the only sensor.

## Safety and non-goals

- Always warn and count down before closing a browser; log every force-close.
- Emergency unlock must be voice-initiated, confirmed, and time-bounded.
- Never treat a probe error as success; fail closed but allow the explicit recovery path.
- Do not implement firewall/packet filtering in v1; do not copy Cold Turkey code or UI.
- Do not ship a hidden daemon with elevated rights; keep root to install scripts only.

## Acceptance checks

1. Extension healthy + active block → browser stays open, page enforcement works.
2. Extension disabled mid-block → spoken warning, countdown, browser closed after grace.
3. Native host unreachable → fail closed, logged, recovery prompt available.
4. Daemon restart → extension reconnects without user action; no state loss.
5. Installer run twice → idempotent, no duplicate LaunchAgent or manifest.
6. Android adapter stub compiles and reports "not configured" without affecting macOS.
