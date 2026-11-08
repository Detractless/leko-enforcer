"""Conversation routing and deterministic, single-use action confirmations."""
from typing import Any
import sqlite3
import subprocess
import assistant as voice
from session.conversation import Conversation, confirmation

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


def _process_query(query, settings_zip, auto_confirm=False, bridge=None,
                  is_cancelled=None, speak_fn=None, conversation=None, confirmation_id=None):
    state = conversation if conversation is not None else DEFAULT_CONVERSATION
    epoch = state.epoch
    cancelled = lambda: state.epoch != epoch or bool(is_cancelled and is_cancelled())
    speak = speak_fn or (lambda text, cancel=None: voice.speak(text, is_cancelled=cancel))

    def respond(text, confirmed=False):
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

    def execute_pending(expected_id=None):
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

    with state.turn_lock:
        if cancelled():
            return ""
        decision = confirmation(query)
        if decision is not None:
            if decision:
                text, success = execute_pending(confirmation_id)
                return respond(text, success)
            with state.lock:
                if confirmation_id is not None and (not state.pending or state.pending["id"] != confirmation_id):
                    return respond("There is no matching pending action to cancel.")
                state.pending = None
                if bridge:
                    bridge.emit({"type": "reset"})
            return respond("Action dismissed.")

        # A new request invalidates the old draft, but history still resolves revisions.
        with state.lock:
            state.pending = None
            if bridge:
                bridge.emit({"type": "reset"})
        try:
            memory_reply = state.memory.command(query)
            if memory_reply is not None:
                # Forget/correct also remove old facts from ephemeral conversation context.
                state.history.clear()
                return respond(memory_reply)
            memories = state.memory.retrieve(query)
        except (OSError, sqlite3.Error):
            return respond("Local memory is unavailable. Please check its storage path.")
        if bridge:
            bridge.emit_thinking(query)
        intent = voice.parse_user_intent_safe(query, history=list(state.history), memories=memories)
        if cancelled():
            return ""
        if intent.get("action") == "reply":
            return respond(intent.get("content") or "Please rephrase that request.")
        tool, args = intent.get("tool"), intent.get("arguments", {})
        if tool not in MUTATIONS | {"check_status"} or not isinstance(args, dict):
            return respond("That action is not supported.")
        try:
            cli_args = voice.tool_to_cli_args(settings_zip, tool, args, write=False)
            # Session CLI launches immediately: it has no preview mode.
            stdout = ""
            if tool != "start_session":
                if cancelled():
                    return ""
                proc = subprocess.run([str(voice.LEKO_BIN)] + cli_args, capture_output=True, text=True)
                if proc.returncode != 0:
                    return respond("Sorry, that request could not be prepared.")
                stdout = proc.stdout.strip()
        except (OSError, ValueError, KeyError, TypeError):
            return respond("Sorry, that request could not be prepared.")
        if cancelled():
            return ""
        summary, name, kind, description = _draft_summary(tool, args, stdout)
        if tool not in MUTATIONS:
            return respond(summary)
        summary += " Say yes to confirm or no to cancel."
        with state.lock:
            if cancelled():
                return ""
            action_id = state.stage(intent, settings_zip)
            if bridge:
                bridge.emit_draft(name, kind, description, raw_intent=intent, action_id=action_id)
        if auto_confirm:
            text, success = execute_pending(action_id)
            return respond(text, success)
        return respond(summary)


def _draft_summary(tool: str, args: dict[str, Any], stdout: str) -> tuple[str, str, str, str]:
    if tool == "start_session":
        length = args.get("length")
        return f"Prepared a focus session of {length}.", "Focus Session", "Session", f"Unskippable session for {length}."
    if tool == "add_block":
        device = args.get("device")
        targets = list(args.get("apps", [])) + list(args.get("sites", []))
        target_str = f"the whole computer ({device})" if device else (", ".join(targets) if targets else "selected targets")
        schedule = args.get("duration") or args.get("open_hours") or args.get("schedule_type", "always")
        return f"Drafted a block for {target_str}, {schedule}.", "Block Rule", "Block", f"Blocks {target_str}."
    if tool == "switch_off":
        block_id = args.get("id")
        return f"Prepared to turn off block {block_id}.", f"Turn Off Block {block_id}", "Switch Off", f"Turns off rule {block_id}."
    if tool == "check_status":
        summary = voice.format_status_summary(stdout, args.get("target_type", ""), args.get("name", ""))
        return summary, "Status Report", "Status", summary
    return "Action prepared.", "Action", "Command", "Prepared action."
