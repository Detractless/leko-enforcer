# Contract: pill-move-mode

## Goal
Let a user reposition the native pill using the reviewed move-mode mock-up while preserving existing voice and card interactions.

## Domains
Domains: ui, web

## Acceptance criteria
- AC-001: Right-clicking the idle pill enters move mode, and right-clicking again drops it at its current position.
  - evidence: tests/ui/test-pill-move-mode.mjs "right click toggles move mode"
  - breaker: A context click starts dictation or fails to enter or exit move mode.
- AC-002: In move mode, dragging moves the native pill window with the pointer without starting or stopping voice capture.
  - evidence: tests/ui/test-pill-move-mode.mjs "drag moves pill without voice actions"
  - breaker: Dragging only moves web content or triggers push-to-talk.
- AC-003: Idle and hover match the existing app, and move and drag states match the reviewed mock-up at 112 by 30 pixels with the dash retained.
  - evidence: tests/ui/test-pill-move-mode.mjs "pill styles match idle hover and move states"
  - breaker: Hover shrinks the pill or the move state shows a dot, text, or dotted outline.
- AC-004: Moving the pill keeps it reachable on an available display and keeps its card anchored to the pill.
  - evidence: ui/src/move_mode.rs "movement_clamps_and_anchors_card"
  - breaker: The pill becomes unreachable or its card opens at the previous position.
- AC-005: A canceled drag, lost pointer, or display change exits safely without leaving voice capture or mouse input stuck.
  - evidence: ui/src/move_mode.rs "interrupted_drag_recovers_input"
  - breaker: An interrupted drag leaves the window capturing input indefinitely.
- AC-006: A keyboard-accessible action can enter and move the pill, Enter drops it, and Escape restores its starting position without triggering dictation.
  - evidence: tests/ui/test-pill-move-mode.mjs "keyboard moves and drops pill"
  - breaker: A user without a mouse cannot reposition the pill.

- AC-007: The dropped position persists across app restarts and is clamped to an available display when restored.
  - evidence: ui/src/move_mode.rs "saved_position_restores_on_available_display"
  - breaker: Restarting loses the position or restores the pill offscreen.
- AC-008: A right-click during dictation or speaking defers move mode until the app is idle without interrupting the voice session.
  - evidence: tests/ui/test-pill-move-mode.mjs "active voice defers move mode"
  - breaker: Entering move mode interrupts active voice or begins a drag during it.

## Checklist coverage
- UI-1 -> out-of-scope: No new screen or loading operation is added.
- UI-2 -> criterion:AC-006
- UI-3 -> criterion:AC-004
- WEB-1 -> out-of-scope: No network request is added.
- WEB-2 -> criterion:AC-004
- WEB-3 -> criterion:AC-001

## Edge cases
- Pointer release outside the window -> AC-005
- Display is disconnected during movement -> AC-005
- Position falls outside available display bounds -> AC-004
- Right-click during active voice capture -> AC-008

## Out of scope
- Changes to daemon, rule engine, extension, or voice recognition.
- New settings screens or changes to voice behavior outside move mode.

## Implementation plan
- Extend `ui/src/web/pill.js` and `ui/src/web/pill.css` for move state, input isolation, and reviewed styling; keep each edited file under 350 lines.
- Add a focused native movement module and integrate it with `ui/src/main.rs`, `ui/src/native_input.rs`, and `ui/src/card.rs` as needed; split existing oversized files before editing if necessary.
- Add focused UI and native tests for state transitions, coordinates, card anchoring, and interrupted input; manually verify on macOS without disturbing the daemon.
- Persist the dropped position and restore it safely on restart; implement deferred entry during voice activity and keyboard cancellation.
- Review the decision page before implementation; verify file and directory limits and run relevant UI and Rust tests.

## Decisions
- Right-click toggles move mode | Matches the reviewed mock-up | REVERSIBLE
- Retain the dash and use 112 by 30 pixels while moving | Matches the approved visual direction | REVERSIBLE
- Keep voice actions isolated during movement | Prevents accidental capture | REVERSIBLE
- Persist the dropped position | Avoids repeated repositioning | REVERSIBLE
- Defer move mode during active dictation or speaking | Preserves the voice session | REVERSIBLE
- Escape restores the starting position and Enter drops | Gives keyboard users a clear cancel and finish | REVERSIBLE
