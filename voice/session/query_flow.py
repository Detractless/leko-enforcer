"""Conversation routing and deterministic, single-use action confirmations."""
import json
import sys
import time
from typing import Any
import sqlite3
import subprocess
import assistant as voice
from session.conversation import Conversation, confirmation
try:
    from ..intent_validation import validate_intent
except ImportError:
    from intent_validation import validate_intent

DEFAULT_CONVERSATION = Conversation()
MUTATIONS = {"add_block", "switch_off", "start_session"}


def clear_pending(conversation=DEFAULT_CONVERSATION, bridge=None):
    conversation.cancel()
    if bridge:
        bridge.emit({"type": "reset"})


def process_query(query, settings_zip, auto_confirm=False, bridge=None,
                  is_cancelled=None, speak_fn=None, conversation=None, confirmation_id=None):
    state = conversation if conversation is not None else DEFAULT_CONVERSATION
    epoch = state.epoch
    with state.turn_lock:
        if state.epoch != epoch or (is_cancelled and is_cancelled()):
            return ""
        try:
            return _process_query(query, settings_zip, auto_confirm, bridge,
                                  is_cancelled, speak_fn, state, confirmation_id)
        finally:
            if is_cancelled and is_cancelled():
                clear_pending(state, bridge)


def _memory_stage(state, query):
    """Run the deterministic memory stage.

    Returns (kind, payload): "reply" -> payload is the sentence to speak and the turn is done;
    "model" -> payload is the memories list to pass to the model.
    """
    try:
        command = state.memory.command(query)
        if command is not None:
            # Forget/correct also remove old facts from ephemeral conversation context.
            state.history.clear()
            return "reply", command
        return "model", state.memory.retrieve(query)
    except (OSError, sqlite3.Error):
        return "reply", "Local memory is unavailable. Please check its storage path."


def _llm_intent(query, state, memories):
    """Call the model, validate what it returns, and print the outcome line."""
    llm_started = time.perf_counter()
    intent = voice.parse_user_intent_safe(query, history=list(state.history), memories=memories)
    intent = validate_intent(intent, query)
    llm_elapsed = time.perf_counter() - llm_started
    tool = intent.get("tool")
    args = intent.get("arguments", {})
    print(f"[LLM Result] elapsed={llm_elapsed:.2f}s action={intent.get('action')} tool={tool}", flush=True)
    return intent




def _prepare_draft(tool, args, settings_zip, cancelled):
    """Run the CLI dry run for a draft.

    Returns (stdout, reply_or_none, was_cancelled): stdout is the CLI output to summarize
    (empty for focus sessions), reply_or_none short-circuits the turn when the preparation
    fails, and was_cancelled is True when the request was cancelled mid-preparation.
    """
    prep_started = time.perf_counter()
    try:
        cli_args = voice.tool_to_cli_args(settings_zip, tool, args, write=False)
        # Session CLI launches immediately: it has no preview mode.
        if tool == "start_session":
            print("[CLI Draft] preview unavailable for immediate focus sessions", flush=True)
            return "", None, False
        if cancelled():
            return "", None, True
        proc = subprocess.run([str(voice.LEKO_BIN)] + cli_args, capture_output=True, text=True)
        elapsed = time.perf_counter() - prep_started
        if proc.returncode != 0:
            print(f"[CLI Draft Error] elapsed={elapsed:.2f}s exit={proc.returncode} "
                  f"stderr={proc.stderr.strip()!r} stdout={proc.stdout.strip()!r}",
                  file=sys.stderr, flush=True)
            return "", "Sorry, that request could not be prepared.", False
        stdout = proc.stdout.strip()
        print(f"[CLI Draft] elapsed={elapsed:.2f}s exit=0 output={stdout!r}", flush=True)
        return stdout, None, False
    except (OSError, ValueError, KeyError, TypeError) as exc:
        elapsed = time.perf_counter() - prep_started
        print(f"[CLI Draft Error] elapsed={elapsed:.2f}s {type(exc).__name__}: {exc}; "
              f"tool={tool!r} arguments={args!r}", file=sys.stderr, flush=True)
        return "", "Sorry, that request could not be prepared.", False


