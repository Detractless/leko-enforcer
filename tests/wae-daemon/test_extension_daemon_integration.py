"""Automated end-to-end integration test simulating the Blocker Extension against the WAE daemon."""

import json
from pathlib import Path
import socket
import subprocess
import sys
import time
import unittest
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

DAEMON_SCRIPT = ROOT / "src/daemon/server.py"
PYTHON_BIN = Path(sys.executable) if "sys" in globals() else Path("python3")


class TestExtensionDaemonIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Find a free port for the test daemon
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            cls.port = s.getsockname()[1]

        # Start daemon with custom port
        cls.daemon_proc = subprocess.Popen(
            [
                str(PYTHON_BIN),
                "-c",
                f"""
from src.daemon.server import create_server, SERVICE
server = create_server('127.0.0.1', {cls.port})
try:
    server.serve_forever()
finally:
    SERVICE.stop_llama_server()
""",
            ],
            cwd=str(ROOT),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for daemon to be healthy
        url = f"http://127.0.0.1:{cls.port}/health"
        healthy = False
        for _ in range(40):
            try:
                with urllib.request.urlopen(url, timeout=1) as resp:
                    if resp.status == 200:
                        healthy = True
                        break
            except Exception:
                time.sleep(0.3)
        if not healthy:
            cls.daemon_proc.terminate()
            raise RuntimeError(f"Daemon failed to start on port {cls.port}")

        # Warm up the daemon pipeline so cold model load doesn't hit client timeout
        warmup_req = urllib.request.Request(
            f"http://127.0.0.1:{cls.port}/api/evaluate",
            data=json.dumps({"url": "https://example.com", "html": "<html><body>Warmup</body></html>"}).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        try:
            with urllib.request.urlopen(warmup_req, timeout=60) as resp:
                resp.read()
        except Exception as e:
            print("Warmup notice:", e)

    @classmethod
    def tearDownClass(cls):
        if cls.daemon_proc:
            cls.daemon_proc.terminate()
            try:
                cls.daemon_proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                cls.daemon_proc.kill()

    def _post_evaluate(self, payload: dict) -> dict:
        req = urllib.request.Request(
            f"http://127.0.0.1:{self.port}/api/evaluate",
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def test_extension_full_lifecycle_challenge_then_stream_block(self):
        # Step 1: Extension intercepts Cloudflare challenge page
        challenge_payload = {
            "url": "https://tubitv.com/movies",
            "title": "Just a moment...",
            "html": (
                "<html><head><title>Just a moment...</title></head>"
                "<body><div id='challenge-running'><div class='cf-turnstile'></div>"
                "Checking if the site connection is secure. Ray ID: 8abc999</div></body></html>"
            ),
            "text": "Checking if the site connection is secure.",
            "images": [],
        }
        res1 = self._post_evaluate(challenge_payload)
        self.assertEqual(res1["verdict"], "PENDING")
        self.assertIn("challenge", res1["reason"])
        self.assertGreaterEqual(res1["retry_in_ms"], 1000)

        # Step 2: Challenge clears; extension re-extracts settled DOM and re-submits
        settled_payload = {
            "url": "https://tubitv.com/movies",
            "title": "Watch Free Movies & TV Shows Online | Tubi",
            "html": (
                "<html><head><title>Watch Free Movies & TV Shows Online | Tubi</title></head>"
                "<body><h1>Tubi TV Streaming</h1>"
                "<p>Stream thousands of free full length movies and TV series episodes online on demand.</p>"
                "</body></html>"
            ),
            "text": "Stream thousands of free full length movies and TV series episodes online on demand.",
            "images": [],
        }
        res2 = self._post_evaluate(settled_payload)
        self.assertEqual(res2["verdict"], "BLOCK")
        self.assertEqual(res2["policy_triggered"], "streaming")
        self.assertIn("tubi", res2["summary"].lower())

    def test_extension_educational_control_allowed(self):
        control_payload = {
            "url": "https://en.wikipedia.org/wiki/Ada_Lovelace",
            "title": "Ada Lovelace - Wikipedia",
            "html": (
                "<html><head><title>Ada Lovelace - Wikipedia</title></head>"
                "<body><h1>Ada Lovelace</h1>"
                "<p>Augusta Ada King, Countess of Lovelace was an English mathematician and writer, "
                "chiefly known for her work on Charles Babbage's mechanical general-purpose computer, "
                "the Analytical Engine. She wrote the first algorithm intended to be carried out by a machine.</p>"
                "</body></html>"
            ),
            "text": "Augusta Ada King, Countess of Lovelace was an English mathematician and writer.",
            "images": [],
        }
        res = self._post_evaluate(control_payload)
        self.assertEqual(res["verdict"], "ALLOW")
        self.assertIsNone(res["policy_triggered"])

    def test_extension_benson_boone_allowed(self):
        profile_payload = {
            "url": "https://www.menshealth.com/entertainment/a63636599/who-is-benson-boone-grammys/",
            "title": "Uh, Who Exactly Is Benson Boone?",
            "html": (
                "<html><head><title>Uh, Who Exactly Is Benson Boone?</title></head>"
                "<body><h1>Uh, Who Exactly Is Benson Boone?</h1>"
                "<p>By Evan Romano. Published on February 3, 2025. "
                "It covers Benson Boone, nominated for Best New Artist and performing his song while "
                "grabbing his crotch at the 2025 Grammys.</p>"
                "</body></html>"
            ),
            "text": "By Evan Romano. It covers Benson Boone performing his song at the 2025 Grammys.",
            "images": [],
        }
        res = self._post_evaluate(profile_payload)
        self.assertEqual(res["verdict"], "ALLOW")
        self.assertIsNone(res["policy_triggered"])


if __name__ == "__main__":
    unittest.main()
