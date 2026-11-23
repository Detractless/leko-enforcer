"""Unit and Integration tests for the WAE Local Daemon Pipeline."""

import json
from pathlib import Path
import sys
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from src.daemon.challenge_detector import detect_challenge, ChallengeResult
from src.daemon.vision_evaluator import VisionEvaluator
from src.daemon.decision_engine import DecisionEngine
from src.daemon.server import DaemonService


class TestChallengeDetector(unittest.TestCase):
    def test_cloudflare_interstitial_detected(self):
        title = "Just a moment..."
        html = '<div id="challenge-running"><div class="cf-turnstile"></div>Ray ID: 7abc123</div>'
        text = "Please wait while your request is verified."
        res = detect_challenge(title, html, text)
        self.assertTrue(res.is_challenge)
        self.assertEqual(res.retry_in_ms, 1500)

    def test_benign_content_clean(self):
        title = "Ada Lovelace - Computer Pioneer"
        html = "<html><body><h1>Ada Lovelace</h1><p>Augusta Ada King was a mathematician who wrote algorithms.</p></body></html>"
        text = "Augusta Ada King was a mathematician who wrote algorithms for the Analytical Engine."
        res = detect_challenge(title, html, text)
        self.assertFalse(res.is_challenge)

    def test_sparse_loading_detected(self):
        title = "Loading App"
        html = '<div class="spinner">Loading... Please wait.</div>'
        text = "Loading..."
        res = detect_challenge(title, html, text)
        self.assertTrue(res.is_challenge)


class TestVisionEvaluator(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.evaluator = VisionEvaluator()

    def test_blank_image_sfw(self):
        from PIL import Image
        import io, base64

        img = Image.new("RGB", (384, 384), color="white")
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        b64 = base64.b64encode(buf.getvalue()).decode("utf-8")

        score, ocr = self.evaluator.evaluate_sparse_media(b64, text_word_count=10)
        self.assertLess(score, 0.35)


class TestDecisionEngine(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.engine = DecisionEngine(device="mps")

    def test_benson_boone_allowed(self):
        summary = (
            'An article titled "Uh, Who Exactly Is Benson Boone?" by Evan Romano. '
            'It covers Benson Boone nominated for Best New Artist and performing his '
            'song while grabbing his crotch at the 2025 Grammys.'
        )
        res = self.engine.evaluate(summary)
        self.assertEqual(res.verdict, "ALLOW")
        self.assertIsNone(res.policy_triggered)

    def test_streaming_service_blocked(self):
        summary = (
            "Tubi is a free ad-supported streaming service offering thousands of movies "
            "and television show episodes available to watch on demand."
        )
        res = self.engine.evaluate(summary)
        self.assertEqual(res.verdict, "BLOCK")
        self.assertEqual(res.policy_triggered, "streaming")

    def test_control_tech_allowed(self):
        summary = (
            "Ars Technica is a technology publication providing news and analysis on "
            "computing, gadgets, policy, and scientific developments."
        )
        res = self.engine.evaluate(summary)
        self.assertEqual(res.verdict, "ALLOW")


class TestDaemonEndToEnd(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.service = DaemonService(llama_port=18767)

    @classmethod
    def tearDownClass(cls):
        cls.service.stop_llama_server()

    def test_challenge_page_returns_pending(self):
        html = "<html><head><title>Just a moment...</title></head><body><div id='challenge-running'>Ray ID: 999</div></body></html>"
        res = self.service.evaluate_page("https://example.com/cf", html, "Just a moment...")
        self.assertEqual(res["verdict"], "PENDING")
        self.assertIn("retry_in_ms", res)

    def test_streaming_page_returns_block(self):
        html = (
            "<html><head><title>Watch Free Movies Online | Tubi TV</title></head>"
            "<body><h1>Stream Movies & Full TV Episodes</h1>"
            "<p>Watch full length movies and television episodes for free on Tubi streaming network.</p>"
            "</body></html>"
        )
        res = self.service.evaluate_page("https://tubitv.com", html, "Watch Free Movies Online | Tubi TV")
        self.assertEqual(res["verdict"], "BLOCK")
        self.assertEqual(res["policy_triggered"], "streaming")
        self.assertIn("scores", res)

    def test_educational_control_returns_allow(self):
        html = (
            "<html><head><title>Ada Lovelace Biography</title></head>"
            "<body><h1>Ada Lovelace</h1>"
            "<p>Augusta Ada King, Countess of Lovelace was an English mathematician and writer, "
            "chiefly known for her work on Charles Babbage's mechanical general-purpose computer, "
            "the Analytical Engine. She wrote the first algorithm intended to be carried out by a machine.</p>"
            "</body></html>"
        )
        res = self.service.evaluate_page("https://wikipedia.org/wiki/Ada_Lovelace", html, "Ada Lovelace Biography")
        self.assertEqual(res["verdict"], "ALLOW")
        self.assertIsNone(res["policy_triggered"])


if __name__ == "__main__":
    unittest.main()
