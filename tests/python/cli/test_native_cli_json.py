"""Native CLI JSON parsing, shape guardrails, and runnable binary checks."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))

from voice.routing import native_cli
from voice.routing.canonical_speech import format_status_summary


BINARY = ROOT / "target/debug/leko"
LEGACY = ('"add-block"', '"switch-off"', '"--open-hours"', '"--wall"', '"--write"')


class NativeCliJsonTests(unittest.TestCase):
    def test_voice_parses_stable_pillar_json_not_plain_text(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            result = native_cli.run_cli(["list-rules", "--store", str(store), "--json"], BINARY)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = native_cli.parse_json(result.stdout)
            self.assertIn("rules", value)
            spoken = format_status_summary(json.dumps({"enforced": False, "reason": "NoPenalty"}))
            self.assertIn("No Penalty", spoken)
            with self.assertRaises(ValueError):
                native_cli.parse_json("reddit.com is open.")

    def test_shape_limits_and_protected_paths_hold(self):
        counts = {}
        for folder in ("voice/routing", "voice/request_state", "voice/core", "voice/session"):
            directory = ROOT / folder
            counts[folder] = sum(1 for _ in directory.iterdir())
            for path in directory.glob("*.py"):
                text = path.read_text(encoding="utf-8")
                self.assertLess(len(text.splitlines()), 350, str(path))
                self.assertNotIn("\u2014", text, str(path))
        self.assertLessEqual(max(counts.values()), 8, str(counts))

        listed = subprocess.run(
            ["git", "ls-files", "src/daemon", "src/blocker-extension", "src/page-regions",
             "enforcers", "voice/audio_io", "voice/normalization"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        )
        names = sorted(listed.stdout.splitlines())
        self.assertEqual(len(names), 66)
        digest = hashlib.sha256()
        for name in names:
            digest.update(name.encode("utf-8"))
            digest.update(b"\0")
            digest.update((ROOT / name).read_bytes())
        self.assertEqual(
            digest.hexdigest(),
            "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e",
        )

    def test_python_tests_have_no_legacy_cli_and_the_built_binary_runs(self):
        for path in (ROOT / "tests/python").rglob("test_*.py"):
            if path.name in {"test_native_cli_json.py", "test_intent_validation.py",
                             "test_voice_native_commands.py", "test_conversation.py"}:
                continue
            text = path.read_text(encoding="utf-8")
            for legacy in LEGACY:
                self.assertNotIn(legacy, text, "{} still has {}".format(path.name, legacy))
        self.assertTrue(BINARY.exists(), "build the CLI with cargo test first")
        result = native_cli.run_cli(["settings", "--json"], BINARY)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = native_cli.parse_json(result.stdout)
        self.assertFalse(value["shadow_mode"])
        self.assertFalse(value["nsfw_overridable"])


if __name__ == "__main__":
    unittest.main()
