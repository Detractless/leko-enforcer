use chrono_tz::UTC;
use leko_core::domain::{pass_active, PassState};
use leko_core::services::{wait_for_requirement, RecordingSleeper, Sleeper};
use leko_core::models::{
    Pass, PassCap, PassLength, PassRequirement, Penalty, Schedule, Seal, Target,
    TechnologyManagementRule,
};
use leko_core::Moment;

fn with_pass(pass: Pass) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "rule-1".into(),
        name: "Study".into(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["a".into()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass,
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn earn(length: PassLength, cap: PassCap) -> Pass {
    Pass::EarnPass {
        requirement: PassRequirement::PhysicalPass,
        length,
        cap,
    }
}

#[test]
fn an_earned_pass_expires_at_its_length_and_respects_its_cap() {
    let timed = earn(PassLength::TimedPass { minutes: 30 }, PassCap::UnlimitedPasses);
    let state = PassState {
        earned_at: Some("2026-01-01T00:00:00Z".into()),
        used_in_window: 0,
        window_key: "2026-01-01".into(),
    };
    let during = Moment::parse("2026-01-01T00:29:00Z", UTC).unwrap();
    let at_end = Moment::parse("2026-01-01T00:30:00Z", UTC).unwrap();
    let before = Moment::parse("2025-12-31T23:59:00Z", UTC).unwrap();
    assert!(pass_active(&timed, &state, &during));
    assert!(!pass_active(&timed, &state, &at_end));
    assert!(!pass_active(&timed, &state, &before));
    assert!(!pass_active(&Pass::NoPass, &state, &during));

    let no_earn = PassState { earned_at: None, ..state.clone() };
    assert!(!pass_active(&timed, &no_earn, &during));

    let capped = earn(
        PassLength::TimedPass { minutes: 30 },
        PassCap::LimitedPasses { max_per_window: 2 },
    );
    let at_cap = PassState { used_in_window: 3, ..state.clone() };
    assert!(!pass_active(&capped, &at_cap, &during));
    assert!(pass_active(&capped, &state, &during));
    let fresh_window = PassState { window_key: "2025-12-31".into(), used_in_window: 9, ..state.clone() };
    assert!(pass_active(&capped, &fresh_window, &during));

    let rest = earn(PassLength::RestOfDayPass, PassCap::UnlimitedPasses);
    let evening = Moment::parse("2026-01-01T23:00:00Z", UTC).unwrap();
    let next_day = Moment::parse("2026-01-02T00:00:00Z", UTC).unwrap();
    assert!(pass_active(&rest, &state, &evening));
    assert!(!pass_active(&rest, &state, &next_day));

    let rule = with_pass(timed.clone());
    let before_json = serde_json::to_string(&rule).unwrap();
    let _ = pass_active(&rule.pass, &state, &during);
    assert_eq!(serde_json::to_string(&rule).unwrap(), before_json);
}

#[test]
fn countdown_wait_is_enforced_with_an_injected_sleeper() {
    let requirement = PassRequirement::CountdownPass { wait_seconds: 5 };
    let mut sleeper = RecordingSleeper::default();
    wait_for_requirement(&requirement, &mut sleeper).unwrap();
    assert_eq!(sleeper.waited, vec![5000]);
    assert_eq!(sleeper.waited_ms(), 5000);

    let mut immediate = RecordingSleeper::default();
    wait_for_requirement(&PassRequirement::TypingPass, &mut immediate).unwrap();
    assert!(immediate.waited.is_empty());
}

#[test]
fn engagement_pass_grants_after_required_focus_duration() {
    use leko_core::services::{
        advance_rule_usage, record_pass, reset_engagement, PassStateFile, UsageStateFile,
    };
    let rule = with_pass(Pass::EarnPass {
        requirement: PassRequirement::EngagementPass {
            targets: vec!["com.example.Reader".into()],
            required_seconds: 60,
        },
        length: PassLength::TimedPass { minutes: 10 },
        cap: PassCap::LimitedPasses { max_per_window: 1 },
    });
    let mut usage = UsageStateFile::default();
    let mut passes = PassStateFile::default();
    let t0 = Moment::parse("2026-01-01T12:00:00Z", UTC).unwrap();
    let t1 = Moment::parse("2026-01-01T12:00:40Z", UTC).unwrap();
    let t2 = Moment::parse("2026-01-01T12:01:05Z", UTC).unwrap();

    assert!(!advance_rule_usage(&mut usage, &rule, &t0, false, true, false, 5, 300).grant_engagement_pass);
    assert!(!advance_rule_usage(&mut usage, &rule, &t1, false, true, false, 5, 300).grant_engagement_pass);
    assert!(advance_rule_usage(&mut usage, &rule, &t2, false, true, false, 5, 300).grant_engagement_pass);

    record_pass(&mut passes, &rule, &t2, "2026-01-01".into()).unwrap();
    reset_engagement(&mut usage, &rule.id);
    let domain_pass = leko_core::services::domain_state(&passes, &rule.id);
    assert!(pass_active(&rule.pass, &domain_pass, &t2));
    assert!(record_pass(&mut passes, &rule, &t2, "2026-01-01".into()).is_err());
}
