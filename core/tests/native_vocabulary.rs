use leko_core::models::{
    CustomVerifierSpec, DeviceAction, Pass, Penalty, Schedule, Seal, Target, TypingMode, Window,
};
use leko_core::TechnologyManagementRule;

fn sample(target: Target, penalty: Option<Penalty>) -> TechnologyManagementRule {
    TechnologyManagementRule {
        id: "rule-1".to_string(),
        name: "Study".to_string(),
        enabled: true,
        target,
        schedule: Schedule::default(),
        penalty,
        pass: Pass::default(),
        seal: Seal::default(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

#[test]
fn every_pillar_type_and_json_key_uses_the_canonical_names() {
    let targets = vec![
        Target::SpecificApps { apps: vec!["a".into()] },
        Target::AllowOnlyApps { allowed_apps: vec!["a".into()] },
        Target::SpecificWebsites { sites: vec!["site.test".into()] },
        Target::TopicDetectedWebsites {
            topics: vec!["study".into()],
            policy_packs: vec!["pack".into()],
        },
        Target::AllowOnlyWebsites {
            allowed_sites: vec!["site.test".into()],
            allowed_topics: vec!["study".into()],
        },
        Target::WholeDevice { action: DeviceAction::Lock },
    ];
    for target in targets {
        let rule = sample(target, Some(Penalty::ForceClosePenalty));
        let value = serde_json::to_value(&rule).unwrap();
        let object = value.as_object().unwrap();
        for key in [
            "id", "name", "enabled", "target", "schedule", "penalty", "pass", "seal",
            "created_at", "updated_at",
        ] {
            assert!(object.contains_key(key), "missing key {}", key);
        }
        let text = value.to_string();
        for legacy in ["Door", "Lock", "BlockSpec", "outsideHours", "everyOpen", "blocked_hours"] {
            assert!(!text.contains(legacy), "legacy name {} leaked", legacy);
        }
    }

    assert_eq!(serde_json::to_value(Penalty::ForceClosePenalty).unwrap(), "force_close_penalty");
    assert_eq!(serde_json::to_value(DeviceAction::Restart).unwrap(), "restart");
    assert_eq!(serde_json::to_value(Pass::NoPass).unwrap()["kind"], "no_pass");
    assert_eq!(serde_json::to_value(Seal::NoSeal).unwrap()["kind"], "no_seal");

    let spec = CustomVerifierSpec {
        addon_id: "checker".into(),
        sha256: "ab".repeat(32),
        config: serde_json::json!({"mode": "strict"}),
    };
    let seals = vec![
        Seal::CountdownSeal { wait_seconds: 30, unseal_requested_at: None },
        Seal::QueuedDelaySeal { delay_seconds: 60, pending_edits: vec![] },
        Seal::TypingSeal { mode: TypingMode::ExactText, length: 12 },
        Seal::TaskSeal { task_spec: "walk".into() },
        Seal::EditWindowSeal { days: vec![1], windows: vec![Window { start_minute: 60, end_minute: 120 }] },
        Seal::ActiveScheduleSeal { buffer_minutes: 15 },
        Seal::PartnerPasswordSeal { password_hash: "hash".into() },
        Seal::CustomVerifier { spec, fallback_seal: None },
    ];
    for seal in seals {
        let value = serde_json::to_value(&seal).unwrap();
        assert!(value.get("kind").is_some(), "seal variant lost its kind tag");
    }

    let original = sample(Target::SpecificApps { apps: vec!["a".into()] }, Some(Penalty::ForceClosePenalty));
    let bytes = serde_json::to_vec(&original).unwrap();
    let back: TechnologyManagementRule = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back, original);
    let engagement = leko_core::models::Pass::EarnPass {
        requirement: leko_core::models::PassRequirement::EngagementPass {
            targets: vec!["kindle".into()],
            required_seconds: 120,
        },
        length: leko_core::models::PassLength::TimedPass { minutes: 10 },
        cap: leko_core::models::PassCap::UnlimitedPasses,
    };
    let pass_json = serde_json::to_value(&engagement).unwrap();
    assert_eq!(pass_json["kind"], "earn_pass");
    assert!(pass_json["requirement"]["engagement_pass"].is_object());
}

#[test]
fn whole_device_rejects_a_penalty_and_apps_and_sites_require_one() {
    let whole_with_penalty = sample(
        Target::WholeDevice { action: DeviceAction::Shutdown },
        Some(Penalty::ForceClosePenalty),
    );
    assert!(whole_with_penalty.validate().is_err());

    let whole = sample(Target::WholeDevice { action: DeviceAction::Logoff }, None);
    assert!(whole.validate().is_ok());

    let watched = vec![
        Target::SpecificApps { apps: vec!["a".into()] },
        Target::AllowOnlyApps { allowed_apps: vec!["a".into()] },
        Target::SpecificWebsites { sites: vec!["site.test".into()] },
        Target::TopicDetectedWebsites { topics: vec!["study".into()], policy_packs: vec![] },
        Target::AllowOnlyWebsites { allowed_sites: vec![], allowed_topics: vec!["study".into()] },
    ];
    for target in watched {
        assert!(sample(target.clone(), None).validate().is_err());
        assert!(sample(target, Some(Penalty::ForceClosePenalty)).validate().is_ok());
    }
}

#[test]
fn no_planned_stubs_and_no_overridable_flags() {
    fn scan(directory: &std::path::Path, hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, hits);
            } else if path.extension().map(|value| value == "rs").unwrap_or(false) {
                let text = std::fs::read_to_string(&path).unwrap();
                for term in [
                    "PomodoroSchedule",
                    "CommunityLinkedApps",
                    "CommunityLinkedWebsites",
                    "RestartSeal",
                    "shadow_mode",
                    "nsfw",
                ] {
                    if text.contains(term) {
                        hits.push(format!("{}:{}", path.display(), term));
                    }
                }
            }
        }
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut hits = Vec::new();
    scan(&root, &mut hits);
    assert!(hits.is_empty(), "planned stubs or flags found: {:?}", hits);
}
