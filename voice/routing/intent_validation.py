"""Validate voice intents and map them to canonical native CLI arguments."""

import re
from typing import Any

from . import native_cli

_DAYS = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")
_SHORT = ("mon", "tue", "wed", "thu", "fri", "sat", "sun")

INTERNAL_TO_CANONICAL = {
    "add_block": "create_rule",
    "start_session": "request_pass",
    "switch_off": "delete_rule",
    "check_status": "check_status",
}


def _reply(message: str) -> dict:
    return {"action": "reply", "content": message}


def _clock_minutes(value: str) -> int | None:
    match = re.fullmatch(r"(\d{1,2}):(\d{2})", value or "")
    if not match:
        return None
    hour, minute = int(match.group(1)), int(match.group(2))
    if hour > 24 or minute > 59 or (hour == 24 and minute):
        return None
    return hour * 60 + minute


def _valid_window(value: str, allow_24_end: bool = False) -> bool:
    parts = (value or "").split("-")
    if len(parts) != 2:
        return False
    start, end = _clock_minutes(parts[0]), _clock_minutes(parts[1])
    if start is None or end is None:
        return False
    if end == 1440:
        return allow_24_end
    return end > start


def _normalize_days(value: str) -> list[str] | None:
    if not value:
        return None
    text = value.lower()
    if "every day" in text or "daily" in text:
        return list(_SHORT)
    if "weekday" in text:
        return list(_SHORT[:5])
    if "weekend" in text:
        return list(_SHORT[5:])
    found = [short for name, short in zip(_DAYS, _SHORT) if name in text or short in text]
    return found or None


def _requested_days(text: str) -> list[str] | None:
    return _normalize_days(text or "")


def _valid_duration(value: str) -> bool:
    match = re.fullmatch(r"(\d+)\s*(minute|minutes|hour|hours|h|m)", (value or "").strip(), re.I)
    return bool(match)


def _canonical_tool(tool: Any) -> str | None:
    if not isinstance(tool, str):
        return None
    if tool in native_cli.CANONICAL_TOOLS or tool in native_cli.PLANNED_TOOLS:
        return tool
    return INTERNAL_TO_CANONICAL.get(tool)


def validate_intent(intent: dict, user_text: str) -> dict:
    """Return a validated intent whose tool is a canonical native tool name."""
    if not isinstance(intent, dict) or intent.get("action") != "tool_call":
        return intent
    tool = _canonical_tool(intent.get("tool"))
    args = intent.get("arguments")
    if tool is None or not isinstance(args, dict):
        return _reply("I did not understand that request.")
    if tool in {"show_rule", "edit_rule", "delete_rule", "queue_edit",
                "cancel_edit", "evaluate_rule"} and not args.get("id"):
        return _reply("That rule needs an id.")
    result = dict(intent)
    result["tool"] = tool
    result["arguments"] = dict(args)
    return result


def tool_to_cli_args(settings_store: Any, tool_name: str, args: dict,
                     write: bool = False) -> list[str]:
    """Build canonical native CLI arguments. The write flag is not a legacy flag."""
    values = dict(args or {})
    if write:
        values["write"] = True
    return native_cli.build_argv(settings_store, tool_name, values, at=values.get("at"))
