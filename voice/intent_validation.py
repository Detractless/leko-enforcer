"""Validate model intents and map safe requests to the local CLI."""
from __future__ import annotations

import re
from typing import Any, Dict, List

WEEKDAYS = ("mon", "tue", "wed", "thu", "fri", "sat", "sun")
_CLOCK = re.compile(r"^(\d{2}):(\d{2})$")
_BLOCKED_WORDS = re.compile(
    r"\b(?:block(?:ed|ing)?|lock(?:ed|ing)?|clos(?:e|ed|ing)|"
    r"can't use|cannot use|not (?:(?:to )?be )?(?:able to use|usable)|not allowed to use|"
    r"don't want to use|don't want (?:to )?be able to use|unusable)\b", re.I)
_ALLOWED_WORDS = re.compile(
    r"\b(?:allow(?:ed)?|open|usable|can use|able to use|available)\b", re.I)
_NEGATED_ALLOWED = re.compile(
    r"\b(?:not|never|don't|do not|can't|cannot)\s+(?:(?:to )?(?:be|remain|stay)\s+)?"
    r"(?:usable|allowed|open|available|able to use|use)\b|"
    r"\bdon't want (?:it )?(?:to )?(?:be )?(?:usable|able to use|use)\b", re.I)
_DAILY_WORDS = re.compile(r"\b(?:daily|every day|everyday|each day|nightly|every night|each night|all week)\b", re.I)
_WEEKDAY_WORDS = re.compile(r"\b(?:weekdays?|workdays?)\b", re.I)
_WEEKEND_WORDS = re.compile(r"\bweekends?\b", re.I)
_CLOCK_MARKER = re.compile(
    r"\b(?:midnight|noon|\d{1,2}(?::\d{2})?\s*(?:a\.?m\.?|p\.?m\.?))(?![A-Za-z])", re.I)
_CLOCK_RANGE_WORDS = re.compile(r"\b(?:from|between)\b.*\b(?:to|until|and)\b", re.I)
_UNTIL_CLOCK = re.compile(r"\b(?:until|by)\s+(?:midnight|noon|\d{1,2}(?::\d{2})?\s*(?:a\.?m\.?|p\.?m\.)?)\b", re.I)
_DAY_ALIASES = {
    "mon": r"\bmon(?:day)?s?\b", "tue": r"\btue(?:s(?:day)?)?s?\b",
    "wed": r"\bwed(?:nesday)?s?\b", "thu": r"\bthu(?:rs(?:day)?)?s?\b",
    "fri": r"\bfri(?:day)?s?\b", "sat": r"\bsat(?:urday)?s?\b",
    "sun": r"\bsun(?:day)?s?\b",
}


def _reply(message: str) -> dict[str, str]:
    return {"action": "reply", "content": message}


def _clock_minutes(value: str, allow_24: bool = False) -> int | None:
    match = _CLOCK.fullmatch(value)
    if not match:
        return None
    hour, minute = map(int, match.groups())
    if minute > 59 or hour > (24 if allow_24 else 23):
        return None
    if hour == 24 and (not allow_24 or minute != 0):
        return None
    return hour * 60 + minute


def _valid_window(value: Any, allow_24_end: bool = False) -> bool:
    if not isinstance(value, str):
        return False
    for window in value.split(","):
        parts = window.split("-")
        if len(parts) != 2:
            return False
        start = _clock_minutes(parts[0])
        end = _clock_minutes(parts[1], allow_24=allow_24_end)
        if start is None or end is None or start == end:
            return False
    return True


def blocked_hours_to_open_hours(value: str) -> str:
    """Invert a closed interval into the CLI's allowed interval."""
    if not _valid_window(value) or "," in value:
        raise ValueError("Blocked hours must be one HH:MM-HH:MM window.")
    start, end = value.split("-")
    return f"{end}-{start}"


def _blocked_hours_for_days(value: str, days: list[str]) -> str:
    if not _valid_window(value) or "," in value:
        raise ValueError("Blocked hours must be one HH:MM-HH:MM window.")
    start, end = value.split("-")
    if days == list(WEEKDAYS):
        return blocked_hours_to_open_hours(value)
    start_minute, end_minute = _clock_minutes(start), _clock_minutes(end)
    if start_minute > end_minute:
        if end_minute != 0:
            raise ValueError("An overnight blocked window can only be mapped safely for every day.")
        return f"{end}-{start}"
    allowed = []
    if start_minute:
        allowed.append(f"00:00-{start}")
    if end_minute < 1440:
        allowed.append(f"{end}-24:00")
    if not allowed:
        raise ValueError("The blocked interval leaves no usable hours.")
    return ",".join(allowed)


def _normalize_days(value: Any) -> list[str] | None:
    if isinstance(value, list):
        words = value
    elif isinstance(value, str):
        lowered = value.strip().lower()
        if lowered in {"all", "daily", "every day", "everyday", "all days"}:
            return list(WEEKDAYS)
        words = lowered.split(",")
    else:
        return None
    days = [str(word).strip().lower() for word in words]
    if not days or any(day not in WEEKDAYS for day in days) or len(set(days)) != len(days):
        return None
    return [day for day in WEEKDAYS if day in days]


