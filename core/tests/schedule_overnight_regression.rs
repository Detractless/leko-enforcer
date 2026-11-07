use leko_core::config_diff::rule_compare::loosens;
use leko_core::config_time::zone_time::ZoneTime;
use leko_core::decision::rule_closes::{rule_state, LocalMoment, RuleState};
use leko_core::models::specs::Rule;
use serde_json::json;

#[test]
fn overnight_weekday_change_is_checked_against_real_decisions() {
    // AC-003: Reproduces the overnight schedule gap in rule_compare.
    // An outsideHours rule from 22:00 to 06:00 (1320 to 360 minutes).
    // Old rule applies only on Monday.
    // New rule applies on Monday and Sunday.
    let old_json = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "22:00", "to": "06:00"}]
    });
    let new_json = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "sun"],
        "allowed": [{"from": "22:00", "to": "06:00"}]
    });

    // The existing comparator inspects days subset (["mon"] is subset of ["mon", "sun"])
    // and allowed_mask subset. It reports FALSE (not loosening, i.e. tightening).
    let comparator_loosens = loosens(&old_json, &new_json);
    assert!(!comparator_loosens, "rule_compare reports false (tightening) because days expanded");

    // But check the actual real decision at Monday 01:00 AM UTC:
    // 2026-11-09 is a Monday. 01:00 UTC = 1794186000000 ms.
    let zone = ZoneTime::new("UTC").unwrap();
    let monday_0100 = LocalMoment::new(1_794_186_000_000, &zone);

    let old_rule = Rule {
        id: "h".into(),
        kind: "outsideHours".into(),
        days: vec!["mon".into()],
        allowed: vec![(1320, 360)],
        ends_at: None,
    };
    let new_rule = Rule {
        id: "h".into(),
        kind: "outsideHours".into(),
        days: vec!["mon".into(), "sun".into()],
        allowed: vec![(1320, 360)],
        ends_at: None,
    };

    let old_decision = rule_state(&old_rule, &monday_0100);
    let new_decision = rule_state(&new_rule, &monday_0100);

    // Under old rule: Monday 01:00 is CLOSED because Sunday was not in days, so no overnight carryover.
    assert_eq!(old_decision, RuleState::Closed, "Monday 01:00 was closed under old rule");

    // Under new rule: Sunday 22:00 carries over into Monday until 06:00, so Monday 01:00 is OPEN!
    assert_eq!(new_decision, RuleState::Open, "Monday 01:00 became open under new rule");

    // Proved: adding Sunday opened time on Monday that was previously closed,
    // while the naive rule comparator called it tightening.
    // Therefore, schedule safety must verify real closed-to-open decisions across all minutes.
}
