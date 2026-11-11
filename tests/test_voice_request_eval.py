"""Held-out replay metrics and production/DSPy separation checks."""
import contextlib
import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))
import bench_llm
from bench_llm import compare, load_cases


class VoiceRequestEvalTests(unittest.TestCase):
    def test_reports_held_out_workflow_metrics(self):
        cases = load_cases(ROOT / "tests/fixtures/voice_request_cases.jsonl")
        self.assertTrue(cases)
        proposals = {
            "Block reddit": ("add_block", {"sites": ["reddit.com"]}, {"sites": "reddit"}),
            "For 30 minutes": ("add_block", {"schedule_type": "for_duration", "duration": "30m"},
                                {"schedule_type": "for 30 minutes", "duration": "30 minutes"}),
            "Focus for 25 minutes": ("start_session", {"length": "25m"}, {"length": "25 minutes"}),
            "Actually make it 45 minutes": ("start_session", {"length": "45m"},
                                            {"length": "45 minutes"}),
            "Block reddit every weekday from 9 a.m. to 5 p.m.": (
                "add_block", {"sites": ["reddit.com"], "schedule_type": "blocked_hours",
                              "blocked_hours": "09:00-17:00", "days": "mon,tue,wed,thu,fri"},
                {"sites": "reddit", "schedule_type": "every weekday", "blocked_hours": "9 a.m. to 5 p.m.",
                 "days": "every weekday"}),
            "Make that every day": ("add_block", {"days": "mon,tue,wed,thu,fri,sat,sun"},
                                    {"days": "every day"}),
            "Turn off the block called bedtime": ("switch_off", {"id": "bedtime"},
                                                  {"id": "bedtime"}),
            "Is reddit blocked?": ("check_status", {"target_type": "site", "name": "reddit.com"},
                                   {"target_type": "reddit", "name": "reddit"}),
        }

        def structured(text, **_):
            action, fields, evidence = proposals[text]
            return {"action": "request_update", "intent": action, "fields": fields,
                    "evidence": evidence}

        def baseline(text, **_):
            action, fields, _ = proposals[text]
            args = dict(fields)
            if action == "add_block":
                args.setdefault("name", "Block reddit")
            return {"action": "tool_call", "tool": action, "arguments": args}

        report = compare(cases, baseline, structured)
        metrics = report["candidate"]
        for key in ("field_accuracy", "redundant_question_rate", "correction_success_rate",
                    "unconfirmed_mutation_count", "latency", "case_results"):
            self.assertIn(key, metrics)
        self.assertEqual(report["split"], "held_out")
        self.assertEqual(report["cli_writes_enabled"], False)
        self.assertEqual(metrics["unconfirmed_mutation_count"], 0)
        self.assertEqual(metrics["field_accuracy"], 1.0, metrics["case_results"])
        self.assertEqual(metrics["redundant_question_rate"], 0.0)
        self.assertEqual(metrics["correction_success_rate"], 1.0)
        self.assertEqual(metrics["unsafe_action_proposals"], 0)
        self.assertEqual(metrics["replayed_turns"], sum(len(case["turns"]) for case in cases))

    def test_dspy_experiment_cannot_change_production_routing(self):
        assistant = (ROOT / "voice/assistant.py").read_text()
        router = (ROOT / "voice/llm_router.py").read_text()
        request = (ROOT / "voice/llm_request.py").read_text()
        query_flow = (ROOT / "voice/session/query_flow.py").read_text()
        bench = (ROOT / "tests/bench_llm.py").read_text()
        self.assertNotIn("import dspy", assistant + router + request + query_flow)
        self.assertIn("import dspy", bench)
        self.assertIn("parse_request_update_safe", query_flow)
        self.assertIn('if __name__ == "__main__"', bench)

    def test_dspy_setup_failure_still_reports_baseline(self):
        output = io.StringIO()
        with patch.object(sys, "argv", ["bench_llm.py", "--candidate", "dspy"]), \
                patch.object(bench_llm, "build_dspy_adapter",
                             side_effect=ModuleNotFoundError("DSPy not installed")), \
                patch.object(bench_llm, "parse_user_intent",
                             return_value={"action": "reply", "content": "fake"}), \
                contextlib.redirect_stdout(output):
            bench_llm.main()
        report = json.loads(output.getvalue())
        self.assertIn("DSPy not installed", report["candidate_error"])
        self.assertEqual(report["baseline"]["replayed_turns"],
                         sum(len(case["turns"]) for case in load_cases(
                             ROOT / "tests/fixtures/voice_request_cases.jsonl")))


if __name__ == "__main__":
    unittest.main()
