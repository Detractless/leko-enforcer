# Managed settings setup

## The recurring failure

Voice previously defaulted to `REPO_ROOT/settings.zip`. When that archive was absent,
`add_block` failed before it could prepare or save a change. The spoken response hid
the CLI's missing-file message. No settings archive was bundled with this checkout.

## Current setup

Settings now live at `~/Library/Application Support/Leko/settings.zip` on macOS.
The CLI owns this path; voice asks the CLI for it, and launch-agent installation uses
the same resolver. An explicit `--settings` argument remains available for overrides.

Before the microphone starts, voice validates the managed archive. If an old
repo-root archive exists, setup imports it without deleting the original, including
its `.waiting.json` pending-change file. Otherwise, a native first-run dialog lets
the user import an existing Enforcer export or choose a fresh setup. A fresh archive
is created only after that choice. Cancelling setup leaves files unchanged and keeps
the microphone off.

Imports are validated before copying and never overwrite an existing managed
archive. The settings folder uses owner-only permissions. Root-level legacy settings
archives and their pending-change files are ignored by Git.

## Limits

An export elsewhere on disk cannot be identified safely without asking the user to
select it. The first-run dialog uses the system file picker, outside the text-free
voice window. Runtime behavior of that dialog still needs a live macOS check.
