# Contract: edit-block-schedule

Draft for review. After the user opens it, the criteria are locked.

## Goal
Let the user change the allowed hours or weekdays of one existing `outsideHours` rule, preview the result, and explicitly save it. Any change that might let an app or site open sooner must obey the block's applicable locks and delays.

## Domains
Domains: none

## Acceptance criteria
- AC-001: `leko edit-schedule FILE --id BLOCK --rule RULE` accepts `--open-hours HH:MM-HH:MM[,HH:MM-HH:MM...]` and/or `--days mon,tue,...` or `--days all`; omitted fields stay as they are. Without `--write`, it previews the current and proposed rule, whether it can land now or must wait, the reason and local due time, and changes no files.
  - evidence: cli/tests/schedule_edit_draft.rs "schedule_preview_shows_old_new_and_wait_without_writing"
  - breaker: Preview saves a schedule, resets an omitted field, or hides a wait.
- AC-002: The edit targets exactly one existing `outsideHours` rule by ID and changes only its allowed stretches and/or weekdays; all other rules, block fields, locks, zip entries and their contents remain unchanged. Unknown blocks or rules, other rule kinds, invalid days or stretches, duplicate flags and a result the settings reader refuses are rejected without saving.
  - evidence: core/tests/schedule_edit_rule.rs "schedule_edit_changes_only_the_named_outside_hours_rule"
  - breaker: A different rule or a lock is replaced, an invalid stretch is accepted, or a session is silently converted to an hours rule.
- AC-003: Before implementing the safety decision, a focused test reproduces or disproves whether adding an overnight weekday can open time that was previously closed while the existing rule comparator calls it tightening; the result and its scope are recorded in the test. A possible example is adding Sunday to a Monday-only 22:00–06:00 rule and checking Monday 01:00.
  - evidence: core/tests/schedule_overnight_regression.rs "overnight_weekday_change_is_checked_against_real_decisions"
  - breaker: A rule-comparison result is trusted without checking whether the actual decision changes from closed to open.
- AC-004: A schedule edit may bypass locks only if the existing loosening judge and an additional decision-based comparison both establish that the rule cannot change from closed to open. The comparison covers every weekday and local minute, including overnight carry and 24:00; if it cannot establish that property, it treats the change as loosening. Clock changes are checked with real zone transitions, including the New York spring and fall transitions.
  - evidence: core/tests/schedule_edit_safety.rs "schedule_comparison_never_misses_a_closed_to_open_change"
  - breaker: Adding an overnight weekday opens an earlier morning period immediately despite an active lock, or a DST jump bypasses the comparison.
- AC-005: Tightening-only changes may land immediately; any loosening or unproven change is atomic and is judged against the block's own lock and every holding routine's lock. A required partner key or incompatible change windows are refused without saving; mixed changes are not silently split.
  - evidence: core/tests/schedule_edit_plan.rs "schedule_plan_applies_all_locks_to_possible_loosenings"
  - breaker: A widened open window or removed weekday bypasses a block or routine wait, or half a mixed edit lands early.
- AC-006: With `--write`, an immediately permitted change is validated by reading the resulting zip again, then saved atomically; an unchanged schedule is a no-op that rewrites neither zip nor queue. A delayed change leaves the zip byte-for-byte unchanged and parks the targeted rule's precondition, requested replacement, original request time and due time.
  - evidence: cli/tests/schedule_edit_save.rs "schedule_saves_now_or_parks_without_touching_the_zip"
  - breaker: A failed save damages the zip, a no-op rewrites it, or a waiting change changes enforcement immediately.
- AC-007: The waiting format supports schedule changes without losing existing format-1 switch-offs or format-2 coverage edits; unknown formats, kinds and fields are refused whole, and older readers fail closed when a new schedule record is present. Queue records are size-bounded and do not overwrite a corrupt waiting file.
  - evidence: core/tests/schedule_waiting_format.rs "schedule_records_preserve_older_waiting_changes_and_fail_closed"
  - breaker: A format upgrade silently drops an existing queued edit or lets an old reader discard a schedule record.
