import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from voice.settings_setup import SettingsSetupError, prepare_settings


class SettingsSetupTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.target = self.root / "Application Support" / "Leko" / "settings.zip"
        self.calls = []

    def tearDown(self):
        self.temp.cleanup()

    def runner(self, command, **_kwargs):
        words = command[1:]
        self.calls.append(words)
        if words == ["settings", "path"]:
            return SimpleNamespace(returncode=0, stdout=f"{self.target}\n", stderr="")
        if words[:2] == ["settings", "validate"]:
            path = Path(words[2])
            valid = path.is_file() and path.read_bytes().startswith(b"valid:")
            return SimpleNamespace(returncode=0 if valid else 1, stdout="", stderr="invalid settings")
        if words[:2] == ["settings", "import"]:
            source = Path(words[2])
            if not source.is_file() or not source.read_bytes().startswith(b"valid:") or self.target.exists():
                return SimpleNamespace(returncode=1, stdout="", stderr="import refused")
            self.target.parent.mkdir(parents=True, exist_ok=True)
            self.target.write_bytes(source.read_bytes())
            return SimpleNamespace(returncode=0, stdout="imported", stderr="")
        if words == ["settings", "init"]:
            self.target.parent.mkdir(parents=True, exist_ok=True)
            self.target.write_bytes(b"valid:fresh")
            return SimpleNamespace(returncode=0, stdout="initialized", stderr="")
        raise AssertionError(f"Unexpected command: {command}")

    def test_imports_existing_export_to_managed_path(self):
        export = self.root / "downloaded-export.zip"
        export.write_bytes(b"valid:existing rules")
        result = prepare_settings("leko", self.repo, runner=self.runner, chooser=lambda: str(export))
        self.assertEqual(result, str(self.target))
        self.assertEqual(self.target.read_bytes(), export.read_bytes())
        self.assertEqual(self.calls[-2:], [["settings", "import", str(export)], ["settings", "validate", str(self.target)]])

    def test_initializes_fresh_only_after_explicit_choice(self):
        result = prepare_settings("leko", self.repo, runner=self.runner, chooser=lambda: "fresh")
        self.assertEqual(result, str(self.target))
        self.assertEqual(self.target.read_bytes(), b"valid:fresh")
        self.assertIn(["settings", "init"], self.calls)

    def test_migrates_valid_legacy_repo_file_without_prompt(self):
        legacy = self.repo / "settings.zip"
        legacy.write_bytes(b"valid:legacy rules")
        result = prepare_settings("leko", self.repo, runner=self.runner, chooser=lambda: self.fail("unexpected setup prompt"))
        self.assertEqual(result, str(self.target))
        self.assertEqual(self.target.read_bytes(), legacy.read_bytes())

    def test_existing_managed_settings_are_validated_and_reused(self):
        self.target.parent.mkdir(parents=True)
        self.target.write_bytes(b"valid:current")
        result = prepare_settings("leko", self.repo, runner=self.runner, chooser=lambda: self.fail("unexpected setup prompt"))
        self.assertEqual(result, str(self.target))
        self.assertEqual(self.calls, [["settings", "path"], ["settings", "validate", str(self.target)]])

    def test_explicit_override_is_validated_without_import_or_init(self):
        override = self.root / "override.zip"
        override.write_bytes(b"valid:override")
        result = prepare_settings("leko", self.repo, str(override), runner=self.runner)
        self.assertEqual(result, str(override))
        self.assertEqual(self.calls, [["settings", "validate", str(override)]])

    def test_cancelled_setup_aborts_without_creating_data(self):
        with self.assertRaises(SettingsSetupError):
            prepare_settings("leko", self.repo, runner=self.runner, chooser=lambda: "")
        self.assertFalse(self.target.exists())


if __name__ == "__main__":
    unittest.main()