def _requested_days(user_text: str) -> list[str] | None:
    if _DAILY_WORDS.search(user_text):
        return list(WEEKDAYS)
    if _WEEKDAY_WORDS.search(user_text):
        return list(WEEKDAYS[:5])
    if _WEEKEND_WORDS.search(user_text):
        return list(WEEKDAYS[5:])
    found = [day for day, pattern in _DAY_ALIASES.items() if re.search(pattern, user_text, re.I)]
    return [day for day in WEEKDAYS if day in found] if found else None


def _has_clock_range(user_text: str) -> bool:
    if len(_CLOCK_MARKER.findall(user_text)) >= 2:
        return True
    return bool(_CLOCK_RANGE_WORDS.search(user_text) and re.search(r"\b\d{1,2}\b", user_text))


def _clarify_blocked_days() -> str:
    return "Which days should I block those hours? I can set this for every day, weekdays, weekends, or named days."


def _block_target_problem(args: dict[str, Any]) -> dict[str, str] | None:
    """The cheap block checks: name, targets, device. None means the block passes."""
    name = args.get("name")
    if not isinstance(name, str) or not name.strip():
        return _reply("What should I call this block?")
    apps, sites, device = args.get("apps", []), args.get("sites", []), args.get("device")
    if not isinstance(apps, list) or not isinstance(sites, list):
        return _reply("Which app, website, or device should I block?")
    if any(not isinstance(value, str) or not value.strip() for value in apps + sites):
        return _reply("I couldn't identify the app or website. Please name it again.")
    if device and (not isinstance(device, str) or device not in {"lock", "logoff", "shutdown"} or apps or sites):
        return _reply("A device block must use lock, logoff, or shutdown by itself. Which should I use?")
    if not device and not apps and not sites:
        return _reply("Which app, website, or device should I block?")
    return None


def _blocked_hours_reply(args: dict[str, Any], user_text: str) -> dict[str, str] | None:
    """The blocked_hours schedule checks; sets args["days"] when the days match. None means ok."""
    blocked = args.get("blocked_hours")
    if not _valid_window(blocked) or "," in blocked:
        return _reply("What exact start and end times should be blocked? Use one time range.")
    days = _normalize_days(args.get("days"))
    requested_days = _requested_days(user_text)
    if requested_days is None and days is None:
        return _reply(_clarify_blocked_days())
    if days is None:
        days = requested_days
    if requested_days is not None and days != requested_days:
        return _reply("The days in that schedule do not match what you said. Which days should I use?")
    start, end = blocked.split("-")
    if _clock_minutes(start) > _clock_minutes(end) and _clock_minutes(end) != 0 and days != list(WEEKDAYS):
        return _reply("That blocked window crosses midnight. I can safely schedule it every day; should it repeat daily?")
    if args.get("duration") or args.get("open_hours"):
        return _reply("I got conflicting schedule details. Please repeat the blocked time range.")
    args["days"] = ",".join(days)
    return None


def _open_hours_reply(args: dict[str, Any], user_text: str) -> dict[str, str] | None:
    """The open_hours schedule checks. None means ok."""
    if not _valid_window(args.get("open_hours"), allow_24_end=True):
        return _reply("What exact hours should remain usable? Please give a time range.")
    if (_BLOCKED_WORDS.search(user_text)
            and (not _ALLOWED_WORDS.search(user_text) or _NEGATED_ALLOWED.search(user_text))):
        return _reply("That request sounds like a blocked time, but I got usable hours. Should the computer be blocked during those hours?")
    if args.get("duration") or args.get("blocked_hours"):
        return _reply("I got conflicting schedule details. Please repeat the time range.")
    return None


def _duration_reply(args: dict[str, Any], user_text: str) -> dict[str, str] | None:
    """The for_duration schedule checks. None means ok."""
    if _has_clock_range(user_text):
        return _reply("You gave clock times, so I need to know whether you want a repeating blocked-hours schedule or a one-time block.")
    if _UNTIL_CLOCK.search(user_text):
        return _reply("Do you mean a one-time request ending at that time, or a repeating hours schedule?")
    if not _valid_duration(args.get("duration")) or args.get("days"):
        return _reply("How long should the block last? Give one duration, such as 2 hours.")
    if args.get("open_hours") or args.get("blocked_hours"):
        return _reply("I got conflicting schedule details. Please repeat the duration.")
    return None


def _valid_duration(value: Any) -> bool:
    return isinstance(value, str) and bool(re.fullmatch(r"[1-9]\d*[mh]", value))


