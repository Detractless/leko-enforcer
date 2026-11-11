"""Ephemeral, typed request slots with per-field provenance."""
from copy import deepcopy
import re


ACTION_FIELDS = {
    "add_block": {"name", "apps", "sites", "device", "target_kind", "schedule_type",
                  "duration", "open_hours", "blocked_hours", "days"},
    "start_session": {"length"},
    "switch_off": {"id"},
    "check_status": {"target_type", "name"},
}
_DEVICE_ACTIONS = {"lock", "logoff", "shutdown"}
_SCHEDULE_TYPES = {"always", "for_duration", "open_hours", "blocked_hours"}
_TARGET_TYPES = {"app", "site", "device"}


def _valid_value(key, value):
    if key in {"apps", "sites"}:
        return (isinstance(value, list) and bool(value)
                and all(isinstance(item, str) and item.strip() for item in value))
    if key == "device":
        return isinstance(value, str) and value in _DEVICE_ACTIONS
    if key in {"target_kind", "target_type"}:
        return isinstance(value, str) and value in _TARGET_TYPES
    if key == "schedule_type":
        return isinstance(value, str) and value in _SCHEDULE_TYPES
    if key in {"duration", "length"}:
        return isinstance(value, str) and bool(re.fullmatch(r"[1-9]\d*[mh]", value))
    return isinstance(value, str) and bool(value.strip()) and len(value) <= 300


def _fold(text):
    return " ".join("".join(c.lower() if c.isalnum() else " " for c in text).split())


def _has_quote(utterance, quote):
    words, cited = utterance.split(), quote.split()
    return bool(cited) and any(words[i:i + len(cited)] == cited
                                for i in range(len(words) - len(cited) + 1))


class RequestState:
    """Tracks one active request; it is not persisted across process restarts."""

    def __init__(self):
        self.action = None
        self.slots = {}
        self.provenance = {}
        self.last_question_field = None

    @property
    def active(self):
        return self.action is not None

    def clear(self):
        self.action = None
        self.slots.clear()
        self.provenance.clear()
        self.last_question_field = None

    def context(self):
        if not self.active:
            return None
        return {"action": self.action, "fields": deepcopy(self.slots),
                "provenance": deepcopy(self.provenance)}

    def set_derived(self, key, value, turn_id=None, reason="deterministic rule"):
        if self.action is None or key not in ACTION_FIELDS[self.action] or not _valid_value(key, value):
            return False
        if self.slots.get(key) != value:
            self._invalidate({key: value}, turn_id)
            self.slots[key] = deepcopy(value)
        self.provenance[key] = {"turn_id": turn_id, "kind": "derived", "evidence": reason}
        self._derive_target_kind(turn_id)
        return True

    def merge(self, action, fields, evidence, utterance, turn_id=None, require_evidence=True):
        """Merge one validated patch; reject values without current-turn evidence."""
        if action not in ACTION_FIELDS or not isinstance(fields, dict):
            return [], list(fields) if isinstance(fields, dict) else ["fields"]
        if self.action != action:
            self.clear()
            self.action = action
        evidence = evidence if isinstance(evidence, dict) else {}
        accepted, rejected, patch = [], [], {}
        utterance_folded = _fold(utterance or "")
        for key, value in fields.items():
            if (action == "add_block" and key == "name"
                    and not re.search(r"\b(?:call|name)\s+(?:it|this|the block)\b", utterance or "", re.I)):
                continue
            quote = evidence.get(key)
            quote_folded = _fold(quote) if isinstance(quote, str) else ""
            supported = not require_evidence or _has_quote(utterance_folded, quote_folded)
            valid = key in ACTION_FIELDS[action] and _valid_value(key, value)
            if valid and not supported and self.slots.get(key) == value:
                continue
            if not valid or not supported:
                rejected.append(key)
            else:
                patch[key] = deepcopy(value)
        if not patch:
            return accepted, rejected
        self._invalidate(patch, turn_id)
        for key, value in patch.items():
            self.slots[key] = value
            quote = evidence.get(key)
            self.provenance[key] = {
                "turn_id": turn_id, "kind": "user_turn",
                "evidence": quote if isinstance(quote, str) else (utterance or "")[:160],
            }
            accepted.append(key)
        self._derive_target_kind(turn_id)
        return accepted, rejected

    def _invalidate(self, patch, turn_id):
        if (self.action == "check_status" and "target_type" in patch
                and patch["target_type"] != self.slots.get("target_type")):
            self._drop("name")
        schedule = patch.get("schedule_type")
        if schedule is not None and schedule != self.slots.get("schedule_type"):
            for key in ("duration", "open_hours", "blocked_hours", "days"):
                if key != "days" or schedule != "blocked_hours":
                    self._drop(key)

        kind = patch.get("target_kind")
        target_fields = {key for key in ("device", "apps", "sites") if key in patch}
        if "device" in target_fields and not ("apps" in target_fields or "sites" in target_fields):
            self._drop("apps")
            self._drop("sites")
        elif target_fields & {"apps", "sites"} and "device" not in target_fields:
            self._drop("device")
        if kind == "device":
            self._drop("apps")
            self._drop("sites")
        elif kind == "app":
            self._drop("device")
            self._drop("sites")
        elif kind == "site":
            self._drop("device")
            self._drop("apps")
        if target_fields or kind:
            if self.provenance.get("name", {}).get("kind") == "derived":
                self._drop("name")

    def _derive_target_kind(self, turn_id):
        if self.action != "add_block":
            return
        categories = sum(bool(self.slots.get(key)) for key in ("device", "apps", "sites"))
        if categories > 1:
            self._drop("target_kind")
            return
        for field, value in (("device", "device"), ("apps", "app"), ("sites", "site")):
            if self.slots.get(field):
                self.slots["target_kind"] = value
                self.provenance["target_kind"] = {
                    "turn_id": turn_id, "kind": "derived", "evidence": f"{field} supplied"}
                return

    def _drop(self, key):
        self.slots.pop(key, None)
        self.provenance.pop(key, None)
