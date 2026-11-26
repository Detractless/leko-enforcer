use chrono_tz::{America::New_York, UTC};
use leko_core::domain::{is_time_active, usage_allows, UsageState};
use leko_core::models::{Period, TimeBasedSchedule, UsageLimit, Window};
use leko_core::Moment;

#[test]
fn windows_cross_midnight_and_follow_the_zone_with_an_injected_clock() {
    let overnight = TimeBasedSchedule::RoutineSchedule {
        days: vec![1, 2, 3, 4, 5, 6, 7],
        windows: vec![Window { start_minute: 23 * 60, end_minute: 24 * 60 + 60 }],
    };
    let inside = Moment::parse("2026-01-01T00:30:00Z", UTC).unwrap();
    let outside = Moment::parse("2026-01-01T02:00:00Z", UTC).unwrap();
    assert!(is_time_active(&overnight, &inside));
    assert!(!is_time_active(&overnight, &outside));

    let local = TimeBasedSchedule::RoutineSchedule {
        days: vec![1, 2, 3, 4, 5, 6, 7],
        windows: vec![Window { start_minute: 9 * 60, end_minute: 17 * 60 }],
    };
    let nine_local = Moment::parse("2026-07-01T13:00:00Z", New_York).unwrap();
    let before_nine = Moment::parse("2026-07-01T12:59:00Z", New_York).unwrap();
    assert!(is_time_active(&local, &nine_local));
    assert!(!is_time_active(&local, &before_nine));

    let across_gap = TimeBasedSchedule::RoutineSchedule {
        days: vec![7],
        windows: vec![Window { start_minute: 90, end_minute: 210 }],
    };
    let after_gap = Moment::parse("2026-03-08T07:00:00Z", New_York).unwrap();
    assert!(is_time_active(&across_gap, &after_gap));

    let once = TimeBasedSchedule::OneTimeSchedule {
        start_at: "2026-01-01T00:00:00Z".into(),
        end_at: "2026-01-02T00:00:00Z".into(),
    };
    assert!(is_time_active(&once, &inside));
    assert!(!is_time_active(&once, &Moment::parse("2026-01-03T00:00:00Z", UTC).unwrap()));
    assert!(is_time_active(&TimeBasedSchedule::AlwaysOnSchedule, &outside));
}

#[test]
fn a_zero_budget_enforces_at_once_and_limits_reset_on_the_period() {
    let moment = Moment::parse("2026-01-01T12:00:00Z", UTC).unwrap();
    let state = UsageState {
        period_key: "2026-01-01".into(),
        used_minutes: 30,
        sessions: 2,
    };

    assert!(!usage_allows(None, &state, &moment));

    let minutes = UsageLimit::TimeLimit { minutes: 60, period: Period::Daily };
    assert!(usage_allows(Some(&minutes), &state, &moment));
    let exhausted = UsageState { used_minutes: 60, ..state.clone() };
    assert!(!usage_allows(Some(&minutes), &exhausted, &moment));

    let stale = UsageState {
        period_key: "2025-01-01".into(),
        used_minutes: 9999,
        sessions: 9999,
    };
    assert!(usage_allows(Some(&minutes), &stale, &moment));

    let sessions = UsageLimit::SessionLimit { max_sessions: 3, period: Period::Daily };
    assert!(usage_allows(Some(&sessions), &state, &moment));
    let used_sessions = UsageState { sessions: 3, ..state.clone() };
    assert!(!usage_allows(Some(&sessions), &used_sessions, &moment));
    assert!(usage_allows(Some(&sessions), &stale, &moment));
}

#[test]
fn usage_budgets_roll_over_and_debounce_flickers() {
    use leko_core::models::{Pass, Penalty, Schedule, Seal, Target, TechnologyManagementRule};
    use leko_core::services::{advance_rule_usage, domain_usage, UsageStateFile};
    let rule = TechnologyManagementRule {
        id: "r1".into(),
        name: "Focus".into(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["com.example.App".into()] },
        schedule: Schedule {
            time_based: TimeBasedSchedule::AlwaysOnSchedule,
            usage_based: Some(UsageLimit::SessionLimit { max_sessions: 2, period: Period::Daily }),
        },
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::NoPass,
        seal: Seal::NoSeal,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    };
    let mut state = UsageStateFile::default();
    let t0 = Moment::parse("2026-01-01T12:00:00Z", UTC).unwrap();
    let t1 = Moment::parse("2026-01-01T12:00:02Z", UTC).unwrap();
    let t2 = Moment::parse("2026-01-01T12:00:04Z", UTC).unwrap();
    let t3 = Moment::parse("2026-01-01T12:00:10Z", UTC).unwrap();
    let t4 = Moment::parse("2026-01-01T12:00:11Z", UTC).unwrap();
    let next_day = Moment::parse("2026-01-02T08:00:00Z", UTC).unwrap();

    advance_rule_usage(&mut state, &rule, &t0, true, false, false, 5, 300);
    advance_rule_usage(&mut state, &rule, &t1, false, false, false, 5, 300);
    advance_rule_usage(&mut state, &rule, &t2, true, false, false, 5, 300);
    assert_eq!(domain_usage(&state, "r1").0.sessions, 1, "2s Cmd-Tab flicker burned extra session");

    advance_rule_usage(&mut state, &rule, &t1, false, false, false, 5, 300);
    advance_rule_usage(&mut state, &rule, &t3, false, false, false, 5, 300);
    advance_rule_usage(&mut state, &rule, &t4, true, false, false, 5, 300);
    assert_eq!(domain_usage(&state, "r1").0.sessions, 2, "6s gap did not start a second session");

    advance_rule_usage(&mut state, &rule, &next_day, true, false, false, 5, 300);
    let (usage, seconds, open) = domain_usage(&state, "r1");
    assert_eq!(usage.period_key, "2026-01-02");
    assert_eq!(usage.sessions, 1);
    assert_eq!(seconds, 0);
    assert!(open);
}
