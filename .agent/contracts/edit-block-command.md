# Contract: edit-block-command

Draft for review. After the user opens it, the criteria are locked.

## Goal
Let the user add or remove named apps and sites from an existing block through a preview and an explicit save. Changes that loosen a locked block must be judged and, when required, wait before they land.

## Domains
Domains: none

## Acceptance criteria
- AC-001: `leko edit-block FILE --id ID` accepts one or more `--add-app`, `--remove-app`, `--add-site`, or `--remove-site` values, checks the request and existing settings, and previews the old and proposed coverage, the judge's result, any wait and its reason; without `--write`, it changes no file and starts no enforcement.
  - evidence: cli/tests/edit_block_draft.rs "edit_previews_coverage_and_saves_nothing"
  - breaker: The preview silently changes the zip or omits that a removal must wait.
- AC-002: The command edits named app and site coverage on an existing block only; the block ID, name, rules, door, locks, switch and unrelated fields and zip entries remain unchanged. It refuses an unknown ID, `allAppsExcept` coverage, malformed values, contradictory operations, and a result the existing settings reader would reject.
  - evidence: core/tests/edit_block_coverage.rs "edit_changes_only_named_coverage_and_refuses_invalid_requests"
  - breaker: An edit drops a lock or another field, edits an all-apps-except block as if it were a named list, or accepts a zip the reader refuses.
- AC-003: The core compares the current and proposed block with the existing loosening judge. Tightening-only changes may land now; a request containing any loosening is treated atomically as a loosening and is subject to the block's lock and all holding-routine locks. A partner key or a lock combination that cannot land without help is refused without saving.
  - evidence: core/tests/edit_block_plan.rs "edit_uses_the_judge_and_all_applicable_locks"
  - breaker: Removing a covered target bypasses a wait or routine lock, or the tightening half of a mixed request lands separately without being shown.
- AC-004: With `--write`, a change allowed now is applied atomically to the settings zip, the resulting zip is re-read by the core before saving, and the command reports the saved result; a no-op reports that nothing changed and does not rewrite either file.
  - evidence: cli/tests/edit_block_write.rs "immediate_edits_are_validated_saved_atomically_and_repeat_safely"
  - breaker: A failed write corrupts the original zip, a second identical operation duplicates a target, or the saved zip is not readable.
- AC-005: With `--write`, a change that must wait leaves the zip byte-for-byte unchanged and saves a bounded record of the requested operations, their original request time and due time in the waiting file; it reports the local due time and that the change is waiting.
  - evidence: cli/tests/edit_block_wait.rs "locked_edits_are_parked_without_changing_settings"
  - breaker: Removing a target from a locked block takes effect immediately, or the queued record contains no due time.
- AC-006: The waiting format is extended deliberately so existing format-1 switch-off records are preserved and readable, new edit records round-trip without losing operations, unknown formats and fields are refused whole, and older readers that cannot understand a new record do not silently drop it.
  - evidence: core/tests/edit_waiting_format.rs "waiting_formats_preserve_old_records_and_refuse_unknown_data"
  - breaker: Adding a delayed edit erases a previously waiting switch-off, or an older reader rewrites a new record without understanding it.
- AC-007: `leko settle` previews due coverage edits and, with `--write`, reapplies their operations to the current block, re-runs the judge and current applicable locks using the original request time, and never lands before the later of the saved and recomputed due times. A removed block, changed coverage mode, conflicting operation, malformed record or newly required key is reported and the affected edit is not applied.
  - evidence: cli/tests/edit_settle.rs "settle_rechecks_current_coverage_and_locks_before_applying"
  - breaker: A stale snapshot erases a later tightening, a newly tightened lock is ignored, or a conflicting edit is silently applied.
- AC-008: At most one waiting change per block is allowed across edit and switch-off kinds; when another exists, both edit and switch-off refuse to add a competing record and point to cancellation or settlement. `leko cancel` can cancel a waiting edit without changing the zip.
  - evidence: cli/tests/edit_waiting_conflicts.rs "waiting_changes_on_one_block_never_conflict"
  - breaker: An edit and switch-off of the same block are both queued and land in an undefined order, or cancelling an edit changes the block.
