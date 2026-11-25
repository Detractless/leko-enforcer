"""Guardrails and end-to-end checks for the native add-on runner."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
BINARY = ROOT / "target/debug/leko"
PROTECTED_HASH = "92aaff9716e775ab77c0f10c332395ecfb81daeff9194df8542a57b4c7728b4e"


def digest(data):
    return hashlib.sha256(data).hexdigest()


class AddonRunnerTests(unittest.TestCase):
    def test_guardrails_and_addon_cli_end_to_end_hold(self):
        touched = [
            "core/src/services/runner.rs", "core/src/services/recipe.rs",
            "core/src/services/snapshot_pin.rs", "cli/src/services/addon.rs",
            "core/tests/native_verifier.rs",
        ]
        for relative in touched:
            path = ROOT / relative
            text = path.read_text(encoding="utf-8")
            self.assertLess(len(text.splitlines()), 350, relative)
            self.assertNotIn("\u2014", text, relative)
            self.assertNotIn("/var/lib/leko", text, relative)
            self.assertNotIn("8765", text, relative)
        for folder in ("core/src/services", "cli/src/services", "core/tests", "cli/tests"):
            self.assertLessEqual(sum(1 for _ in (ROOT / folder).iterdir()), 8, folder)

        listed = subprocess.run(
            ["git", "ls-files", "src/daemon", "src/blocker-extension", "src/page-regions",
             "enforcers", "voice/audio_io", "voice/normalization"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        )
        names = sorted(listed.stdout.splitlines())
        self.assertEqual(len(names), 66)
        combined = hashlib.sha256()
        for name in names:
            combined.update(name.encode("utf-8"))
            combined.update(b"\0")
            combined.update((ROOT / name).read_bytes())
        self.assertEqual(combined.hexdigest(), PROTECTED_HASH)

        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            source = base / "source"
            source.mkdir()
            snapshots = base / "snapshots"
            payload = b"payload"
            config = {}
            manifest = {
                "version": 1,
                "addon_id": "checker",
                "config_sha256": digest(json.dumps(config, separators=(",", ":")).encode()),
                "files": [{"name": "addon.bin", "sha256": digest(payload)}],
            }
            (source / "addon.bin").write_bytes(payload)
            (source / "manifest.json").write_text(json.dumps(manifest))
            combined_text = "{}|{}|{}".format(1, "checker", manifest["config_sha256"])
            combined_text += "|addon.bin:{}".format(manifest["files"][0]["sha256"])
            fingerprint = digest(combined_text.encode())

            def run(*arguments):
                result = subprocess.run(
                    [str(BINARY), *arguments, "--snapshot-root", str(snapshots)],
                    capture_output=True, text=True, timeout=10,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                return json.loads(result.stdout)

            pinned = run("pin-addon", "--addon-id", "checker", "--sha256", fingerprint,
                         "--addon-source", str(source), "--json")
            self.assertEqual(pinned["addon_id"], "checker")
            listed_addons = run("list-addons", "--json")
            self.assertEqual(len(listed_addons["addons"]), 1)
            verified = run("verify-addon", "--addon-id", "checker", "--sha256", fingerprint, "--json")
            self.assertTrue(verified["verified"])
            for path in snapshots.rglob("*"):
                path.chmod(0o755 if path.is_dir() else 0o644)
            snapshots.chmod(0o755)


if __name__ == "__main__":
    unittest.main()
