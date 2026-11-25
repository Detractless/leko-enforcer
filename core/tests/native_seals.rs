use chrono_tz::UTC;
use leko_core::domain::{
    allow_edit, queued_edits_due, seal_satisfied, ChangeDirection, EditDecision, ProposedEdit,
    SealProof,
};
use leko_core::models::{
    CustomVerifierSpec, Pass, Penalty, QueuedEdit, Schedule, Seal, Target, TechnologyManagementRule,
    TypingMode, Window,
};
use leko_core::Moment;

fn sealed(seal: Seal) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "rule-1".into(),
        name: "Study".into(),
        enabled: true,
        target: Target::SpecificApps { apps: vec!["a".into()] },
        schedule: Schedule::default(),
        penalty: Some(Penalty::ForceClosePenalty),
        pass: Pass::default(),
        seal,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn edit(direction: ChangeDirection) -> ProposedEdit {
    ProposedEdit { field: "penalty".into(), direction }
}

#[test]
fn tightening_lands_now_and_weakening_waits_for_the_seal() {
    let moment = Moment::parse("2026-01-01T00:00:30Z", UTC).unwrap();
    let proof = SealProof::default();
    let rule = sealed(Seal::CountdownSeal {
        wait_seconds: 60,
        unseal_requested_at: None,
    });

    assert_eq!(
        allow_edit(&rule, &edit(ChangeDirection::Tighten), &moment, &proof),
        EditDecision::Allowed
    );
    assert_eq!(
        allow_edit(&rule, &edit(ChangeDirection::Weaken), &moment, &proof),
        EditDecision::Blocked
    );
    assert_eq!(
        allow_edit(&rule, &edit(ChangeDirection::Pause), &moment, &proof),
        EditDecision::Blocked
    );
    assert_eq!(
        allow_edit(&rule, &edit(ChangeDirection::Delete), &moment, &proof),
        EditDecision::Blocked
    );

    let satisfied = sealed(Seal::CountdownSeal {
        wait_seconds: 60,
        unseal_requested_at: Some("2026-01-01T00:00:00Z".into()),
    });
    let later = Moment::parse("2026-01-01T00:01:00Z", UTC).unwrap();
    assert_eq!(
        allow_edit(&satisfied, &edit(ChangeDirection::Weaken), &later, &proof),
        EditDecision::Allowed
    );

    let queued = sealed(Seal::QueuedDelaySeal {
        delay_seconds: 60,
        pending_edits: vec![],
    });
    assert_eq!(
        allow_edit(&queued, &edit(ChangeDirection::Weaken), &moment, &proof),
        EditDecision::Queued
    );
}

#[test]
fn each_seal_declaration_matches_and_a_queued_edit_can_be_cancelled() {
    let spec = CustomVerifierSpec {
        addon_id: "checker".into(),
        sha256: "ab".repeat(32),
        config: serde_json::json!({}),
    };
    let seals = vec![
        Seal::NoSeal,
        Seal::CountdownSeal { wait_seconds: 30, unseal_requested_at: None },
        Seal::QueuedDelaySeal { delay_seconds: 60, pending_edits: vec![] },
        Seal::TypingSeal { mode: TypingMode::RandomText, length: 10 },
        Seal::TaskSeal { task_spec: "walk".into() },
        Seal::EditWindowSeal { days: vec![1], windows: vec![Window { start_minute: 0, end_minute: 60 }] },
        Seal::ActiveScheduleSeal { buffer_minutes: 15 },
        Seal::PartnerPasswordSeal { password_hash: "hash".into() },
        Seal::CustomVerifier { spec: spec.clone(), fallback_seal: None },
    ];
    for seal in seals {
        let value = serde_json::to_value(&seal).unwrap();
        assert!(value.get("kind").is_some());
    }

    let moment = Moment::parse("2026-01-01T00:01:00Z", UTC).unwrap();
    let pending = QueuedEdit {
        key: "name".into(),
        value: "New name".into(),
        requested_at: "2026-01-01T00:00:00Z".into(),
        cancelled: false,
    };
    let seal = Seal::QueuedDelaySeal { delay_seconds: 60, pending_edits: vec![pending.clone()] };
    assert_eq!(queued_edits_due(&seal, &moment).len(), 1);

    let cancelled = Seal::QueuedDelaySeal {
        delay_seconds: 60,
        pending_edits: vec![QueuedEdit { cancelled: true, ..pending }],
    };
    assert!(queued_edits_due(&cancelled, &moment).is_empty());
    assert!(!seal_satisfied(&cancelled, &moment, &SealProof::default()));

    let no_fallback = Seal::CustomVerifier { spec: spec.clone(), fallback_seal: None };
    assert!(!seal_satisfied(&no_fallback, &moment, &SealProof::default()));
    let fallback = Seal::CustomVerifier { spec, fallback_seal: Some(Box::new(Seal::NoSeal)) };
    assert!(seal_satisfied(&fallback, &moment, &SealProof::default()));
    let with_proof = SealProof { verifier_satisfied: true, ..SealProof::default() };
    assert!(seal_satisfied(&no_fallback, &moment, &with_proof));
}
