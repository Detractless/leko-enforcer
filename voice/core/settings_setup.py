"""Resolve, validate, and prepare the user's settings before the assistant listens."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from typing import Callable, Optional


class SettingsSetupError(RuntimeError):
    pass


def _run(cli_binary, arguments, runner=None):
    execute = runner or subprocess.run
    try:
        result = execute([str(cli_binary), *map(str, arguments)], capture_output=True, text=True)
    except OSError as exc:
        raise SettingsSetupError(f"Could not start the settings service: {exc}") from exc
    if result.returncode:
        detail = (result.stderr or result.stdout or "The settings command failed.").strip()
        raise SettingsSetupError(detail)
    return result.stdout.strip()


def _choose_first_run(runner=None):
    if sys.platform != "darwin":
        raise SettingsSetupError("First-run setup needs the macOS import dialog.")
    script = '''
set choice to button returned of (display dialog "Leko needs a local rule set. Import rules from your Leko Enforcer app, or start fresh." buttons {"Cancel", "Start fresh", "Import existing"} default button "Import existing")
if choice is "Start fresh" then return "fresh"
set selectedFile to choose file with prompt "Choose the exported rules from your Leko Enforcer app."
return POSIX path of selectedFile
'''
    execute = runner or subprocess.run
    try:
        result = execute(["/usr/bin/osascript", "-e", script], capture_output=True, text=True)
    except OSError as exc:
        raise SettingsSetupError(f"Could not open the first-run setup dialog: {exc}") from exc
    if result.returncode:
        raise SettingsSetupError("Settings setup was cancelled; the microphone was not started.")
    choice = result.stdout.strip()
    if not choice:
        raise SettingsSetupError("The setup dialog did not return a choice.")
    return choice


def prepare_settings(cli_binary, repo_root, explicit_path=None, runner=None,
                     chooser: Optional[Callable[[], str]] = None):
    """Return the validated archive path; import or initialize on first run only."""
    if explicit_path:
        selected = Path(explicit_path).expanduser()
        _run(cli_binary, ["settings", "validate", selected], runner)
        return str(selected)

    default_path = Path(_run(cli_binary, ["settings", "path"], runner)).expanduser()
    if not default_path.is_absolute():
        raise SettingsSetupError("The managed settings location is not an absolute path.")
    if default_path.exists():
        _run(cli_binary, ["settings", "validate", default_path], runner)
        return str(default_path)

    legacy_path = Path(repo_root) / "settings.zip"
    if legacy_path.is_file():
        try:
            _run(cli_binary, ["settings", "import", legacy_path], runner)
            return str(default_path)
        except SettingsSetupError:
            # A stale or damaged checkout file must not prevent a user from importing a valid export.
            if default_path.exists():
                _run(cli_binary, ["settings", "validate", default_path], runner)
                return str(default_path)

    choice = (chooser or _choose_first_run)()
    if not choice:
        raise SettingsSetupError("Settings setup was cancelled; the microphone was not started.")
    if choice == "fresh":
        _run(cli_binary, ["settings", "init"], runner)
    else:
        _run(cli_binary, ["settings", "import", choice], runner)
    _run(cli_binary, ["settings", "validate", default_path], runner)
    return str(default_path)
