use chrono_tz::UTC;
use leko_core::domain::{
    allow_edit, decide, pass_active, ChangeDirection, EditDecision, PassState, ProposedEdit,
    SealProof, UsageState,
};
use leko_core::models::{
    Pass, PassCap, PassLength, PassRequirement, Penalty, Schedule, Seal, Target, TimeBasedSchedule,
    Window,
};
use leko_core::{Moment, TechnologyManagementRule};
use std::fs;
use std::path::Path;

fn rule(target: Target, time: TimeBasedSchedule, pass: Pass, seal: Seal) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "rule-1".into(),
        name: "Study".into(),
        enabled: true,
        target,
        schedule: Schedule { time_based: time, usage_based: None },
        penalty: Some(Penalty::ForceClosePenalty),
        pass,
        seal,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[test]
fn edge_cases_match_the_vectors_without_legacy_json_keys() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for file in [
        "vectors/canonical-cases.json",
        "vectors/key-vectors.json",
        "vectors/loosening-cases.json",
        "extension-vectors/seal-cases.json",
        "extension-vectors/session-cases.json",
        "extension-vectors/site-cases.json",
    ] {
        let text = fs::read_to_string(workspace.join(file)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(value.is_object(), "{} is not an object", file);
    }
    let reference = fs::read_to_string(workspace.join("vectors/loosening-cases.json")).unwrap();
    assert!(reference.contains("outsideHours"), "reference vectors lost the legacy case");

    let usage = UsageState { period_key: "2026-01-01".into(), used_minutes: 0, sessions: 0 };
    let pass = PassState { earned_at: None, used_in_window: 0, window_key: "2026-01-01".into() };
    let midnight = Moment::parse("2026-01-01T00:30:00Z", UTC).unwrap();

    let whole = rule(
        Target::WholeDevice { action: leko_core::models::DeviceAction::Lock },
        TimeBasedSchedule::AlwaysOnSchedule,
        Pass::NoPass,
        Seal::NoSeal,
    );
    let mut whole_without_penalty = whole.clone();
    whole_without_penalty.penalty = None;
    assert!(whole_without_penalty.validate().is_ok());
    assert!(decide(&whole_without_penalty, &midnight, &usage, &pass).enforced);

    let overnight = rule(
        Target::SpecificWebsites { sites: vec!["site.test".into()] },
        TimeBasedSchedule::RoutineSchedule {
            days: vec![1, 2, 3, 4, 5, 6, 7],
            windows: vec![Window { start_minute: 23 * 60, end_minute: 24 * 60 + 60 }],
        },
        Pass::NoPass,
        Seal::NoSeal,
    );
    assert!(decide(&overnight, &midnight, &usage, &pass).enforced);
    let midday = Moment::parse("2026-01-01T12:00:00Z", UTC).unwrap();
    assert!(!decide(&overnight, &midday, &usage, &pass).enforced);

    let weakening = ProposedEdit { field: "penalty".into(), direction: ChangeDirection::Weaken };
    let sealed = rule(
        Target::SpecificApps { apps: vec!["a".into()] },
        TimeBasedSchedule::AlwaysOnSchedule,
        Pass::NoPass,
        Seal::CountdownSeal { wait_seconds: 3600, unseal_requested_at: None },
    );
    assert_eq!(allow_edit(&sealed, &weakening, &midnight, &SealProof::default()), EditDecision::Blocked);

    let capped = Pass::EarnPass {
        requirement: PassRequirement::TypingPass,
        length: PassLength::TimedPass { minutes: 30 },
        cap: PassCap::LimitedPasses { max_per_window: 1 },
    };
    let earned = PassState {
        earned_at: Some("2026-01-01T00:00:00Z".into()),
        used_in_window: 1,
        window_key: "2026-01-01".into(),
    };
    assert!(!pass_active(&capped, &earned, &midnight));

    let text = serde_json::to_string(&whole_without_penalty).unwrap();
    for legacy in ["outsideHours", "everyOpen", "blocked_hours", "BlockSpec"] {
        assert!(!text.contains(legacy), "legacy key {} reached the native format", legacy);
    }
}
