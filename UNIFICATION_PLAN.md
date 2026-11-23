# Unification Plan — Leko (leko-enforcer)

Correction: an earlier draft of this plan treated the browser extension and GLiDE daemon as the product. That was wrong. Leko is a local-first, voice-controlled self-enforcement product with almost no UI. The browser stack is one optional enforcement/research surface.

Status: audit only. No branch was deleted, overwritten, or force-pushed; `main` and all existing branches are untouched. The live daemon on `127.0.0.1:8765` was not restarted.

## 1. What Leko is end-to-end

A hands-free enforcement loop:

1. **Input** — native audio capture with echo cancellation and VAD (`audio/src/`, `voice/audio_io/`), hands-free turn-taking, barge-in and push-to-talk (contracts on `codex/voice-assistant-sync`).
2. **Understanding** — `voice/assistant.py` drives the loop; `voice/routing/intent_router.py` classifies `tool_call / conversational / correction / status_check`; `voice/llm_router.py` runs a local LFM3-1.7B model with tools (`add_block`, `edit_block`, `switch_on`, `status`, `session`, delayed schedules `always / for_duration / open_hours / blocked_hours`, device actions `lock / logoff / shutdown`).
3. **Validation** — `voice/intent_validation.py` maps tool calls to CLI arguments; `voice/request_state/` (on `voice-request-state-eval`) extracts and evaluates request state before execution.
4. **Enforcement** — the `leko` CLI (`cli/src/`) executes the block. `core/src/lib.rs` is a pure decision judge over settings zips (blocks, schedules, waiting changes) with no OS calls or clock. `cli/src/enforcement/` kills apps by bundle ID on each tick; `cli/src/launch_agent/` installs a per-user macOS LaunchAgent; device lock/logoff/shutdown are platform adapters.
5. **Minimal UI** — `ui/src/main.rs` is a native WebView circle; `voice/ui_bridge.py` emits only state/draft/confirm events ("Maeve Speaking…", "Locked & Saved"). No dashboards.
6. **Feedback** — `voice/verbalizer.py` and reviewed pronunciation rules speak confirmations; `voice/session/session_log.py` writes dated JSONL diagnostics. Confirmations are backend-owned (6-message history, 120 s pending action).

Target platforms: macOS now; Android and others later via the same core/CLI contract. The circle UI is intentionally cross-platform (`.agent/contracts/minimalist-circle-ui.md`).

Names searched: "Proser" appears nowhere in the repos or workspace; "Lego" appears only as `lego.com` in WAE research notes. The product name is Leko; "Maeve" is the TTS voice, "Zoey" the model voice.

## 2. Local + GitHub audit

| Location | Role | State |
|---|---|---|
| `/Users/calibro/Projects/leko-enforcer` | Canonical repo, current branch `feat/glide-policy-enforcer` (local commits `cba8c80`, `6a32f76`, `030cd48`) | Carries voice/Rust product plus the added GLiDE enforcer; `main` unchanged |
| `/Users/calibro/.t3/scratch/leko-enforcer` | Second clone on `main` | Uncommitted opt-in collector config (deployed worker URL, GitHub App ID, extension key, `.wrangler/`) |
| `/Users/calibro/.t3/scratch/2026-11-12-when-it-comes-to-this-5b9a1cbc` | Research workspace, not a git repo | WAE/LFM3 reports, Kaggle artifacts, `regions.mjs` Rev8, evaluations logs, live GLiDE daemon source |

Remote: `https://github.com/Detractless/leko-enforcer.git`. Branches: `main`, `codex/voice-assistant-sync`, `voice-request-state-eval`, `agent/contract-runtime-bundle`, plus local `feat/glide-policy-enforcer`.

Push status: `git push -u origin feat/glide-policy-enforcer` failed — GitHub authenticated as **Dr-FacePlant** and returned 403 for the Detractless repository. The commits are safe locally. After authenticating as a collaborator:

```
git -C /Users/calibro/Projects/leko-enforcer push -u origin feat/glide-policy-enforcer
```

## 3. Component classification

**Active Working Core (the product)**
- `voice/` assistant, intent router, LLM router, normalization, session logs, UI bridge, verbalizer.
- `core/` pure Rust decision judge; `cli/` commands and macOS enforcement, LaunchAgent, device lock; `audio/` FFI; `ui/` minimal circle.
- `.agent/contracts/` decisions for hands-free turn-taking, push-to-talk, barge-in, device lock, minimal UI.
- Newest voice work lives on `codex/voice-assistant-sync` (push-to-talk, speech normalization, session logs, settings store) and `voice-request-state-eval` (request-state extraction/evaluation, LLM benchmark fixtures).

