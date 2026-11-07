# Contract: minimalist-circle-ui

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.

## Goal
Provide a minimalist, resource-efficient cross-platform WebView interface centered around a single organic circle that visually communicates voice assistant states (idle, listening, thinking, speaking) and allows natural conversation and block/session drafting on macOS, Windows, Linux, and Android.

## Domains
Domains: ui, web

## Acceptance criteria
- AC-001: The interface renders a centered minimalist ring on a neutral background (#E2E1DD) with no external web dependencies, fonts, or network assets, loading in under 100ms and consuming under 35 MB of RAM in a native Webview window.
  - evidence: tests/ui_rendering.test.mjs "interface_renders_pure_minimalist_circle_with_no_external_assets"
  - breaker: External CDN requests are made, fonts fail to render offline, or memory footprint exceeds 35 MB.
- AC-002: The circle component exposes distinct visual states ('idle', 'listening', 'thinking', 'speaking', 'confirmed') with CSS/SVG hardware-accelerated transitions: gentle breathing in idle, inward ripples during user listening, orbit/rotation during LLM reasoning, and outward ripples during speech synthesis.
  - evidence: tests/ui_states.test.mjs "circle_transitions_across_all_voice_states"
  - breaker: State transitions drop frames, fail to update classes, or ripples are misplaced or missing.
- AC-003: An in-process / local bridge connects the UI to the Leko voice pipeline (`llm_router` and `leko-core`), sending user voice/text turns and receiving structured plan drafts and state transitions without polling loops.
  - evidence: tests/ui_bridge.test.mjs "bridge_transfers_intents_and_draft_responses"
  - breaker: Messages stall, UI blocks during LLM inference, or audio output desyncs from the speaking animation.
- AC-004: When a block or session is drafted, the UI renders an unobtrusive, minimalist confirmation card below the circle detailing targets and duration, which can be confirmed or cancelled verbally or via touch/click.
  - evidence: tests/ui_confirmation.test.mjs "draft_card_renders_and_handles_confirm_actions"
  - breaker: Malformed blocks are shown without validation, confirmation writes without user consent, or the card obscures the circle.

## Checklist coverage
- UI-1 -> criterion:AC-002
- UI-2 -> criterion:AC-004
- UI-3 -> criterion:AC-001
- WEB-1 -> criterion:AC-003
- WEB-2 -> criterion:AC-004
- WEB-3 -> criterion:AC-004

## Edge cases
- Audio input device is missing or permission is denied -> criterion:AC-003
- LLM inference takes longer than expected or times out -> criterion:AC-002
- Window resized to vertical phone proportions or wide desktop window -> criterion:AC-001

## Out of scope
- Complex multi-tab configuration panels or manual JSON editing screens (the CLI handles direct configuration).
- Third-party cloud telemetry or analytics.

## Decisions
- Use OS-native Webview (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux, Android System WebView) | Provides cross-platform compatibility across all desktop and mobile OSs with ~20-30 MB RAM and zero bundled Chromium | REVERSIBLE
