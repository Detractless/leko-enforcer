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
    from ..request_state.flow import advance_request
    from ..request_state.draft_summary import draft_summary
except ImportError:
    from request_state.flow import advance_request
    from request_state.draft_summary import draft_summary

DEFAULT_CONVERSATION = Conversation()
MUTATIONS = {"add_block", "switch_off", "start_session"}


def clear_pending(conversation=DEFAULT_CONVERSATION, bridge=None):
    conversation.cancel()
    if bridge:
        bridge.emit({"type": "reset"})


def _log(state, event, **details):
    if state.session_log is not None:
        return state.session_log.append(event, turn_id=state.current_turn_id, **details)
    return False


def process_query(query, settings_zip, auto_confirm=False, bridge=None,
                  is_cancelled=None, speak_fn=None, conversation=None, confirmation_id=None,
                  preserve_pending_on_cancel=False, session_log=None, turn_id=None,
                  input_source="text"):
    state = conversation if conversation is not None else DEFAULT_CONVERSATION
    if session_log is not None:
        state.session_log = session_log
    epoch = state.epoch
    with state.turn_lock:
        state.current_turn_id = (turn_id or state.session_log.new_turn_id()) if state.session_log else turn_id
        state.current_input_source = input_source
        if state.epoch != epoch or (is_cancelled and is_cancelled()):
            _log(state, "turn_cancelled", reason="cancelled_before_processing")
            return ""
        _log(state, "user_query_submitted", text=query, source=input_source)
        try:
            return _process_query(query, settings_zip, auto_confirm, bridge,
                                  is_cancelled, speak_fn, state, confirmation_id)
        finally:
            if is_cancelled and is_cancelled():
                _log(state, "turn_cancelled", reason="superseded_or_interrupted")
                if not preserve_pending_on_cancel:
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
    """Request a turn-local patch, with fallback for older test/runtime adapters."""
    llm_started = time.perf_counter()
    trace_callback = None
    if state.session_log is not None:
        trace_callback = lambda trace: _log(state, "model_call", trace=trace)
    parser = getattr(voice, "parse_request_update_safe", None)
    if parser:
        intent = parser(query, history=list(state.history), memories=memories,
                        active_request=state.active_request.context(), trace_callback=trace_callback)
    else:
        intent = voice.parse_user_intent_safe(query, history=list(state.history), memories=memories,
                                              trace_callback=trace_callback)
    llm_elapsed = time.perf_counter() - llm_started
    tool = intent.get("tool") or intent.get("intent")
    args = intent.get("arguments", intent.get("fields", {}))
    print(f"[LLM Result] elapsed={llm_elapsed:.2f}s action={intent.get('action')} tool={tool}", flush=True)
    _log(state, "model_decision", elapsed_seconds=round(llm_elapsed, 3), action=intent.get("action"),
         tool=tool, arguments=args, content=intent.get("content"),
         reasoning_content=intent.get("reasoning_content"))
    return intent




def _prepare_draft(tool, args, settings_zip, cancelled, state=None):
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
            if state:
                _log(state, "tool_preview", tool=tool, arguments=args, status="unavailable")
            return "", None, False
        if cancelled():
            if state:
                _log(state, "tool_preview", tool=tool, arguments=args, status="cancelled")
            return "", None, True
        command = [str(voice.LEKO_BIN)] + cli_args
        proc = subprocess.run(command, capture_output=True, text=True)
        elapsed = time.perf_counter() - prep_started
        if proc.returncode != 0:
            print(f"[CLI Draft Error] elapsed={elapsed:.2f}s exit={proc.returncode} "
                  f"stderr={proc.stderr.strip()!r} stdout={proc.stdout.strip()!r}",
                  file=sys.stderr, flush=True)
            if state:
                _log(state, "tool_preview", tool=tool, arguments=args, status="error",
                     command=command, returncode=proc.returncode, stdout=proc.stdout,
                     stderr=proc.stderr, elapsed_seconds=round(elapsed, 3))
            return "", "Sorry, that request could not be prepared.", False
        stdout = proc.stdout.strip()
        print(f"[CLI Draft] elapsed={elapsed:.2f}s exit=0 output={stdout!r}", flush=True)
        if state:
            _log(state, "tool_preview", tool=tool, arguments=args, status="ok", command=command,
                 returncode=proc.returncode, stdout=proc.stdout, stderr=proc.stderr,
                 elapsed_seconds=round(elapsed, 3))
        return stdout, None, False
    except (OSError, ValueError, KeyError, TypeError) as exc:
        elapsed = time.perf_counter() - prep_started
        print(f"[CLI Draft Error] elapsed={elapsed:.2f}s {type(exc).__name__}: {exc}; "
              f"tool={tool!r} arguments={args!r}", file=sys.stderr, flush=True)
        if state:
            _log(state, "tool_preview", tool=tool, arguments=args, status="error",
                 error={"type": type(exc).__name__, "message": str(exc)},
                 elapsed_seconds=round(elapsed, 3))
        return "", "Sorry, that request could not be prepared.", False


def _stage_pending(tool, args, settings_zip, intent, name, kind, description, state, bridge, cancelled):
    """Stage the draft as the one pending action and emit it; returns the action id or None."""
    with state.lock:
        if cancelled():
            return None
        action_id = state.stage(intent, settings_zip, origin_turn_id=state.current_turn_id)
        _log(state, "tool_draft_staged", action_id=action_id, tool=tool, arguments=args,
             name=name, kind=kind, description=description)
        if bridge:
            bridge.emit_draft(name, kind, description, raw_intent=intent, action_id=action_id)
    return action_id