def _stage_pending(tool, args, settings_zip, intent, name, kind, description, state, bridge, cancelled):
    """Stage the draft as the one pending action and emit it; returns the action id or None."""
    with state.lock:
        if cancelled():
            return None
        action_id = state.stage(intent, settings_zip)
        if bridge:
            bridge.emit_draft(name, kind, description, raw_intent=intent, action_id=action_id)
    return action_id


def _respond(query, state, bridge, speak, cancelled, text, confirmed=False):
    if cancelled():
        return ""
    state.record(query, text)
    if bridge:
        bridge.emit_speaking(text)
    speak(voice.verbalize_for_speech(text), cancelled)
    if bridge and not cancelled():
        if confirmed:
            bridge.emit_confirmed("Session started" if text.startswith("Started") else "Locked & Saved")
        else:
            bridge.emit_idle()
    return text


def _execute_pending(state, bridge, cancelled, expected_id=None):
    # Claim and launch under the same lock as cancellation: no late or duplicate writes.
    with state.lock:
        if cancelled():
            return "", False
        pending = state.take(expected_id)
        if not pending:
            if bridge and state.pending is None:
                bridge.emit({"type": "reset"})
            return "There is no matching pending action to confirm.", False
        if bridge:
            bridge.emit({"type": "reset"})
        intent = pending["intent"]
        try:
            command = voice.tool_to_cli_args(pending["settings"], intent["tool"],
                                             intent["arguments"], write=True)
            result = subprocess.run([str(voice.LEKO_BIN)] + command, capture_output=True, text=True)
        except (OSError, ValueError, KeyError):
            return "Could not complete the action. Please make a new request.", False
        if result.returncode != 0:
            return "Could not complete the action. Please make a new request.", False
        if intent["tool"] == "start_session":
            return f"Started a focus session of {intent['arguments']['length']}.", True
        return "Changes have been locked and saved.", True


def _confirmation_reply(query, state, bridge, speak, cancelled, confirmation_id):
    decision = confirmation(query)
    if decision is None:
        return None
    if decision:
        text, success = _execute_pending(state, bridge, cancelled, confirmation_id)
        return _respond(query, state, bridge, speak, cancelled, text, success)
    with state.lock:
        if confirmation_id is not None and (not state.pending or state.pending["id"] != confirmation_id):
            return _respond(query, state, bridge, speak, cancelled,
                            "There is no matching pending action to cancel.")
        state.pending = None
        if bridge:
            bridge.emit({"type": "reset"})
    return _respond(query, state, bridge, speak, cancelled, "Action dismissed.")


def _reset_pending(state, bridge):
    with state.lock:
        state.pending = None
        if bridge:
            bridge.emit({"type": "reset"})


def _route_intent(query, settings_zip, auto_confirm, bridge, state, speak,
                  cancelled, confirmation_id, intent):
    if intent.get("action") == "reply":
        return _respond(query, state, bridge, speak, cancelled,
                        intent.get("content") or "Please rephrase that request.")
    tool, args = intent.get("tool"), intent.get("arguments", {})
    if isinstance(args, dict):
        print(f"[LLM Draft] {json.dumps({'tool': tool, 'arguments': args}, ensure_ascii=False, default=str)}", flush=True)
    if tool not in MUTATIONS | {"check_status"} or not isinstance(args, dict):
        return _respond(query, state, bridge, speak, cancelled, "That action is not supported.")
    stdout, reply, was_cancelled = _prepare_draft(tool, args, settings_zip, cancelled)
    if was_cancelled:
        return ""
    if reply is not None:
        return _respond(query, state, bridge, speak, cancelled, reply)
    if cancelled():
        return ""
    summary, name, kind, description = _draft_summary(tool, args, stdout)
    if tool not in MUTATIONS:
        return _respond(query, state, bridge, speak, cancelled, summary)
    summary += " Say yes to confirm or no to cancel."
    action_id = _stage_pending(tool, args, settings_zip, intent, name, kind, description,
                               state, bridge, cancelled)
    if action_id is None:
        return ""
    if auto_confirm:
        text, success = _execute_pending(state, bridge, cancelled, action_id)
        return _respond(query, state, bridge, speak, cancelled, text, success)
    return _respond(query, state, bridge, speak, cancelled, summary)


