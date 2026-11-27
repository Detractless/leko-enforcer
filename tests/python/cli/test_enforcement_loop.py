"""Guardrails and safe binary checks for the native enforcement loop."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
BINARY = ROOT / "target/release/leko"
PROTECTED_HASH = "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e"


def protected_digest():
    listed = subprocess.run(
        ["git", "ls-files", "src/daemon", "src/blocker-extension", "src/page-regions",
         "enforcers", "voice/audio_io", "voice/normalization"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    names = sorted(listed.stdout.splitlines())
    combined = hashlib.sha256()
    for name in names:
        combined.update(name.encode("utf-8"))
        combined.update(b"\0")
        combined.update((ROOT / name).read_bytes())
    return len(names), combined.hexdigest()


class EnforcementLoopTests(unittest.TestCase):
    def test_shape_limits_and_protected_paths_hold(self):
        for relative in ("cli/src/services/enforcement_loop.rs",
                         "cli/src/services/os_executor.rs",
                         "cli/tests/status_evaluate.rs", "cli/tests/arguments.rs",
                         "cli/tests/guardrails.rs", "cli/tests/store_atomic.rs"):
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
        for folder in ("cli/src/services", "cli/tests", "core/tests", "tests/python/cli"):
            self.assertLessEqual(sum(1 for _ in (ROOT / folder).iterdir()), 8, folder)
        count, digest = protected_digest()
        self.assertEqual(count, 66)
        self.assertEqual(digest, PROTECTED_HASH)

    def test_no_live_targets_are_touched(self):
        forbidden = ("Projects/leko-enforcer", "LaunchAgents", "var/lib/leko", "8765")
        for relative in ("cli/src/services/enforcement_loop.rs",
                         "cli/src/services/os_executor.rs",
                         "cli/tests/status_evaluate.rs", "cli/tests/arguments.rs",
                         "cli/tests/guardrails.rs", "cli/tests/store_atomic.rs"):
            text = (ROOT / relative).read_text(encoding="utf-8")
            for term in forbidden:
                self.assertNotIn(term, text, "{} contains {}".format(relative, term))

    def test_release_binary_and_suites_are_verified_externally(self):
        self.assertTrue(BINARY.exists(), "build the release binary before this check")
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            store.write_text(json.dumps({"version": 1, "rules": []}))
            result = subprocess.run(
                [str(BINARY), "enforce", str(store), "--once", "--dry-run",
                 "--at", "2026-01-01T12:00:00Z"],
                capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Enforcement ticks: 1", result.stdout)

    def m9_shape_limits_and_protected_paths_hold(self):
        m9_files = (
            "core/src/services/usage_state.rs",
            "core/src/domain/usage_eval.rs",
            "cli/src/services/usage_tracker.rs",
            "cli/src/services/os_executor.rs",
            "cli/src/services/enforcement_loop.rs",
            "cli/tests/status_evaluate.rs",
            "cli/tests/output_modes.rs",
            "cli/tests/pass_request.rs",
            "cli/tests/arguments.rs",
            "cli/tests/store_atomic.rs",
            "ui/src/timer_poll.rs",
            "ui/src/main.rs",
            "voice/routing/canonical_speech.py",
            "voice/routing/native_cli.py",
            "voice/routing/llm_router.py",
        )
        for relative in m9_files:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
        runtime_lines = len((ROOT / "voice/session/runtime.py").read_text(encoding="utf-8").splitlines())
        self.assertLessEqual(runtime_lines, 349)
        for folder in ("core/src/services", "cli/src/services", "cli/tests", "core/tests",
                       "ui/src", "tests/python/cli", "tests/python/routing"):
            count = sum(1 for p in (ROOT / folder).iterdir() if p.name != "__pycache__")
            self.assertLessEqual(count, 8, folder)
        count, digest = protected_digest()
        self.assertEqual(count, 66)
        self.assertEqual(digest, PROTECTED_HASH)

    def m9_no_live_targets_are_touched(self):
        forbidden = ("Projects/leko-enforcer", "LaunchAgents", "var/lib/leko", "8765")
        for relative in ("core/src/services/usage_state.rs", "cli/src/services/usage_tracker.rs",
                         "cli/src/services/os_executor.rs", "cli/src/services/enforcement_loop.rs",
                         "ui/src/timer_poll.rs", "ui/src/main.rs"):
            text = (ROOT / relative).read_text(encoding="utf-8")
            for term in forbidden:
                self.assertNotIn(term, text, f"{relative} contains {term}")
        res = subprocess.run([str(BINARY), "settings", "--json"], capture_output=True, text=True, check=True)
        settings = json.loads(res.stdout)
        self.assertEqual(settings["shadow_mode"], False)
        self.assertEqual(settings["nsfw"], False)

    def m9_release_binary_and_tracker_smoke_verified(self):
        self.assertTrue(BINARY.exists())
        self.assertTrue((ROOT / "target/release/leko-pill").exists())
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            created = subprocess.run([
                str(BINARY), "create-rule", "--store", str(store), "--id", "r1", "--name", "Focus",
                "--target", '{"kind":"specific_apps","apps":["com.example.Bad"]}',
                "--schedule", '{"time_based":{"kind":"always_on_schedule"},"usage_based":{"kind":"time_limit","minutes":5,"period":"daily"}}',
                "--penalty", '"force_close_penalty"',
                "--engagement-pass", "--engagement-targets", "com.apple.Notes", "--engagement-seconds", "120",
                "--at", "2026-01-01T12:00:00Z",
            ], capture_output=True, text=True, check=True)
            self.assertEqual(created.returncode, 0)
            tick = subprocess.run([
                str(BINARY), "enforce", str(store), "--once", "--dry-run", "--at", "2026-01-01T12:00:00Z",
            ], capture_output=True, text=True, check=True)
            self.assertIn("Enforcement ticks: 1", tick.stdout)
            status = subprocess.run([
                str(BINARY), "check-status", "--store", str(store), "--id", "r1",
                "--at", "2026-01-01T12:00:00Z", "--json",
            ], capture_output=True, text=True, check=True)
            data = json.loads(status.stdout)
            self.assertEqual(data["usage"]["kind"], "Time Limit")
            self.assertEqual(data["usage"]["remaining_seconds"], 300)

    def test_m9_shape_limits_and_protected_paths_hold(self):
        self.m9_shape_limits_and_protected_paths_hold()

    def test_m9_no_live_targets_are_touched(self):
        self.m9_no_live_targets_are_touched()

    def test_m9_release_binary_and_tracker_smoke_verified(self):
        self.m9_release_binary_and_tracker_smoke_verified()

    def test_countdowns_and_voice_shape_and_protected_paths_hold(self):
        files = (
            "core/src/domain/schedule_eval.rs", "core/src/domain/usage_eval.rs", "core/src/lib.rs",
            "cli/src/arguments.rs", "cli/src/commands/mod.rs", "cli/src/commands/check_status.rs",
            "cli/src/services/enforcement_loop.rs",
            "ui/src/main.rs", "ui/src/state.rs", "ui/src/card.rs", "ui/src/timer_poll.rs",
            "ui/src/web/pill.html", "ui/src/web/pill.css", "ui/src/web/card.css", "ui/src/web/pill.js",
            "voice/routing/deterministic_router.py", "voice/routing/native_cli.py",
            "voice/routing/llm_request.py", "voice/routing/llm_router.py",
            "voice/core/ui_bridge.py", "voice/session/query_flow.py", "voice/session/runtime.py",
        )
        for relative in files:
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
        for folder in ("core/src", "core/src/domain", "cli/src/commands", "cli/src/services",
                       "ui/src", "ui/src/web", "voice/routing", "voice/session"):
            count = sum(1 for p in (ROOT / folder).iterdir() if p.name != "__pycache__")
            self.assertLessEqual(count, 8, folder)
        count, digest = protected_digest()
        self.assertEqual(count, 66)
        self.assertEqual(digest, PROTECTED_HASH)


if __name__ == "__main__":
    unittest.main()
