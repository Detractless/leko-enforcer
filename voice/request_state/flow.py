"""Deterministic slot merging and one-question request progression."""
from copy import deepcopy
import re

try:
    from ..intent_validation import (_normalize_days, _requested_days, _valid_duration,
                                     _valid_window, validate_intent)
except ImportError:
    from intent_validation import (_normalize_days, _requested_days, _valid_duration,
                                   _valid_window, validate_intent)


_SUPPORTED_ACTIONS = {"add_block", "start_session", "switch_off", "check_status"}


def _question(action, slots):
    if action == "add_block":
        apps, sites, device = slots.get("apps", []), slots.get("sites", []), slots.get("device")
        kind = slots.get("target_kind")
        if device and (apps or sites):
            return "target", "Should I block the whole computer or only the selected apps and websites?"
        if not device and not apps and not sites:
            if kind == "device":
                return "device", "Should I lock, log out, or shut down the whole computer?"
            if kind == "app":
                return "apps", "Which app should I block?"
            if kind == "site":
                return "sites", "Which website should I block?"
            return "target", "Should I block an app, a website, or the whole computer?"
        if kind == "device" and not device:
            return "device", "Should I lock, log out, or shut down the whole computer?"
        if kind == "app" and not apps:
            return "apps", "Which app should I block?"
        if kind == "site" and not sites:
            return "sites", "Which website should I block?"
        schedule = slots.get("schedule_type")
        if not schedule:
            return "schedule_type", "Should the block apply all the time, for a duration, or during specific hours?"
        if schedule == "for_duration" and not slots.get("duration"):
            return "duration", "How long should the block last?"
        if schedule == "open_hours" and not slots.get("open_hours"):
            return "open_hours", "During which hours should the target remain usable?"
        if schedule == "blocked_hours":
            if not slots.get("blocked_hours"):
                return "blocked_hours", "What hours should the target be blocked?"
            if not slots.get("days"):
                return "days", "Which days should I block those hours? You can say every day, weekdays, weekends, or named days."
    elif action == "start_session" and not slots.get("length"):
        return "length", "How long should the focus session last?"
    elif action == "switch_off" and not slots.get("id"):
        return "id", "Which block should I turn off?"
    elif action == "check_status":
        if slots.get("target_type") not in {"app", "site", "device"}:
            return "target_type", "Should I check an app, a website, or the whole computer?"
        if slots.get("target_type") != "device" and not slots.get("name"):
            return "name", "Which app or website should I check?"
    return None, None


def _default_name(action, slots):
    if action != "add_block" or slots.get("name"):
        return
    if slots.get("device"):
        label = {"lock": "Computer Lock", "logoff": "Computer Log Out",
                 "shutdown": "Computer Shutdown"}[slots["device"]]
    else:
        targets = slots.get("apps") or slots.get("sites") or []
        label = ", ".join(targets[:2]) if targets else "Distraction Block"
    slots["name"] = f"Block {label}"[:80]


def _rejected_question(field, utterance=""):
    if field in {"duration", "length"} and re.search(r"\b(?:until|by)\b", utterance, re.I):
        return "Do you mean a one-time request ending at that time, or a repeating hours schedule?"
    questions = {
        "target_kind": "Should I block an app, a website, or the whole computer?",
        "target": "Should I block an app, a website, or the whole computer?",
        "name": "What name should I use for this target?",
        "apps": "Which app should I block?",
        "sites": "Which website should I block?",
        "device": "Should I lock, log out, or shut down the whole computer?",
        "schedule_type": "Should the block apply all the time, for a duration, or during specific hours?",
        "duration": "How long should the block last?",
        "open_hours": "During which hours should the target remain usable?",
        "blocked_hours": "What exact start and end times should be blocked? Use one time range.",
        "days": "Which days should I block those hours?",
        "length": "How long should the focus session last?",
        "id": "Which block should I turn off?",
        "target_type": "Should I check an app, a website, or the whole computer?",
        "name": "Which app or website should I check?",
    }
    return questions.get(field, "I couldn't verify that detail. Please say it again.")


