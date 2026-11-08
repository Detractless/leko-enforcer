# Conversation and local memory

The existing local LFM3 model receives three recent user/assistant exchanges
(six messages, each capped at 400 characters). History lasts only for this
process. Followups such as “make that 45 minutes” can use the previous request
and prepared summary. Model selection, temperature, server context size, and
thinking settings are unchanged. No extra model is used.

## Confirmations

One backend-owned action can be pending, for 120 seconds. Blocks, switching off
blocks, and focus sessions require confirmation. Sessions have no CLI preview
mode, so their CLI command runs only after confirmation. Status checks run
without confirmation. The existing explicit `--confirm` flag bypasses the prompt.

Say `yes`, `yeah`, `yep`, `yup`, `yes please`, `confirm`, `go ahead`, or `do it`.
Say `no`, `nope`, `no thanks`, `cancel`, `cancel it`, `never mind`, or `nevermind`
to dismiss. Matching ignores case and punctuation and requires the whole phrase.
These responses bypass the model and the spoken filler filter.

UI buttons send a backend-generated action ID. The backend ignores the intent
payload supplied by the UI and consumes a matching action once. New substantive
requests, stop, mute, cancellation, and window closure clear pending actions.
Expired or replayed IDs cannot execute; a stale ID cannot confirm or cancel a
replacement. The UI displays success only after the backend reports success.
Preparation, confirmation, cancellation, and tool results use fixed templates;
ordinary conversation still uses the model.

Cancellation prevents future launches; it cannot undo a CLI action already
launched. Action claiming and launching are serialized with cancellation.

## Explicit memory commands

- `remember focus length is 25m` (also `remember focus length = 25m`, or `equals`)
- `correct focus length is 45m` (requires an existing key)
- `forget focus length` or `forget all`

Keys are case-insensitive. Remembering an existing key replaces its value.
Commands are deterministic, with no model call. Only explicit user-supplied
preferences/facts are persisted: no automatic extraction, transcript archive,
or background learning. Correction and forgetting also clear recent history,
so obsolete facts are not carried forward there.

Storage defaults to `~/Library/Application Support/Leko/memory.sqlite3`.
Set `LEKO_MEMORY_PATH` to override it. The SQLite file has owner-only permissions;
it is local plaintext, not encrypted. SQLite secure deletion is enabled; external
backups are outside this feature. No memory file is created until a memory
command is used. History and pending actions are never saved to this file.

Limits: 200 entries, 80 characters per key, 320 per value. Retrieval selects up
to three entries by shared nontrivial words in the current query and saved facts.
It is lexical retrieval, not semantic embeddings; paraphrases without shared
words may miss. Unrelated facts are excluded. Retrieved facts are labeled
untrusted context, never authorization, and cannot bypass backend confirmation.
Model followup quality and prompt-injection resistance remain model-dependent.

## Files and verification

New files: `voice/session/conversation.py`, `voice/memory/local_store.py`,
`tests/test_conversation.py`, `tests/test_runtime_confirmation.py`, and this guide.
Updated files: `voice/session/query_flow.py`, `voice/session/runtime.py`,
`voice/llm_router.py`, `voice/ui_bridge.py`, `ui/web/app.js`,
`tests/ui_confirmation.test.mjs`, `tests/ui_voice_ipc.test.mjs`, and
`tests/clean_summaries.test.mjs`. Existing unrelated dirty edits are preserved.

Validation result: 14 isolated Python tests and 21 existing Node regression tests
passed (35 total). `git diff --check` passed.

```sh
rtk proxy python3 -m unittest discover -s tests -p 'test_*.py' -v
rtk proxy node --test tests/ui_confirmation.test.mjs tests/ui_voice_ipc.test.mjs tests/ui_states.test.mjs tests/ui_rendering.test.mjs tests/clean_summaries.test.mjs tests/barge_in_and_cancel.test.mjs tests/serialized_turns.test.mjs tests/ui_bridge.test.mjs
```

New tests fake the model, CLI, speech, microphone, and runtime worker; memory
uses temporary directories. Coverage includes followup context, history bounds,
spoken confirmation before filler filtering, UI IDs, concurrent duplicate
confirmations, cancellation during preview and after staging, stale/expired
confirmations, failed action results, sessions, explicit auto-confirmation,
relevant retrieval, persistence, correction, forgetting, limits, permissions,
and router payloads.

Live model quality, physical microphone behavior, native UI rendering, and real
CLI effects were deliberately not exercised. No commits were made.
