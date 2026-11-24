# Leko user vocabulary

This is the canonical ubiquitous language for the native Leko rule engine. It replaces the legacy Cold Turkey terms. A `TechnologyManagementRule` is the one object the user creates and edits.

## Naming rules

- Rust types and enums use `PascalCase`.
- Rust fields, JSON keys and filenames use `snake_case`.
- CLI flags use `kebab-case`.
- No legacy names are allowed: `Door`, `Lock`, `BlockSpec`, `outsideHours`, `everyOpen` and `blocked_hours` are removed.
- Features marked `[PLANNED]` have no variant, field or flag until implemented.

## The five pillars

| Pillar | Name | Answers |
| --- | --- | --- |
| 1 | `Target` | What is watched |
| 2 | `Schedule` | When the rule is active |
| 3 | `Penalty` | What happens when it is breached, for apps and websites |
| 4 | `Pass` | A temporary bypass that never edits the rule |
| 5 | `Seal` | How the rule is protected from weakening, pausing or deletion |

## TechnologyManagementRule

Fields: `id`, `name`, `enabled`, `target`, `schedule`, `penalty`, `pass`, `seal`, `created_at`, `updated_at`.

## Pillar 1: Target

- `SpecificApps { apps }`
- `AllowOnlyApps { allowed_apps }`
- `SpecificWebsites { sites }`
- `TopicDetectedWebsites { topics, policy_packs }`
- `AllowOnlyWebsites { allowed_sites, allowed_topics }`
- `WholeDevice { action }` where `DeviceAction` is `Lock`, `Logoff`, `Restart` or `Shutdown`

Invariant: a `WholeDevice` rule has no `penalty` and enforces on every unlock, sign-in and boot. Apps and websites rules require `Some(penalty)`.

## Pillar 2: Schedule

`Schedule { time_based, usage_based }`.

`time_based` is one of:

- `AlwaysOnSchedule` (the default)
- `OneTimeSchedule { start_at, end_at }`
- `RoutineSchedule { days, windows }`, which wraps past midnight and follows a timezone

`usage_based` is `None`, meaning immediate zero-minute enforcement during the active `time_based`, or one of:

- `TimeLimit { minutes, period }`
- `SessionLimit { max_sessions, period }`

## Pillar 3: Penalty

`ForceClosePenalty` is the default for apps and websites. It enforces continuously on every reopen or revisit until the schedule ends, the usage budget resets or an active pass is earned.

## Pillar 4: Pass

- `NoPass` (the default)
- `EarnPass { requirement, length, cap }`

`PassRequirement` is `PhysicalPass`, `MentalPass`, `CountdownPass`, `TypingPass` or `CustomVerifier(spec)`.

`PassLength` is `TimedPass { minutes }` or `RestOfDayPass`.

`PassCap` is `LimitedPasses { max_per_window }` or `UnlimitedPasses` (the default).

A pass never edits the rule. It only suspends enforcement until it expires or its cap is reached.

## Pillar 5: Seal

- `NoSeal` (the default)
- `CountdownSeal { wait_seconds, unseal_requested_at }`
- `QueuedDelaySeal { delay_seconds, pending_edits }`, which holds one tweak that can be cancelled before it applies
- `TypingSeal { mode, length }`
- `TaskSeal { task_spec }`
- `EditWindowSeal { days, windows }`
- `ActiveScheduleSeal { buffer_minutes }`
- `PartnerPasswordSeal { password_hash }`
- `CustomVerifier { spec, fallback_seal }`

Invariant: tightening a rule is always allowed immediately. Weakening, pausing or deleting a rule is blocked unless the seal is satisfied or the change is queued under `QueuedDelaySeal`.

## CustomVerifierSpec

Fields: `addon_id`, `sha256`, `config`.

A sealed add-on cannot be disabled, edited or deleted to bypass a rule. The hash is pinned, the runner snapshot is daemon-owned and read-only, and every attempt carries a fresh `nonce`. Evaluation is fail-closed: a missing runner, hash mismatch, invalid nonce or runtime error always means `NotSatisfied`.

## Planned, not built

`PomodoroSchedule`, `CommunityLinkedApps`, `CommunityLinkedWebsites`, `EngagementPass` and `RestartSeal` are `[PLANNED]`. They are not represented in code until implemented.