- AC-008: `leko settle` reapplies a due schedule change only to the current target rule, checks its saved precondition, re-runs both safety checks and current applicable locks at the original request time, and never applies it before the later of the stored and recomputed due moments. A changed or missing target, changed rule kind, newly required partner key or conflicting schedule is reported and kept; unrelated later block changes are preserved.
  - evidence: cli/tests/schedule_edit_settle.rs "settle_rechecks_schedule_preconditions_and_current_locks"
  - breaker: A stale queued schedule replaces a newer schedule, undoes a coverage tightening, or ignores a newly extended lock.
- AC-009: A block with any waiting change cannot receive a competing waiting schedule change; other change commands also refuse to queue a competing change for that block. `leko cancel` removes a waiting schedule change without editing the zip, and `leko status` shows the pending schedule and due time for targets covered by that block.
  - evidence: cli/tests/schedule_edit_waiting.rs "waiting_schedule_changes_are_visible_cancellable_and_nonconflicting"
  - breaker: A schedule edit and a switch-off queue for the same block in an undefined order, cancellation changes the zip, or status hides a pending schedule change.
- AC-010: The phone compatibility warning reflects whether the complete proposed zip is accepted by Enforcer 0.3, including extended fields already present; bad settings or waiting files and failed saves are reported without replacement. If the zip saves but queue cleanup fails, the command reports that partial result and a rerun safely clears the already-applied record.
  - evidence: cli/tests/schedule_edit_errors.rs "schedule_errors_phone_warning_and_half_saved_recovery_are_safe"
  - breaker: A malformed waiting file is overwritten, a prior site field is ignored in the phone warning, or a failed second save loses the waiting request.
- AC-011: A real CLI test previews and saves an immediate schedule tightening, parks a locked loosening, lists it and cancels or settles it; it checks the edited zip and unchanged unrelated entries.
  - evidence: cli/tests/real_schedule_edit.rs "the_real_program_previews_saves_and_queues_schedule_edits"
  - breaker: Pure tests pass but the actual CLI skips the judge, writes during preview, or fails to preserve other zip entries.

## Checklist coverage
Not used: Domains is none; the derived checklist below stands in for it.

## Derived checklist
- D-1: Preview shows the exact rule change and writes nothing -> criterion:AC-001
- D-2: Only the named hours rule changes -> criterion:AC-002
- D-3: The possible overnight comparison gap is measured -> criterion:AC-003
- D-4: Closed-to-open changes cannot bypass the safety comparison -> criterion:AC-004
- D-5: Block and routine locks govern possible loosenings -> criterion:AC-005
- D-6: Writes are validated, atomic or safely queued -> criterion:AC-006
- D-7: Older and unknown waiting formats are handled safely -> criterion:AC-007
- D-8: Settlement rechecks rule preconditions and current locks -> criterion:AC-008
- D-9: Waiting changes are visible, cancellable and nonconflicting -> criterion:AC-009
- D-10: Failures and phone compatibility are reported honestly -> criterion:AC-010

## Edge cases
- An overnight 22:00–06:00 stretch when adding or removing its starting weekday -> AC-003
- 24:00 as an end, overlapping or equal stretches, an empty old list and `--days all` -> AC-002
- New York's skipped and repeated hours on the daylight-saving transition days -> AC-004
- Another edit changes the target rule or extends a routine lock while the request waits -> AC-008
- A zip saves but waiting-file cleanup fails -> AC-010
- Two processes write the same settings concurrently -> out-of-scope: cross-process serialization needs its own contract.

## Out of scope
- Creating or deleting rules, changing rule kinds, editing `always`, `everyOpen`, `minutes`, `opens` or timed `session` rules, and changing a session seal.
- Editing block coverage, names, doors, passes or locks; partner-key entry; automatic settlement; phone synchronization.
- Changing the system clock or putting the user's Mac to sleep in automated tests; zone transitions use fixed instants.

## Decisions
- Edit one existing outside-hours rule by ID | avoids silently changing other rules or a timed session's seal | REVERSIBLE
- Preserve omitted fields; use `--days all` for all weekdays | makes partial edits explicit and avoids treating an omitted flag as a reset | REVERSIBLE
- Treat anything not proven tightening as loosening | the existing comparator's handling of overnight weekday changes needs a measured safety check | REVERSIBLE
- Store the original target rule as a precondition plus its intended replacement | detects competing rule edits without replacing unrelated block fields | REVERSIBLE
- Retain preview and `--write`, one waiting change per block, and manual settlement | matches the existing editing workflow | REVERSIBLE