**Optional Enforcement Surfaces / Missing Valuable Modules**
- Browser/GLiDE stack added on the feature branch: `src/daemon/` GLiDE trees + 15 declarative packs, `src/blocker-extension/` curtain and adapters. Keep behind an `EnforcementAdapter` interface; it must never become the product's identity.
- `main`: opt-in capture collector (`src/browser-extension/`, `src/collector-worker/`) for training data; sanitize the scratch clone's deployed IDs/keys before committing.
- Workspace: `regions.mjs` Revision 8 (SHA-pinned) used by the daemon; make its path configurable and never edit it.
- `agent/contract-runtime-bundle`: contract/plan guard tooling — developer tooling, not shipped runtime.

**Superseded Experiments**
- WAE/Kaggle research reports, old decision-tree benchmarks, policy-v0 pilots (`wae-source-reference/docs/POLICY_v0.md`, `policy.default.json`), one-off browser injection bundles, VAD comparison notes (`docs/voice-comparison.md` is a test-setup record), and older extension prototypes in unrelated scratch folders. Archive only after migration is verified.

## 4. Proposed single-repo layout

Every folder stays at or below 8 files and every file below 350 lines.

```
leko-enforcer/
  core/                 pure judge (blocks, schedules, decisions, waiting changes)
  cli/                  add_block, edit_block, switch_on, status, session, enforcement
  platform/
    macos/              launch agent, app kill, device lock, objc bridge
    android/            future adapters
  ui/circle/            minimal native WebView shell only
  audio/                capture, VAD, echo cancellation
  voice/
    assistant.py        main loop
    routing/            intent router, LLM router, validation
    request_state/      extraction, draft summary, flow, state
    session/            conversation, query flow, session log
    normalization/      speech text
    audio_io/           capture gate, push_to_talk
    ui_bridge.py
  enforcers/
    browser/
      daemon/           (7 .py max, policies/ 4 .py)
      policy_packs/     config.json, allow/ 6 JSON, block/ subfolders
      extension/        (6 files + adapters/ + ui/)
    native/             delegates to cli/
  data/collector/       browser/ + worker/ (sanitized config)
  tools/contract-runtime/
  docs/
  UNIFICATION_PLAN.md
```

The repo already has oversized folders (`.agent/contracts` 52 files, `tests` 50, `src/browser-extension` 10, `voice` 11). Split these during migration into subfolders of at most 8; do not add to them.

## 5. Migration steps (awaiting approval)

1. Push the preservation branch with the command above; open a PR to `main` without merging.
2. Land the voice work first, in this order: `codex/voice-assistant-sync` (push-to-talk, normalization, session logs), then `voice-request-state-eval` (request-state and eval harness). Split folders over 8 files as part of each PR.
3. Define a single `EnforcementAdapter` contract in `core/`; implement native (delegates to `cli/`) as primary and the browser extension as a mandatory sensor on every supported browser, with GLiDE packs supplying policy rules.
4. Add a `platform/` abstraction so macOS code moves out of `cli/` and Android can be added later without touching the judge.
5. Move the collector into `data/collector/{browser,worker}`, replace deployed IDs/keys with `.env.example`, keep secrets out of git.
6. Vendor `regions.mjs` Rev8 with a SHA check and an env-overridable path; never edit the file.
7. Move contract guards to `tools/contract-runtime/`; keep them out of the shipped app.
8. Add CI failing on files >= 350 lines or folders > 8 files under `core/`, `voice/`, `cli/`, `ui/`, `enforcers/`, `tests/`.
9. Only after tests pass, cut over the live host: stop the scratch daemon, start the repo daemon with the same config, re-run the 15-pack benchmark and browser flows. Keep `shadow_mode: false`.
10. Archive superseded research artifacts last. Never delete or rewrite branches.

## 6. Mandatory browser enforcement (approved direction)

The browser extension is required, not optional. If a supported browser is open without a healthy Leko extension, Leko speaks one warning, counts down, then closes that browser while a block is locked. See [MANDATORY_ENFORCEMENT_DESIGN.md](MANDATORY_ENFORCEMENT_DESIGN.md) for the handshake, grace period, fail-closed behavior, installer/watchdog pattern, Android fallback, and acceptance checks. This is a behavioral design derived from reviewing other blockers; no proprietary code or assets are used.

Add to migration: implement `ExtensionHandshake` alongside `EnforcementAdapter`, ship the `.pkg` installer with a per-user KeepAlive LaunchAgent and native host manifests, and add the six acceptance checks to CI/manual verification before any live cutover.
