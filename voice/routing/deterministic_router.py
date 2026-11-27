"""Deterministic intent matching and CLI-backed card/speech formatting (zero LLM calls)."""
from __future__ import annotations

import json
import os
import re
from typing import Any, Callable, Dict, Optional

from .canonical_speech import canonical_name, empty_notice, planned_reply

FALLBACK_REPLY = (
    "I can show active countdowns, list rules, check current blocks, "
    "or block or remove a named rule."
)


def llm_router_enabled() -> bool:
    return os.environ.get("LEKO_ENABLE_LLM_ROUTER") == "1"


def _fmt_remaining(seconds: int) -> str:
    secs = max(0, int(seconds))
    mins, rem = divmod(secs, 60)
    hours, mins = divmod(mins, 60)
    if hours:
        return f"{hours:02d}:{mins:02d}:{rem:02d}"
    return f"{mins:02d}:{rem:02d}"


def _spoken_duration(seconds: int) -> str:
    secs = max(0, int(seconds))
    if secs < 60:
        return f"{secs} seconds"
    mins = round(secs / 60)
    if mins < 60:
        return f"{mins} minute" if mins == 1 else f"{mins} minutes"
    hours, rem_m = divmod(mins, 60)
    if rem_m == 0:
        return f"{hours} hour" if hours == 1 else f"{hours} hours"
    return f"{hours}h {rem_m}m"


def classify_text(query: str) -> Dict[str, Any]:
    """Deterministically route natural voice/text commands without calling an LLM."""
    text = (query or "").strip()
    refusal = planned_reply(text)
    if refusal:
        return {"action": "reply", "content": refusal, "card": {"text": refusal, "columns": [], "rows": []}}

    low = text.lower()
    m_rem = re.search(
        r"\b(?:how much time|time remaining|remaining time|time left)\b.*?\b(?:on|for)\s+([a-z0-9_.\- ]+)",
        low,
    )
    if m_rem:
        hint = m_rem.group(1).strip(" ?.!\"'")
        return {"action": "tool_call", "tool": "countdowns", "deterministic": True,
                "arguments": {"rule_hint": hint}}

    if re.search(r"\b(?:what|which|show|list|any|my|earned|active)\b.*\bpass(?:es)?\b|\bactive passes\b", low):
        if not re.search(r"\b(?:request|earn|get|grant)\b", low):
            return {"action": "tool_call", "tool": "countdowns", "deterministic": True,
                    "arguments": {"filter": "pass"}}

    if re.search(r"\b(?:countdowns?|upcoming|next timer|what'?s next|when does)\b", low):
        return {"action": "tool_call", "tool": "countdowns", "deterministic": True, "arguments": {}}

    if re.search(r"\b(?:list|show|what|my|all)\b.*\brules?\b", low):
        return {"action": "tool_call", "tool": "list_rules", "deterministic": True, "arguments": {}}

    if re.search(r"\b(?:what'?s|what is|currently|active)\s+(?:blocked|enforced)\b|\bactive blocks\b|\bcheck status\b|^status\??$", low):
        return {"action": "tool_call", "tool": "status_summary", "deterministic": True, "arguments": {}}

    m_del = re.match(r"^(?:please\s+)?(?:delete|remove)\s+(?:the\s+)?rule\s+([a-z0-9_.\-]+)\s*[.!?]*$", low)
    if m_del:
        return {"action": "tool_call", "tool": "delete_rule", "arguments": {"id": m_del.group(1)}}

    m_pass = re.match(r"^(?:please\s+)?(?:request|earn)\s+(?:a\s+)?pass\s+(?:for|on)\s+(?:rule\s+)?([a-z0-9_.\-]+)\s*[.!?]*$", low)
    if m_pass:
        return {"action": "tool_call", "tool": "request_pass", "arguments": {"id": m_pass.group(1)}}

    m_block = re.match(
        r"^(?:please\s+)?block\s+([a-z0-9_.\-]+)\s+for\s+(\d+)\s*(minutes?|mins?|hours?|hrs?)\s*[.!?]*$",
        low,
    )
    if m_block:
        target = m_block.group(1).strip()
        qty = int(m_block.group(2))
        unit = "h" if m_block.group(3).startswith("h") else "m"
        args: Dict[str, Any] = {
            "name": f"Block {target}",
            "schedule_type": "for_duration",
            "duration": f"{qty}{unit}",
        }
        if "." in target and not target.startswith("com."):
            args["sites"] = [target]
        else:
            args["apps"] = [target]
        return {"action": "tool_call", "tool": "create_rule", "arguments": args}

    return {
        "action": "reply",
        "content": FALLBACK_REPLY,
        "card": {"text": FALLBACK_REPLY, "columns": [], "rows": []},
    }


def _run_json(cmd: list[str], runner: Callable[..., Any]) -> Any:
    proc = runner(cmd, capture_output=True, text=True)
    if getattr(proc, "returncode", 1) != 0:
        return None
    try:
        return json.loads((proc.stdout or "").strip() or "{}")
    except (TypeError, ValueError):
        return None


