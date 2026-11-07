use leko_core::edit_schedule::schedule_safety::is_schedule_loosening;
use serde_json::json;

#[test]
fn schedule_comparison_never_misses_a_closed_to_open_change() {
    // 1. Overnight weekday addition (the regression gap):
    // Old: Monday only, 22:00-06:00
    // New: Monday and Sunday, 22:00-06:00
    // Opens Monday 01:00 AM (which was closed under old rule).
    let old_overnight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "22:00", "to": "06:00"}]
    });
    let new_overnight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "sun"],
        "allowed": [{"from": "22:00", "to": "06:00"}]
    });
    assert!(is_schedule_loosening(&old_overnight, &new_overnight), "Adding overnight weekday must be caught as loosening");

    // 2. Strict tightening: narrowing open window from 09:00-17:00 to 10:00-16:00
    let old_tight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "tue"],
        "allowed": [{"from": "09:00", "to": "17:00"}]
    });
    let new_tight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "tue"],
        "allowed": [{"from": "10:00", "to": "16:00"}]
    });
    assert!(!is_schedule_loosening(&old_tight, &new_tight), "Narrowing allowed window must be recognized as tightening");

    // 3. Widening open window: 09:00-17:00 to 08:00-18:00
    let new_wide = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon", "tue"],
        "allowed": [{"from": "08:00", "to": "18:00"}]
    });
    assert!(is_schedule_loosening(&old_tight, &new_wide), "Widening allowed window is loosening");

    // 4. Removing weekdays leaves app open on that day (loosening)
    let new_fewer_days = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "09:00", "to": "17:00"}]
    });
    assert!(is_schedule_loosening(&old_tight, &new_fewer_days), "Removing weekdays leaves app open, so it is loosening");

    // 5. Handling 24:00 as an end
    let old_midnight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "20:00", "to": "24:00"}]
    });
    let new_midnight_tight = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "21:00", "to": "24:00"}]
    });
    assert!(!is_schedule_loosening(&old_midnight, &new_midnight_tight), "Narrowing with 24:00 end is tightening");

    let new_midnight_wide = json!({
        "id": "h",
        "kind": "outsideHours",
        "days": ["mon"],
        "allowed": [{"from": "19:00", "to": "24:00"}]
    });
    assert!(is_schedule_loosening(&old_midnight, &new_midnight_wide), "Widening with 24:00 end is loosening");
}
