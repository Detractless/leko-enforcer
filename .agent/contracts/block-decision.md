# Contract: block-decision

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Given the settings that were read, one app or one website, the current moment and a time zone, the core says whether a block closes it right now, which blocks and rules close it, and the next moment at which anything could change. Only the time rules close here (always, every open, outside hours, session); nothing is closed on a real Mac yet.

## Domains
Domains: none

## Acceptance criteria
- AC-001: A block with `"on": false` closes nothing and is not mentioned in the answer; a block with no `on` field is on, as on the phone; an app or site that no block covers is answered open with nothing listed.
  - evidence: core/tests/decision_cover.rs "an_off_block_closes_nothing_and_a_missing_on_means_on"
  - breaker: A switched-off block still closes its apps, or a block with no `on` field is treated as off.
- AC-002: An `always` or `everyOpen` rule closes a covered target on the weekdays it names, none named meaning every day, and the weekday is the one in the given time zone, not in UTC.
  - evidence: core/tests/decision_rules.rs "always_and_every_open_close_on_the_named_weekdays_in_the_given_zone"
  - breaker: Monday's rule closes on Tuesday, an empty day list closes nothing, or 23:30 on Sunday in New York is judged as Monday because UTC has turned over.
- AC-003: An `outsideHours` rule closes a covered target at every local minute outside its allowed stretches, and follows the phone's cases: a stretch that crosses midnight belongs to the day it starts on, its part after midnight is judged by that day's rule, an empty allowed list closes all day, and a stretch ending at 24:00 closes at midnight.
  - evidence: core/tests/decision_rules.rs "outside_hours_follows_the_phone_across_midnight_and_weekdays"
  - breaker: Monday 23:00 is closed inside a 22:00 to 06:00 stretch, Tuesday 03:00 is closed although it is the end of Monday's stretch, Monday 01:00 is open because Sunday's stretch was assumed, or 23:59 is closed for a stretch that runs to 24:00.
- AC-004: A `session` rule closes a covered target at every instant before its end moment and not at or after it, on every weekday: one millisecond before the end is closed and the end itself is open.
  - evidence: core/tests/decision_rules.rs "a_session_closes_until_its_end_moment_and_not_after"
  - breaker: The session is still closed at its end moment, opens one second early, or is judged by weekday.
