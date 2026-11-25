"""Guardrails and safe binary checks for live passes."""

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


class NativePassTests(unittest.TestCase):
    def test_guardrails_and_protected_paths_hold(self):
        for relative in ("core/src/services/pass_state.rs", "core/src/models/pass.rs",
                         "cli/src/commands/request_pass.rs", "cli/src/commands/check_status.rs",
                         "cli/src/commands/edit_rule.rs"):
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
        for folder in ("core/src/services", "core/tests", "cli/src/commands", "cli/tests",
                       "tests/python/cli"):
            self.assertLessEqual(sum(1 for _ in (ROOT / folder).iterdir()), 8, folder)
        count, digest = protected_digest()
        self.assertEqual(count, 66)
        self.assertEqual(digest, PROTECTED_HASH)

    def test_release_binary_and_suites_are_verified_externally(self):
        self.assertTrue(BINARY.exists(), "build the release binary before this check")
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            store = base / "rules.json"
            state = base / "pass-state.json"
            rule = {
                "id": "r1", "name": "Study", "enabled": True,
                "target": {"kind": "specific_apps", "apps": ["a"]},
                "schedule": {"time_based": {"kind": "always_on_schedule"}, "usage_based": None},
                "penalty": "force_close_penalty",
                "pass": {"kind": "earn_pass", "requirement": "physical_pass",
                         "length": {"kind": "timed_pass", "minutes": 1},
                         "cap": {"kind": "unlimited_passes"}},
                "seal": {"kind": "no_seal"},
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-01-01T00:00:00Z",
            }
            store.write_text(json.dumps({"version": 1, "rules": [rule]}))
            before = store.read_bytes()
            result = subprocess.run(
                [str(BINARY), "request-pass", "--store", str(store), "--id", "r1",
                 "--pass-state", str(state), "--json", "--at", "2026-01-01T12:00:00Z"],
                capture_output=True, text=True, timeout=10,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(result.stdout)
            self.assertTrue(value["active"])
            self.assertTrue(state.exists())
            self.assertEqual(store.read_bytes(), before)

    def test_shape_limits_and_protected_paths_hold(self):
        for relative in ("ui/src/main.rs", "ui/src/state.rs", "ui/src/web/pill.html",
                         "ui/src/web/pill.css", "ui/src/web/pill.js", "ui/src/web/tokens.css",
                         "cli/src/services/enforcement_loop.rs", "voice/core/ui_bridge.py"):
            text = (ROOT / relative).read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
        for folder in ("ui", "ui/src", "ui/src/web", "tests/ui", "cli/src/services", "core/tests"):
            self.assertLessEqual(sum(1 for _ in (ROOT / folder).iterdir()), 8, folder)
        self.assertEqual(protected_digest()[1], PROTECTED_HASH)
        self.assertFalse((ROOT / "ui/web").exists())
        self.assertFalse((ROOT / "ui/pill").exists())


if __name__ == "__main__":
    unittest.main()
