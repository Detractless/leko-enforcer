# Known issue: voice block requests cannot find settings.zip

**Status:** Recurring and unresolved as of 2026-11-09.

## What happens

The voice assistant can produce a block draft, then says “Sorry, that request could not be prepared.” In the captured failure, the draft was a whole-computer lock from 00:00 to 07:00 every day. The CLI returned exit code 1 with:

```text
The settings file "/Users/calibro/Projects/leko-enforcer/settings.zip" was not found. Nothing was saved.
```

No block or settings change was saved.

## Cause

The voice runtime defaults to `REPO_ROOT/settings.zip` (`voice/session/runtime.py`). The repository-root archive was absent when checked on 2026-11-09. The voice query flow converts a nonzero CLI result into the generic spoken apology (`voice/session/query_flow.py`), which hides the specific file error unless the CLI log is reviewed.

## Current state and next step

The same failure will recur for requests that need this missing settings archive. No archive was created or changed while recording this note. To resolve it, configure the assistant to use a valid settings archive or initialize one safely; then retry the action and confirm the CLI reports success.
