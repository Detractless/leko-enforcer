"""Exact Title Case canonical speech and planned-feature refusals."""

import json
import re

PLANNED_REPLY = "That feature is not available yet."
PLANNED_NAMES = (
    "Pomodoro Schedule",
    "Community-Linked Apps",
    "Community-Linked Websites",
    "Restart Seal",
)

TITLE = {
    "technology_management_rule": "Technology Management Rule",
    "specific_apps": "Specific Apps",
    "allow_only_apps": "Allow Only Apps",
    "specific_websites": "Specific Websites",
    "topic_detected_websites": "Topic Detected Websites",
    "allow_only_websites": "Allow Only Websites",
    "whole_device": "Whole Device",
    "always_on_schedule": "Always On Schedule",
    "one_time_schedule": "One Time Schedule",
    "routine_schedule": "Routine Schedule",
    "time_limit": "Time Limit",
    "session_limit": "Session Limit",
    "force_close_penalty": "Force Close Penalty",
    "no_pass": "No Pass",
    "earn_pass": "Earn Pass",
    "engagement_pass": "Engagement Pass",
    "timed_pass": "Timed Pass",
    "rest_of_day_pass": "Rest Of Day Pass",
    "no_seal": "No Seal",
    "countdown_seal": "Countdown Seal",
    "queued_delay_seal": "Queued Delay Seal",
    "typing_seal": "Typing Seal",
    "task_seal": "Task Seal",
    "edit_window_seal": "Edit Window Seal",
    "active_schedule_seal": "Active Schedule Seal",
    "partner_password_seal": "Partner Password Seal",
    "custom_verifier": "Custom Verifier",
    "enforced": "Enforced",
    "pass_active": "Pass Active",
    "outside_schedule": "Outside Schedule",
    "usage_within_budget": "Usage Within Budget",
    "disabled": "Disabled",
    "no_penalty": "No Penalty",
}


def canonical_name(value):
    """Return the exact Title Case name for one canonical key."""
    if not isinstance(value, str):
        return ""
    raw = value.strip().replace("-", "_").replace(" ", "_")
    key = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", "_", raw).lower()
    if key in TITLE:
        return TITLE[key]
    return " ".join(part.capitalize() for part in key.split("_") if part)


def planned_reply(text):
    """Return the fixed refusal for a planned feature, or None."""
    if not isinstance(text, str):
        return None
    lowered = text.lower()
    for name in PLANNED_NAMES:
        if name.lower() in lowered:
            return PLANNED_REPLY
    return None


def status_sentence(data):
    """Speak one deterministic status or enforcement result."""
    if not isinstance(data, dict) or not data:
        return "No status is available."
    rule = data.get("rule", "")
    enforced = data.get("enforced")
    reason = canonical_name(data.get("reason", ""))
    passed = data.get("pass", {})
    pass_active = passed.get("active") if isinstance(passed, dict) else False
    prefix = rule + ": " if rule else ""
    if enforced is True:
        sentence = "{}{} Enforced. Reason: {}.".format(prefix, "", reason)
    elif enforced is False:
        sentence = "{}{} Not Enforced. Reason: {}.".format(prefix, "", reason)
    else:
        return "No status is available."
    if pass_active:
        sentence += " An Earn Pass is active; the rule was not edited."
    return sentence


def format_status_summary(raw_stdout, target_type="", name=""):
    """Turn native CLI JSON into one spoken status sentence."""
    try:
        data = json.loads(raw_stdout)
    except (TypeError, ValueError):
        return "That status could not be read."
    return status_sentence(data)


def seal_refusal(seal):
    return "That change was blocked by the {}.".format(canonical_name(seal))


def queued_notice(seconds):
    return "That edit is queued under a Queued Delay Seal for {} seconds.".format(seconds)


def cancelled_notice():
    return "The queued edit was cancelled."


def pass_notice():
    return "A pass was earned. The rule was not edited."


def empty_notice():
    return "No rules are stored."


def loading_notice():
    return "The voice model is still loading."


def error_notice(detail):
    return "That request could not be completed: {}".format(detail)
