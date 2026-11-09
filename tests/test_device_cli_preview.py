"""Exercise the production release CLI's device-block draft without saving settings."""
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEKO = ROOT / "target" / "release" / "leko"


@unittest.skipUnless(LEKO.exists(), "build leko-cli in release mode first")
class DeviceCliPreviewTests(unittest.TestCase):
    def make_settings(self, folder):
        settings = Path(folder) / "settings.zip"
        with zipfile.ZipFile(settings, "w") as archive:
            archive.writestr("GUIDE.md", "guide")
            archive.writestr("general.json", '{"schema": 2}')
        return settings

    def test_device_flag_builds_a_draft_without_writing(self):
        with tempfile.TemporaryDirectory() as folder:
            settings = self.make_settings(folder)
            original = settings.read_bytes()

            result = subprocess.run(
                [str(LEKO), "add-block", str(settings), "--name", "Night Lock",
                 "--device", "lock", "--for", "7h"],
                capture_output=True, text=True, timeout=10)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("the whole computer: the screen is locked", result.stdout)
            self.assertIn("Nothing was saved", result.stdout)
            self.assertEqual(settings.read_bytes(), original)

    def test_complemented_blocked_hours_preview_does_not_write(self):
        with tempfile.TemporaryDirectory() as folder:
            settings = self.make_settings(folder)
            original = settings.read_bytes()
            result = subprocess.run(
                [str(LEKO), "add-block", str(settings), "--name", "Night Lock",
                 "--device", "lock", "--open-hours", "07:00-00:00",
                 "--days", "mon,tue,wed,thu,fri,sat,sun"],
                capture_output=True, text=True, timeout=10)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("open only during 07:00-00:00", result.stdout)
            self.assertIn("Nothing was saved", result.stdout)
            self.assertEqual(settings.read_bytes(), original)


if __name__ == "__main__":
    unittest.main()
