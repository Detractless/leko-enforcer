# Contract: ui-voice-ipc

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Wire the minimalist circle UI to the Leko voice assistant pipeline via an IPC bridge, dynamically reflecting voice states (listening, thinking, speaking, confirmed) and enabling interactive plan confirmation cards.

## Domains
Domains: ui, cli

## Acceptance criteria
- AC-001: The voice pipeline emits standardized JSON event protocol messages (listening, thinking, speaking, draft, confirmed) across query lifecycle.
  - evidence: tests/ui_voice_ipc.test.mjs "voice_pipeline_emits_lifecycle_events_to_ipc"
  - breaker: query processing executes without emitting state transitions to the IPC channel.
- AC-002: The UI IPC listener consumes JSON state events and updates the circle animation classes accordingly.
  - evidence: tests/ui_voice_ipc.test.mjs "ui_bridge_updates_circle_state_on_ipc_event"
  - breaker: incoming IPC state event leaves the visual circle in idle state.
- AC-003: When a policy change is drafted, the bridge triggers the plan confirmation card and waits for user confirm/cancel.
  - evidence: tests/ui_voice_ipc.test.mjs "draft_event_renders_card_and_transmits_user_action"
  - breaker: draft event applies policy changes immediately without user confirmation.
- AC-004: Native desktop UI launcher forwards IPC postMessage from WebView to stdout and evaluates incoming stdin JSON events into WebView.
  - evidence: tests/ui_voice_ipc.test.mjs "native_launcher_handles_bidirectional_stdio_ipc"
  - breaker: WebView events do not escape the native process to the assistant controller.

## Checklist coverage
- UI-1 -> criterion:AC-002
- UI-2 -> criterion:AC-003
- UI-3 -> criterion:AC-002
- CLI-1 -> out-of-scope: IPC binary takes no positional cli arguments
- CLI-2 -> criterion:AC-004
- CLI-3 -> criterion:AC-001

## Edge cases
- Malformed JSON sent across IPC channel -> AC-004
- User dismisses confirmation card without speaking -> AC-003
- Voice pipeline receives concurrent queries while speaking -> AC-001

## Out of scope
- Cloud-hosted remote WebSockets (local Unix stdio / in-memory IPC only).
- Audio DSP filtering in JS (audio capture remains in native/Python backend).

## Decisions
- Stdio JSON lines IPC | Zero port conflicts, cross-platform on macOS/Linux/Windows, zero network attack surface | REVERSIBLE
