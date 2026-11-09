"""Opt-in, slow local LFM3 evaluation; never invokes the CLI or applies an action."""
import os
import sys
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "voice"))
from llm_router import parse_user_intent

AMBIGUOUS_WINDOW = (
    "I want a computer block starting at midnight until 4 a.m. No, sorry, 7 a.m. "
    "I can't use my whole computer from 4 to 7. Can you do that?"
)


@unittest.skipUnless(os.environ.get("LEKO_RUN_LLM_EVAL") == "1",
                     "set LEKO_RUN_LLM_EVAL=1 to run the slow local-model evaluation")
class LocalLlmEvalTests(unittest.TestCase):
    def test_ambiguous_overnight_window_requests_clarification(self):
        started = time.perf_counter()
        result = parse_user_intent(AMBIGUOUS_WINDOW, history=[], memories=[])
        elapsed = time.perf_counter() - started
        print(f"[LFM3 eval] elapsed={elapsed:.2f}s action={result.get('action')} result={result}")
        self.assertEqual(result.get("action"), "reply",
                         f"expected a clarification before an action after {elapsed:.2f}s; got {result}")


if __name__ == "__main__":
    unittest.main()