def _respond(query, state, bridge, speak, cancelled, text, confirmed=False):
    if cancelled():
        return ""
    state.record(query, text)
    _log(state, "assistant_reply", text=text, confirmed=confirmed)
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
            _log(state, "tool_execution", status="no_matching_draft", expected_id=expected_id)
            if bridge and state.pending is None:
                bridge.emit({"type": "reset"})
            return "There is no matching pending action to confirm.", False
        if bridge:
            bridge.emit({"type": "reset"})
        intent = pending["intent"]
        origin_turn_id = pending.get("origin_turn_id")
        try:
            command = voice.tool_to_cli_args(pending["settings"], intent["tool"],
                                             intent["arguments"], write=True)
            result = subprocess.run([str(voice.LEKO_BIN)] + command, capture_output=True, text=True)
        except (OSError, ValueError, KeyError) as exc:
            _log(state, "tool_execution", status="error", origin_turn_id=origin_turn_id,
                 tool=intent.get("tool"), arguments=intent.get("arguments"),
                 error={"type": type(exc).__name__, "message": str(exc)})
            return "Could not complete the action. Please make a new request.", False
        _log(state, "tool_execution", status="ok" if result.returncode == 0 else "error",
             origin_turn_id=origin_turn_id, tool=intent.get("tool"),
             arguments=intent.get("arguments"), command=[str(voice.LEKO_BIN)] + command,
             returncode=result.returncode, stdout=result.stdout, stderr=result.stderr)
        if result.returncode != 0:
            return "Could not complete the action. Please make a new request.", False
        state.active_request.clear()
        if intent["tool"] == "start_session":
            return f"Started a focus session of {intent['arguments']['length']}.", True
        return "Changes have been locked and saved.", True


def _confirmation_reply(query, state, bridge, speak, cancelled, confirmation_id):
    decision = confirmation(query)
    if decision is None:
        return None
    if decision:
        pending = state.pending or {}
        _log(state, "tool_confirmation", accepted=True, action_id=pending.get("id"),
             origin_turn_id=pending.get("origin_turn_id"))
        text, success = _execute_pending(state, bridge, cancelled, confirmation_id)
        return _respond(query, state, bridge, speak, cancelled, text, success)
    with state.lock:
        if confirmation_id is not None and (not state.pending or state.pending["id"] != confirmation_id):
            return _respond(query, state, bridge, speak, cancelled,
                            "There is no matching pending action to cancel.")
        pending = state.pending or {}
        _log(state, "tool_confirmation", accepted=False, action_id=pending.get("id"),
             origin_turn_id=pending.get("origin_turn_id"))
        state.pending = None
        state.active_request.clear()
        if bridge:
            bridge.emit({"type": "reset"})
    return _respond(query, state, bridge, speak, cancelled, "Action dismissed.")


def _reset_pending(state, bridge):
    with state.lock:
        if state.pending:
            _log(state, "tool_confirmation", accepted=False, action_id=state.pending.get("id"),
                 origin_turn_id=state.pending.get("origin_turn_id"), reason="superseded_by_new_request")
        state.pending = None
        if bridge:
            bridge.emit({"type": "reset"})


def _route_intent(query, settings_zip, auto_confirm, bridge, state, speak,
                  cancelled, confirmation_id, intent):
    if intent.get("action") == "reply":
        return _respond(query, state, bridge, speak, cancelled,
                        intent.get("content") or "Please rephrase that request.")
    tool, args = intent.get("tool"), intent.get("arguments", {})
    _log(state, "tool_call_selected", tool=tool, arguments=args)
    if isinstance(args, dict):
        print(f"[LLM Draft] {json.dumps({'tool': tool, 'arguments': args}, ensure_ascii=False, default=str)}", flush=True)
    if tool not in MUTATIONS | {"check_status"} or not isinstance(args, dict):
        return _respond(query, state, bridge, speak, cancelled, "That action is not supported.")
    stdout, reply, was_cancelled = _prepare_draft(tool, args, settings_zip, cancelled, state)
    if was_cancelled:
        return ""
    if reply is not None:
        return _respond(query, state, bridge, speak, cancelled, reply)
    if cancelled():
        return ""
    summary, name, kind, description = draft_summary(tool, args, stdout, voice.format_status_summary)
    if tool not in MUTATIONS:
        state.active_request.clear()
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
    speak = speak_fn or (lambda text, cancel=None: voice.speak(
        text, is_cancelled=cancel, playback_event_sink=state.session_log.playback_sink(state.current_turn_id) if state.session_log else None))
    with state.turn_lock:
        if cancelled():
            return ""
        decision = confirmation(query)
        answer_to_active = decision is True and state.pending is None and state.active_request.active
        if not answer_to_active:
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
        progress = advance_request(state.active_request, intent, query, state.current_turn_id)
        if progress["kind"] == "clarify":
            _log(state, "request_clarification", field=progress["field"], action=state.active_request.action)
            return _respond(query, state, bridge, speak, cancelled, progress["text"])
        if progress["kind"] == "reply":
            return _respond(query, state, bridge, speak, cancelled, progress["text"])
        return _route_intent(query, settings_zip, auto_confirm, bridge, state, speak,
                             cancelled, confirmation_id, progress["intent"])