def _process_query(query, settings_zip, auto_confirm=False, bridge=None,
                  is_cancelled=None, speak_fn=None, conversation=None, confirmation_id=None):
    state = conversation if conversation is not None else DEFAULT_CONVERSATION
    epoch = state.epoch
    cancelled = lambda: state.epoch != epoch or bool(is_cancelled and is_cancelled())
    speak = speak_fn or (lambda text, cancel=None: voice.speak(text, is_cancelled=cancel))
    with state.turn_lock:
        if cancelled():
            return ""
        replied = _confirmation_reply(query, state, bridge, speak, cancelled, confirmation_id)
        if replied is not None:
            return replied
        # A new request invalidates the old draft, but history still resolves revisions.
        _reset_pending(state, bridge)
        memory_kind, payload = _memory_stage(state, query)
        if memory_kind == "reply":
            return _respond(query, state, bridge, speak, cancelled, payload)
        memories = payload
        if bridge:
            bridge.emit_thinking(query)
        intent = _llm_intent(query, state, memories)
        if cancelled():
            return ""
        return _route_intent(query, settings_zip, auto_confirm, bridge, state, speak,
                             cancelled, confirmation_id, intent)


def _draft_summary(tool: str, args: dict[str, Any], stdout: str) -> tuple[str, str, str, str]:
    if tool == "start_session":
        length = args.get("length")
        return f"Prepared a focus session of {length}.", "Focus Session", "Session", f"Unskippable session for {length}."
    if tool == "add_block":
        device = args.get("device")
        targets = list(args.get("apps", [])) + list(args.get("sites", []))
        target_str = f"the whole computer ({device})" if device else (", ".join(targets) if targets else "selected targets")
        if args.get("schedule_type") == "blocked_hours":
            start, end = args["blocked_hours"].split("-")
            window = f"{_spoken_clock(start)} to {_spoken_clock(end)} {_spoken_days(args.get('days', ''))}"
            return (f"Drafted a block for {target_str} from {window}.", "Block Rule", "Block",
                    f"Blocks {target_str} from {window}.")
        if args.get("schedule_type") == "open_hours":
            schedule = f"usable during {args['open_hours']}"
        else:
            schedule = args.get("duration") or "all day"
        return f"Drafted a block for {target_str}, {schedule}.", "Block Rule", "Block", f"Blocks {target_str}."
    if tool == "switch_off":
        block_id = args.get("id")
        return f"Prepared to turn off block {block_id}.", f"Turn Off Block {block_id}", "Switch Off", f"Turns off rule {block_id}."
    if tool == "check_status":
        summary = voice.format_status_summary(stdout, args.get("target_type", ""), args.get("name", ""))
        return summary, "Status Report", "Status", summary
    return "Action prepared.", "Action", "Command", "Prepared action."


def _spoken_clock(value: str) -> str:
    hour, minute = map(int, value.split(":"))
    if hour == 0 and minute == 0:
        return "midnight"
    if hour == 12 and minute == 0:
        return "noon"
    suffix = "a.m." if hour < 12 else "p.m."
    shown_hour = hour % 12 or 12
    return f"{shown_hour} {suffix}" if minute == 0 else f"{shown_hour}:{minute:02d} {suffix}"


def _spoken_days(value: str) -> str:
    days = [day.strip() for day in value.split(",") if day.strip()]
    if days == ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]:
        return "every day"
    if days == ["mon", "tue", "wed", "thu", "fri"]:
        return "every weekday"
    if days == ["sat", "sun"]:
        return "every weekend"
    names = {"mon": "Monday", "tue": "Tuesday", "wed": "Wednesday", "thu": "Thursday",
             "fri": "Friday", "sat": "Saturday", "sun": "Sunday"}
    return "on " + ", ".join(names.get(day, day) for day in days)