def _countdowns_reply(data: Any, args: Dict[str, Any]) -> Dict[str, Any]:
    items = list(data.get("countdowns", [])) if isinstance(data, dict) else []
    kind_filter = args.get("filter")
    hint = (args.get("rule_hint") or "").lower().strip()
    if kind_filter == "pass":
        items = [c for c in items if "pass" in str(c.get("kind", "")).lower()]
    if hint:
        items = [
            c for c in items
            if hint in str(c.get("rule", "")).lower() or hint in str(c.get("rule_id", "")).lower()
        ]
    if not items:
        msg = (f"No countdowns match {hint}." if hint else
               "No active Passes right now." if kind_filter == "pass" else "No active countdowns.")
        return {"action": "reply", "content": msg, "card": {"text": msg, "columns": [], "rows": []}}

    first = items[0]
    lead = f"{first['rule']} {first['kind']} in {_spoken_duration(first['remaining_seconds'])}."
    if hint:
        msg = lead
    elif kind_filter == "pass":
        msg = f"{len(items)} active Pass countdowns. Next: {lead}" if len(items) > 1 else f"1 active Pass: {lead}"
    else:
        msg = f"{len(items)} active countdowns. Next: {lead}" if len(items) > 1 else f"1 active countdown: {lead}"
    rows = [[str(c.get("rule", "")), str(c.get("kind", "")), _fmt_remaining(c.get("remaining_seconds", 0))]
            for c in items[:8]]
    return {"action": "reply", "content": msg, "card": {"text": msg, "columns": ["Rule", "Kind", "Remaining"], "rows": rows}}


def _list_rules_reply(data: Any) -> Dict[str, Any]:
    rules = list(data.get("rules", [])) if isinstance(data, dict) else []
    if not rules:
        msg = empty_notice()
        return {"action": "reply", "content": msg, "card": {"text": msg, "columns": [], "rows": []}}
    rows = []
    for r in rules[:8]:
        target_kind = canonical_name((r.get("target") or {}).get("kind", ""))
        sched_tb = canonical_name(((r.get("schedule") or {}).get("time_based") or {}).get("kind", ""))
        rows.append([str(r.get("name") or r.get("id", "")), target_kind, sched_tb])
    names = ", ".join(row[0] for row in rows[:4])
    msg = f"{len(rules)} Technology Management Rules: {names}."
    return {"action": "reply", "content": msg, "card": {"text": msg, "columns": ["Rule", "Target", "Schedule"], "rows": rows}}


def _status_summary_reply(settings_zip: str, leko_bin: str, runner: Callable[..., Any], at: Optional[str]) -> Dict[str, Any]:
    base = [str(leko_bin), "list-rules", "--store", str(settings_zip), "--json"]
    if at:
        base.extend(["--at", at])
    data = _run_json(base, runner)
    rules = list(data.get("rules", [])) if isinstance(data, dict) else []
    if not rules:
        msg = empty_notice()
        return {"action": "reply", "content": msg, "card": {"text": msg, "columns": [], "rows": []}}
    rows = []
    enforced_names = []
    for r in rules[:8]:
        rid = str(r.get("id", ""))
        cmd = [str(leko_bin), "check-status", "--store", str(settings_zip), "--id", rid, "--json"]
        if at:
            cmd.extend(["--at", at])
        st = _run_json(cmd, runner) or {}
        enforced = bool(st.get("enforced"))
        state_lbl = "Enforced" if enforced else "Not Enforced"
        reason_lbl = canonical_name(st.get("reason", "disabled"))
        if enforced:
            enforced_names.append(str(r.get("name") or rid))
        rows.append([str(r.get("name") or rid), state_lbl, reason_lbl])
    msg = (f"{len(enforced_names)} active blocks: {', '.join(enforced_names)}."
           if enforced_names else "No rules are currently Enforced.")
    return {"action": "reply", "content": msg, "card": {"text": msg, "columns": ["Rule", "State", "Reason"], "rows": rows}}


def maybe_handle_intent(
    intent: Dict[str, Any],
    settings_zip: str,
    leko_bin: Any,
    runner: Callable[..., Any],
    at: Optional[str] = None,
) -> Optional[Dict[str, Any]]:
    """Execute deterministic read tools and return reply + card, or None for regular pipeline."""
    if not isinstance(intent, dict):
        return None
    tool = intent.get("tool")
    if not intent.get("deterministic") and tool not in ("countdowns", "status_summary"):
        return None
    args = intent.get("arguments") or {}
    if tool == "countdowns":
        cmd = [str(leko_bin), "countdowns", "--store", str(settings_zip), "--json"]
        if at:
            cmd.extend(["--at", at])
        return _countdowns_reply(_run_json(cmd, runner), args)
    if tool == "list_rules":
        cmd = [str(leko_bin), "list-rules", "--store", str(settings_zip), "--json"]
        return _list_rules_reply(_run_json(cmd, runner))
    if tool == "status_summary":
        return _status_summary_reply(settings_zip, str(leko_bin), runner, at)
    return None