def _schedule_reply(args: dict[str, Any], user_text: str) -> dict[str, str] | None:
    """The schedule checks, dispatched by schedule_type. None means the schedule passes."""
    schedule = args.get("schedule_type")
    if not isinstance(schedule, str) or schedule not in {
            "always", "for_duration", "open_hours", "blocked_hours"}:
        return _reply("When should this block apply: always, for a duration, or during specific hours?")
    if schedule == "blocked_hours":
        return _blocked_hours_reply(args, user_text)
    if schedule == "open_hours":
        return _open_hours_reply(args, user_text)
    if schedule == "for_duration":
        return _duration_reply(args, user_text)
    if schedule == "always" and _has_clock_range(user_text):
        return _reply("You gave clock times, so I need the exact hours and days before I can prepare the block.")
    if any(args.get(key) for key in ("duration", "open_hours", "blocked_hours")):
        return _reply("I got conflicting schedule details. Please repeat when the block should apply.")
    return None


def validate_intent(intent: dict[str, Any], user_text: str) -> dict[str, Any]:
    """Reject unsafe or incomplete actions before preview or confirmation."""
    if not isinstance(intent, dict):
        return _reply("I couldn't prepare that request. Please say what you want to do.")
    if intent.get("action") != "tool_call":
        return intent
    args = intent.get("arguments")
    if not isinstance(args, dict):
        return _reply("I couldn't prepare that request. Please repeat the details.")
    tool = intent.get("tool")
    if tool == "start_session":
        return intent if _valid_duration(args.get("length")) else _reply(
            "How long should the focus session last? Give one duration, such as 25 minutes.")
    if tool == "switch_off":
        return intent if isinstance(args.get("id"), str) and args["id"].strip() else _reply(
            "Which block should I turn off?")
    if tool == "check_status":
        target = args.get("target_type")
        if target not in {"app", "site", "device"}:
            return _reply("Should I check an app, a website, or the whole computer?")
        if target == "device":
            return intent if not args.get("name") else _reply(
                "For a whole-computer check, I don't need an app or website name.")
        return intent if isinstance(args.get("name"), str) and args["name"].strip() else _reply(
            "Which app or website should I check?")
    if tool != "add_block":
        return _reply("That action is not supported.")
    args = dict(args)
    problem = _block_target_problem(args) or _schedule_reply(args, user_text)
    if problem is not None:
        return problem
    intent = dict(intent)
    intent["arguments"] = args
    return intent


def tool_to_cli_args(settings_zip: str, tool_name: str, args: Dict[str, Any], write: bool = False) -> List[str]:
    """Map structured tool calls into `leko` CLI arguments."""
    if not isinstance(args, dict):
        raise ValueError("Action arguments must be an object.")
    cli_args: List[str] = []
    if tool_name == "start_session":
        if not _valid_duration(args.get("length")):
            raise ValueError("A focus session needs one positive duration.")
        return ["session", args["length"]]
    if tool_name == "add_block":
        args = dict(args)
        problem = _block_target_problem(args) or _schedule_reply(args, "")
        if problem:
            raise ValueError(problem["content"])
        cli_args.extend(["add-block", settings_zip, "--name", args["name"]])
        if args.get("device"):
            cli_args.extend(["--device", args["device"]])
        for app in args.get("apps", []):
            cli_args.extend(["--app", app])
        for site in args.get("sites", []):
            cli_args.extend(["--site", site])
        schedule = args.get("schedule_type")
        if schedule == "always":
            cli_args.append("--always")
        elif schedule == "for_duration" and args.get("duration"):
            if not _valid_duration(args["duration"]):
                raise ValueError("A duration block needs one positive duration.")
            cli_args.extend(["--for", args["duration"]])
        elif schedule == "open_hours" and _valid_window(args.get("open_hours"), allow_24_end=True):
            cli_args.extend(["--open-hours", args["open_hours"]])
        elif schedule == "blocked_hours" and args.get("blocked_hours"):
            days = _normalize_days(args.get("days"))
            if not days:
                raise ValueError("Blocked-hours schedules need explicit days.")
            cli_args.extend(["--open-hours", _blocked_hours_for_days(args["blocked_hours"], days)])
        else:
            raise ValueError("Incomplete or unsupported block schedule.")
        if args.get("days"):
            cli_args.extend(["--days", args["days"]])
        if write:
            cli_args.append("--write")
        return cli_args
    if tool_name == "switch_off":
        if not isinstance(args.get("id"), str) or not args["id"].strip():
            raise ValueError("Turning off a block needs its name or identifier.")
        cli_args.extend(["switch-off", settings_zip, "--id", args["id"]])
        if write:
            cli_args.append("--write")
        return cli_args
    if tool_name == "check_status":
        target = args.get("target_type")
        if target not in {"app", "site", "device"}:
            raise ValueError("A status check needs app, site, or device as its target.")
        cli_args.extend(["status", settings_zip])
        if target == "device":
            if args.get("name"):
                raise ValueError("A whole-computer status check does not take a name.")
            cli_args.append("--device")
        elif target == "app":
            if not isinstance(args.get("name"), str) or not args["name"].strip():
                raise ValueError("An app status check needs a name.")
            cli_args.extend(["--app", args["name"]])
        else:
            if not isinstance(args.get("name"), str) or not args["name"].strip():
                raise ValueError("A website status check needs a domain.")
            cli_args.extend(["--site", args["name"]])
        return cli_args
    raise ValueError(f"Unknown tool: {tool_name}")