- AC-009: `leko status` shows waiting edits relevant to the requested app or site, including a target added or removed by a queued edit, with the operation and due time; a bad waiting file retains the existing status warning behavior and is not overwritten.
  - evidence: cli/tests/edit_status.rs "status_shows_edits_for_affected_targets"
  - breaker: A queued addition is invisible because the current block does not yet cover the target, or a bad waiting file is treated as empty and replaced.
- AC-010: The command warns when the proposed zip is refused by the phone's current Enforcer 0.3 reader, and only when that reader refuses it; the warning does not change whether the Mac can save a valid extended-format block.
  - evidence: cli/tests/edit_phone.rs "phone_compatibility_warning_matches_the_phone_reader"
  - breaker: Adding sites produces no compatibility warning, or an unchanged phone-compatible app block gets a false warning.
- AC-011: Missing, repeated or unknown arguments exit 2 with usage; unreadable or invalid settings and waiting files, unhonourable changes, and failed saves exit 1 with a plain reason. Successful previews and saves exit 0; every failure leaves pre-existing files intact, except that a zip already saved before a waiting-file cleanup failure is reported and recoverable by rerunning settle.
  - evidence: cli/tests/edit_errors.rs "errors_are_reported_and_saves_recover_safely"
  - breaker: A corrupt waiting file is overwritten, a bad argument is guessed, or a partial two-file settle is reported as complete.
- AC-012: A real CLI integration test previews and applies an immediate edit, parks a locked removal, lists and cancels or settles it, and verifies the resulting zip and unchanged unrelated entries.
  - evidence: cli/tests/real_edit_block.rs "the_real_program_edits_waits_and_settles"
  - breaker: The tested core functions work but the actual command bypasses them or writes different files.

## Checklist coverage
Not used: Domains is none; the derived checklist below stands in for it.

## Derived checklist
- D-1: Preview explains changes and writes nothing -> criterion:AC-001
- D-2: Only requested coverage fields change -> criterion:AC-002
- D-3: The existing loosening judge and all applicable locks are respected -> criterion:AC-003
- D-4: Immediate changes are validated and atomic -> criterion:AC-004
- D-5: Delayed changes preserve the settings zip -> criterion:AC-005
- D-6: Existing waiting records and unknown formats are handled safely -> criterion:AC-006
- D-7: Settlement rechecks current state and locks -> criterion:AC-007
- D-8: Competing changes cannot queue for the same block -> criterion:AC-008
- D-9: Status and cancellation cover queued edits -> criterion:AC-009
- D-10: Phone compatibility and errors are reported honestly -> criterion:AC-010

## Edge cases
- The same target is both added and removed, appears twice, is absent on removal, or is already present on addition -> AC-002
- A mixed addition and removal touches a locked block -> AC-003
- A previously waiting switch-off exists for the same block -> AC-008
- The block or its locks change between queueing and settlement -> AC-007
- A waiting file has an unknown kind or extra field -> AC-006
- A settings zip saves but waiting-file cleanup fails -> AC-011
- Two independent processes write settings at the same time -> out-of-scope: cross-process serialization is not implemented by the existing commands and needs its own contract.

## Out of scope
- Editing names, rules, schedules, doors, passes or locks; switching on or off and removing a block have separate commands or later contracts.
- Editing `allAppsExcept` coverage, partner-key entry, automatic background settlement, signed queues and phone sync.
- Installing or changing the LaunchAgent; editing uses the explicitly supplied settings file.

## Decisions
- Start with named app and site coverage | it delivers useful editing without claiming every block field is supported | REVERSIBLE
- Treat mixed tightening and loosening atomically | the preview and queued record describe one change and never silently split it | REVERSIBLE
- Store operations and their preconditions rather than a finished block snapshot | settlement must not erase unrelated changes made while waiting | REVERSIBLE
- Refuse competing waiting changes for the same block | avoids undefined order between edits and switch-offs | REVERSIBLE
- Preserve existing format-1 records and fail closed on unrecognized waiting data | existing delayed changes must not disappear during an upgrade | REVERSIBLE
- Keep preview by default and require `--write` | matches the existing change commands | REVERSIBLE
