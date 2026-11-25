"""Exercise the native CLI rule creation against a temporary store."""

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
LEKO = ROOT / "target" / "debug" / "leko"


@unittest.skipUnless(LEKO.exists(), "build leko-cli with cargo test first")
class NativeCliPreviewTests(unittest.TestCase):
    def test_device_rule_uses_canonical_target(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            result = subprocess.run(
                [str(LEKO), "create-rule", "--store", str(store), "--id", "w1",
                 "--name", "Night Lock",
                 "--target", json.dumps({"kind": "whole_device", "action": "lock"}),
                 "--at", "2026-01-01T00:00:00Z"],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            saved = json.loads(store.read_text(encoding="utf-8"))
            self.assertEqual(saved["rules"][0]["target"]["kind"], "whole_device")
            self.assertIsNone(saved["rules"][0]["penalty"])

    def test_list_json_has_stable_pillar_shape(self):
        with tempfile.TemporaryDirectory() as folder:
            store = Path(folder) / "rules.json"
            result = subprocess.run(
                [str(LEKO), "list-rules", "--store", str(store), "--json"],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            value = json.loads(result.stdout)
            self.assertIn("rules", value)


if __name__ == "__main__":
    unittest.main()
