"""Persistence and query-flow coverage for local session diagnostics."""
from contextlib import redirect_stderr
from datetime import datetime, timezone
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "voice"))
import llm_router

fake_voice = sys.modules.get("assistant") or types.ModuleType("assistant")
fake_voice.LEKO_BIN = "FAKE-LEKO"
fake_voice.tool_to_cli_args = llm_router.tool_to_cli_args
fake_voice.verbalize_for_speech = lambda text: text
fake_voice.format_status_summary = lambda output, *_: output
fake_voice.parse_user_intent_safe = lambda *_args, **_kwargs: {"action": "reply", "content": "Ready."}
fake_voice.speak = lambda *_args, **_kwargs: None
sys.modules["assistant"] = fake_voice

from memory.local_store import LocalMemory
from session.conversation import Conversation
from session.query_flow import process_query
from session.session_log import SCHEMA_VERSION, SessionLog


STAMP = datetime(2026, 11, 9, 12, 0, tzinfo=timezone.utc)


class SessionLogTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "session-logs"

    def test_session_log_persists_across_restart_without_overwrite(self):
        first = SessionLog(self.root, "first", STAMP)
        first.append("stt_result", "turn-1", text="Original transcript")
        old_bytes = first.path.read_bytes()
        second = SessionLog(self.root, "second", STAMP)
        second.append("assistant_reply", "turn-2", text="Later reply")
        records, warning = SessionLog.read_records(first.path)
        self.assertIsNone(warning)
        self.assertEqual(records[0]["details"]["text"], "Original transcript")
        self.assertEqual(first.path.read_bytes(), old_bytes)
        self.assertNotEqual(first.path, second.path)

    def test_records_stt_text_and_exact_submitted_text(self):
        log = SessionLog(self.root, "input", STAMP)
        log.append("stt_result", "turn-1", text="raw recognizer words", result="text")
        log.append("user_query_submitted", "turn-1", text="final edited request", source="voice")
        records, _ = SessionLog.read_records(log.path)
        self.assertEqual(records[0]["details"]["text"], "raw recognizer words")
        self.assertEqual(records[1]["details"]["text"], "final edited request")

    def test_session_logs_are_private_local_files(self):
        log = SessionLog(self.root, "private", STAMP)
        self.assertEqual(log.path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(log.directory.stat().st_mode & 0o777, 0o700)
        self.assertIn("session-logs", str(log.path))

    def test_archived_sessions_are_not_loaded_as_memory_or_prompt_context(self):
        log = SessionLog(self.root, "archive", STAMP)
        log.append("user_query_submitted", "turn-1", text="secret archived phrase")
        memory = LocalMemory(Path(self.temp.name) / "memory.sqlite3")
        self.assertEqual(memory.retrieve("secret archived phrase"), [])
        messages = llm_router._build_messages("new request", [], memory.retrieve("new request"), "system")
        self.assertNotIn("secret archived phrase", json.dumps(messages))

    def test_partial_event_write_preserves_prior_records(self):
        log = SessionLog(self.root, "partial", STAMP)
        log.append("first", "turn-1", value=1)
        before = log.path.read_bytes()
        real_write = os.write
        calls = 0

        def short_then_fail(fd, content):
            nonlocal calls
            calls += 1
            if calls == 1:
                return real_write(fd, content[:4])
            raise OSError("disk full")

        with redirect_stderr(io.StringIO()), patch("session.session_log.os.write", side_effect=short_then_fail):
            self.assertFalse(log.append("second", "turn-2", value=2))
        self.assertEqual(log.path.read_bytes(), before)
        records, warning = SessionLog.read_records(log.path)
        self.assertIsNone(warning)
        self.assertEqual([row["event"] for row in records], ["first"])

    def test_missing_empty_and_corrupt_logs_are_handled_without_overwrite(self):
        log = SessionLog(self.root, "damaged", STAMP)
        self.assertEqual(SessionLog.read_records(log.path), ([], None))
        log.path.write_bytes(b'{"schema_version":1,"event":"valid"}\n{"broken"')
        damaged_bytes = log.path.read_bytes()
        with redirect_stderr(io.StringIO()) as errors:
            SessionLog(self.root, "fresh", STAMP)
        self.assertIn("incomplete final record", errors.getvalue())
        self.assertEqual(log.path.read_bytes(), damaged_bytes)
        rows, warning = SessionLog.read_records(log.path)
        self.assertEqual(len(rows), 1)
        self.assertIn("incomplete final record", warning)

    def test_versioned_session_records_remain_readable(self):
        log = SessionLog(self.root, "versions", STAMP)
        log.path.write_text(json.dumps({"schema_version": SCHEMA_VERSION, "event": "legacy"}) + "\n")
        rows, warning = SessionLog.read_records(log.path)
        self.assertIsNone(warning)
        self.assertEqual(rows[0]["event"], "legacy")
        before = log.path.read_bytes()
        log.path.write_text(json.dumps({"schema_version": SCHEMA_VERSION + 1}) + "\n")
        unsupported = log.path.read_bytes()
        rows, warning = SessionLog.read_records(log.path)
        self.assertEqual(rows, [])
        self.assertIn("unsupported schema version", warning)
        self.assertNotEqual(log.path.read_bytes(), before)
        self.assertEqual(log.path.read_bytes(), unsupported)

    def test_records_tool_confirmation_result_and_playback(self):
        log = SessionLog(self.root, "tool-flow", STAMP)
        state = Conversation(LocalMemory(Path(self.temp.name) / "facts.sqlite3"))
        state.session_log = log
        intent = {"action": "tool_call", "tool": "add_block", "arguments": {
            "name": "Social", "sites": ["reddit.com"],
            "schedule_type": "for_duration", "duration": "30m"}}

        def model(query, history=None, memories=None, trace_callback=None):
            if trace_callback:
                trace_callback({"status": "ok", "request": {"messages": [{"role": "user", "content": query}]},
                                "response": {"choices": [{"message": {"reasoning_content": "Select add_block.",
                                "tool_calls": [{"function": {"name": "add_block", "arguments": "{}"}}]}}]}})
            return intent

        def speak(text, is_cancelled=None, playback_event_sink=None):
            if playback_event_sink:
                playback_event_sink({"status": "completed", "text": text})

        cli_result = types.SimpleNamespace(returncode=0, stdout="preview or write succeeded", stderr="")
        with patch.object(fake_voice, "parse_user_intent_safe", side_effect=model), \
                patch.object(fake_voice, "speak", side_effect=speak), \
                patch("session.query_flow.subprocess.run", return_value=cli_result):
            process_query("block reddit for 30 minutes", "settings.zip", conversation=state,
                          session_log=log, turn_id="turn-1", input_source="voice")
            action_id = state.pending["id"]
            process_query("yes", "settings.zip", conversation=state, confirmation_id=action_id,
                          session_log=log, turn_id="turn-2", input_source="voice")
            process_query("block reddit for 30 minutes", "settings.zip", conversation=state,
                          session_log=log, turn_id="turn-3", input_source="voice")
            process_query("no", "settings.zip", conversation=state, session_log=log,
                          turn_id="turn-4", input_source="voice")

        rows, warning = SessionLog.read_records(log.path)
        self.assertIsNone(warning)
        events = [row["event"] for row in rows]
        for expected in ("model_call", "tool_preview", "tool_draft_staged", "tool_confirmation",
                         "tool_execution", "assistant_reply", "playback"):
            self.assertIn(expected, events)
        trace = next(row["details"]["trace"] for row in rows if row["event"] == "model_call")
        self.assertEqual(trace["response"]["choices"][0]["message"]["reasoning_content"], "Select add_block.")
        execution = next(row for row in rows if row["event"] == "tool_execution")
        self.assertEqual(execution["details"]["status"], "ok")
        decisions = [row["details"]["accepted"] for row in rows if row["event"] == "tool_confirmation"]
        self.assertEqual(decisions, [True, False])


if __name__ == "__main__":
    unittest.main()
