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


if __name__ == "__main__":
    unittest.main()