def _partial_problem(action, slots):
    if action != "add_block":
        return None
    schedule = slots.get("schedule_type")
    if schedule == "blocked_hours":
        if slots.get("blocked_hours") and not _valid_window(slots["blocked_hours"]):
            return "blocked_hours", "What exact start and end times should be blocked? Use one time range."
        if slots.get("days") and not _normalize_days(slots["days"]):
            return "days", "Which days should I block those hours?"
        if slots.get("duration") or slots.get("open_hours"):
            return "blocked_hours", "I got conflicting schedule details. Please repeat the blocked time range."
    if schedule == "open_hours":
        if slots.get("open_hours") and not _valid_window(slots["open_hours"], allow_24_end=True):
            return "open_hours", "What exact hours should remain usable? Please give a time range."
        if slots.get("duration") or slots.get("blocked_hours"):
            return "open_hours", "I got conflicting schedule details. Please repeat the usable time range."
    if schedule == "for_duration":
        if slots.get("duration") and not _valid_duration(slots["duration"]):
            return "duration", "How long should the block last? Give one duration, such as 2 hours."
        if slots.get("open_hours") or slots.get("blocked_hours"):
            return "schedule_type", "Should this be a duration, an allowed-hours window, or a blocked-hours schedule?"
    if schedule == "always" and any(slots.get(key) for key in
                                     ("duration", "open_hours", "blocked_hours", "days")):
        return "schedule_type", "Should the block apply all the time or during specific hours?"
    return None


def advance_request(request_state, model_result, utterance, turn_id=None):
    """Return a reply, one clarification, or a validated complete tool intent."""
    if not isinstance(model_result, dict):
        return {"kind": "reply", "text": "I couldn't understand that request. Please say it again."}
    if model_result.get("action") == "reply":
        return {"kind": "reply", "text": model_result.get("content") or "How can I help?"}
    if model_result.get("action") == "request_update":
        action = model_result.get("intent")
        fields = model_result.get("fields", {})
        evidence = model_result.get("evidence", {})
        require_evidence = True
    elif model_result.get("action") == "tool_call":
        action = model_result.get("tool")
        fields = deepcopy(model_result.get("arguments", {}))
        if action == "add_block" and isinstance(fields, dict) and _requested_days(utterance or "") is None:
            fields.pop("days", None)
        evidence = {}
        require_evidence = False
    else:
        return {"kind": "reply", "text": "Please tell me what you'd like to do."}
    if action == "reply":
        return {"kind": "reply", "text": model_result.get("content") or "How can I help?"}
    if action not in _SUPPORTED_ACTIONS:
        request_state.clear()
        return {"kind": "reply", "text": "That action is not supported."}
    accepted, rejected = request_state.merge(action, fields, evidence, utterance, turn_id,
                                             require_evidence=require_evidence)
    if action == "add_block" and request_state.slots.get("schedule_type") == "blocked_hours":
        days = _requested_days(utterance or "")
        if days and request_state.slots.get("days") != ",".join(days):
            request_state.set_derived("days", ",".join(days), turn_id, "spoken recurrence")
    if rejected:
        field = rejected[0]
        request_state.last_question_field = field
        return {"kind": "clarify", "field": field, "text": _rejected_question(field, utterance or "")}
    problem = _partial_problem(action, request_state.slots)
    if problem:
        request_state.last_question_field = problem[0]
        return {"kind": "clarify", "field": problem[0], "text": problem[1]}
    question_field, text = _question(action, request_state.slots)
    if question_field:
        request_state.last_question_field = question_field
        return {"kind": "clarify", "field": question_field, "text": text}
    if not accepted:
        request_state.last_question_field = "request"
        return {"kind": "clarify", "field": "request",
                "text": "I didn't catch a new detail. Please tell me what you'd like to change, or say cancel."}
    if action == "add_block" and not request_state.slots.get("name"):
        slots = deepcopy(request_state.slots)
        _default_name(action, slots)
        request_state.set_derived("name", slots["name"], turn_id, "generated from target")
    slots = deepcopy(request_state.slots)
    intent = validate_intent({"action": "tool_call", "tool": action, "arguments": slots}, utterance or "")
    if intent.get("action") != "tool_call":
        return {"kind": "clarify", "field": "validation", "text": intent.get("content", "Please clarify that request.")}
    request_state.last_question_field = None
    return {"kind": "ready", "intent": intent}
