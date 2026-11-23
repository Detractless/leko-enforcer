"""Structured, turn-local model output for the request-state workflow."""
import json
import re

_STRING_FIELDS = ("name", "device", "target_kind", "schedule_type", "duration", "open_hours",
                  "blocked_hours", "days", "length", "id", "target_type")
_FIELD_PROPERTIES = {key: {"type": "string"} for key in _STRING_FIELDS}
_FIELD_PROPERTIES["apps"] = {"type": "array", "items": {"type": "string"}}
_FIELD_PROPERTIES["sites"] = {"type": "array", "items": {"type": "string"}}
_FIELD_PROPERTIES["schedule_type"]["enum"] = ["always", "for_duration", "open_hours", "blocked_hours"]
_FIELD_PROPERTIES["device"]["enum"] = ["lock", "logoff", "shutdown"]
_FIELD_PROPERTIES["target_kind"]["enum"] = ["app", "site", "device"]
_FIELD_PROPERTIES["target_type"]["enum"] = ["app", "site", "device"]

REQUEST_UPDATE_TOOL = {
    "type": "function",
    "function": {
        "name": "request_update",
        "description": "Propose only details stated or corrected in the user's latest turn. Never execute an action.",
        "parameters": {
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": [
                    "add_block", "start_session", "switch_off", "check_status", "reply"]},
                "fields": {"type": "object", "properties": _FIELD_PROPERTIES},
                "evidence": {"type": "object", "properties": {
                    key: {"type": "string", "description": "Exact quote from the latest user turn."}
                    for key in _STRING_FIELDS + ("apps", "sites")}},
                "content": {"type": "string", "description": "Short reply only when action is reply."},
            },
            "required": ["action", "fields", "evidence"],
        },
    },
}

REQUEST_UPDATE_GUIDANCE = """
For this request-collection call, use the request_update function for focus, block,
switch-off, or status requests. It only proposes fields; the application owns state,
clarification, previews, confirmation, and execution. Put only details stated or
corrected in the latest user turn in fields. For a changed list of apps or sites,
return the full corrected list, but cite a quote from this turn in evidence. Every
evidence value must be an exact short quote from the latest user turn. Do not invent
values, labels, schedules, recurrence, or confirmations. Leave fields empty and use
action reply for ordinary conversation. Never return more than one action.
""".strip()


def normalize_duration(value):
    text = value.strip().lower()
    match = re.fullmatch(r"(\d+)\s*(?:hours?|hrs?|h)", text)
    if match:
        return f"{match.group(1)}h"
    match = re.fullmatch(r"(\d+)\s*(?:minutes?|mins?|m)", text)
    return f"{match.group(1)}m" if match else text


def completion_result(data):
    """Convert one OpenAI-style completion, retaining local reasoning when present."""
    message = data["choices"][0]["message"]
    calls = message.get("tool_calls", [])
    if not calls:
        result = {"action": "reply", "content": message.get("content", "")}
    else:
        if len(calls) != 1:
            raise ValueError("The model returned more than one action.")
        call = calls[0]
        name = call["function"]["name"]
        args = json.loads(call["function"]["arguments"])
        if name == "request_update":
            fields = args.get("fields", {})
            if not isinstance(fields, dict):
                raise ValueError("Request fields must be an object.")
            for key in ("duration", "length"):
                if isinstance(fields.get(key), str):
                    fields[key] = normalize_duration(fields[key])
            result = {"action": "request_update", "intent": args.get("action"),
                      "fields": fields, "evidence": args.get("evidence", {}),
                      "content": args.get("content", "")}
        else:
            for key in ("duration", "length"):
                if isinstance(args.get(key), str):
                    args[key] = normalize_duration(args[key])
            result = {"action": "tool_call", "tool": name, "arguments": args}
    if "reasoning_content" in message:
        result["reasoning_content"] = message["reasoning_content"]
    return result