- AC-005: On the 23-hour and 25-hour days of a clock change, outside-hours rules keep following local clock minutes (America/New_York on 2026-03-08 and 2026-11-01, with the phone's own cases), and a stretch whose edge falls in the skipped hour is judged by the local clock after the jump.
  - evidence: core/tests/decision_rules.rs "clock_changes_keep_local_minutes"
  - breaker: A rule is judged in UTC or by a fixed offset, so on the day of a clock change it closes or opens one hour wrong.
- AC-006: Coverage: `apps` covers only the apps it names; `allAppsExcept` covers every app that is not listed; an app in the never-blocked set handed in is open under every block and rule, a session included; `sites` cover a host that is a listed name or a subdomain of one ("notinstagram.com" and "instagram.com.example.org" are not covered, upper case and one trailing dot are ignored); an app name is never matched against sites and a host never against apps.
  - evidence: core/tests/decision_cover.rs "named_apps_all_except_and_sites_decide_what_a_block_covers"
  - breaker: An all-except block closes a never-blocked app, a look-alike host is closed, or a host named like an app is closed through `apps`.
- AC-007: A closed answer lists every closing block and rule (block id, rule id, kind): blocks in the order of their ids, rules as written in the file; two blocks that close the same target are both listed, two rules of one block are both listed, and the target is closed exactly when the list is not empty. The same settings, target, moment and zone always give the same answer, whatever order the files were read in.
  - evidence: core/tests/decision_report.rs "the_answer_names_every_closing_block_and_rule_in_a_fixed_order"
  - breaker: Only the first closing rule is listed, the order depends on the order of the zip, or `closed` is true with an empty list.
- AC-008: A `minutes` or `opens` rule on a block that covers the target is listed as "not judged" and never counts as open: a target covered only by such rules is answered open with those rules named under not judged, and when a time rule also closes it, both lists are filled.
  - evidence: core/tests/decision_report.rs "minutes_and_opens_rules_are_reported_as_not_judged_never_as_open"
  - breaker: A minutes rule is silently ignored, so the caller reads "open" as "nothing here applies", or it is counted as spent without any usage figures.
- AC-009: The next change is the first moment after the given one at which the closing state of any rule of an on block differs, looking 8 days ahead by the minute and at every session end, and it is exactly that moment (a stretch edge is that minute, a session end is that second, a clock-change jump is the jump); when nothing changes within 8 days the answer is none, and at the largest moment the core can count it is none and does not overflow.
  - evidence: core/tests/decision_next_change.rs "the_next_change_is_the_first_moment_the_closing_state_differs"
  - breaker: The next change is later than a real change (an app stays open after its hours end), a weekly rule is missed because the look-ahead is under 7 days, a clock-change jump is missed, or a far-away moment crashes with an overflow.
- AC-010: The settings readers keep what the decision needs (the `on` flag, which apps or sites a block covers, the weekdays, the allowed stretches and a session's end moment), and nothing that the readers accepted or refused before is accepted or refused differently.
  - evidence: core/tests/decision_reading.rs "the_readers_keep_what_the_decision_needs"
  - breaker: A kept field is read wrongly (a weekday list of seven becomes "mon"), or a change to the readers lets a file through that was refused.
- AC-011: The decision code never reads the clock, the environment or the time zone of the computer: the moment and the zone are arguments, as in the rest of the core.
  - evidence: core/tests/no_clock_reads.rs "only_the_time_module_reads_the_clock"
  - breaker: A new file in the core reads the clock, so tests can no longer fix the moment.

## Checklist coverage
Not used: Domains is none; the derived checklist below stands in for it.

## Derived checklist
- D-1: A block that is switched off never closes anything, and a missing switch means on -> criterion:AC-001
- D-2: Weekday rules are judged by the day in the user's zone, not in UTC -> criterion:AC-002
- D-3: Hours that cross midnight are judged the way the phone judges them -> criterion:AC-003
- D-4: A session ends at exactly its end moment -> criterion:AC-004
- D-5: Clock changes never move a rule by an hour -> criterion:AC-005
- D-6: Coverage never leaks: a never-blocked app stays open, and apps and sites are not mixed up -> criterion:AC-006
- D-7: The answer says why, in a fixed order -> criterion:AC-007
- D-8: A rule that cannot be judged yet is said so and never counted as open -> criterion:AC-008
- D-9: The wake-up moment is never later than a real change -> criterion:AC-009
- D-10: The core never reads the clock -> criterion:AC-011

## Edge cases
- No blocks at all, only switched-off blocks, or a target that no block names -> AC-001
- An outside-hours rule with an empty allowed list, a stretch ending at 24:00, and a stretch 22:00 to 06:00 on Monday only judged on Monday 01:00 and Tuesday 03:00 -> AC-003
- A session ending exactly at the given moment, one millisecond later, and nine days later -> AC-004
- 2026-03-08 and 2026-11-01 in America/New_York, with a stretch from 02:30 that starts in the skipped hour -> AC-005
- An app listed in `allAppsExcept` and also in the never-blocked set, a host with capitals and a trailing dot, "notinstagram.com" -> AC-006
- A target closed by two blocks and by two rules of one block -> AC-007
- A block whose only rule counts minutes -> AC-008
- Settings where nothing ever changes (an `always` rule on every day), and the largest moment the core can count -> AC-009
- Leap seconds, and zone offsets that are not whole minutes (very old dates) -> out-of-scope: the look-ahead steps by whole UTC minutes and uses current zone data; tests use modern dates

## Out of scope
- Whether the user may get through a closure: passes, doors, walls, routine holds and locks (the next contracts; Enforcer's PassBook, DoorSessions, RoutineHolds).
- Counting use for `minutes` and `opens` rules (needs the macOS usage reader).
- Reading the real clock, the computer's zone, the settings file from disk, the list of running apps, and closing anything (the macOS ports).
- A command to ask the question (`leko status`, Phase 3) and the take-over screen text.
- Turning Android package names into Mac bundle ids, and the shared catalog (the open "Gap A" in the feature map); apps are matched here by the exact name written in the block.
- The real list of Mac apps that must never be blocked (Finder, the login window, the blocker itself); here it is only an argument.
- Run-time exemptions (Enforcer's Exemptions) and the order of reasons on the block screen.

## Decisions
- A pure function in a new folder `core/src/decision/`, taking the read settings, one target (an app or a host), the moment in milliseconds, the zone and the never-blocked set | the core stays free of the clock and the operating system, and a test can fix every input | REVERSIBLE
- Core only: no command line yet | a command needs the clock, the zone and file reading at the edge, which is its own piece of work | REVERSIBLE
- Only always, everyOpen, outsideHours and session close here; minutes and opens are returned as "not judged" | their usage counts do not exist yet, and "not judged" is safer than "open" | REVERSIBLE
- `everyOpen` closes like `always` here | the two differ only when a pass is used, and passes are not here | REVERSIBLE
- The readers keep the weekdays, stretches, covers, `on` and a session's end, by adding fields to the existing `Rule` and `BlockSpec` | the decision cannot use what was thrown away, and the file must not be parsed a second time | REVERSIBLE
- The next change is found by looking at every whole minute for 8 days, plus each session end | exact across clock changes without copying the zone rules, and cheap (about 11,500 steps) | REVERSIBLE
- Apps match by the exact name in `apps` | how a Mac app is named in a shared block is the later "Gap A" decision | REVERSIBLE
