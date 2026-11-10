# Plan entry template

Add one entry per NEW code file to `.agent/plans/ACTIVE.md` BEFORE creating the file. The write is
blocked until the entry is valid (a file made through a shell command is checked after the fact
and blocks you until it is fixed). Existing files, tests, docs and `.agent/`, `.claude/`,
`node_modules/` paths need no entry. Keep the plan in place after the task.

## Format

```
### <path relative to the project root>
**Purpose:** <one sentence, >= 25 characters and >= 5 words>
**Feature:** <kebab-case slug of the feature this file belongs to>
**Existing code checked:** `<one grep, rg, find, git grep or git ls-files command>` <what it showed>
**Failure paths:** <behavior on empty input, missing files, errors; >= 25 characters>
**Criterion:** <AC-001: the acceptance criterion of the Feature's open contract that this file serves>
**Split justification:** <only if the Feature is already in .agent/features.json: why this is a separate responsibility>
```

Rules the hook enforces:
- The heading is exactly `### <path>`: one path, relative to the project root, no backticks, no `./`.
- Fields are `**Name:** value` or `Name: value` lines. No placeholders (n/a, TBD, ...), no copying
  of the example values below.
- `Existing code checked` holds ONE plain read-only command in backticks. The hook re-runs it from
  the project root (no shell, 10 s). No `; | & > < $` or backticks inside it, no find
  `-delete -exec -execdir -ok -okdir -fprint* -fls`. For alternatives repeat `-e PATTERN`.
  It must exit 0 or 1, so search directories that exist. If the entry says the search found
  nothing but the re-run hits code files outside `.agent/`, the write is blocked and the hits are
  shown: extend that code instead.
- `Feature` is looked up in `.agent/features.json` (slug -> files). A slug already there (also
  near-duplicates: `task-export2`, `task-export-v2`, `task-export-new` match `task-export`) blocks
  the new file unless the entry has a real `Split justification:`. Prefer extending the listed files.
- `Criterion` (projects started with /contract new) names an acceptance criterion `AC-nnn` that exists in
  `.agent/contracts/<Feature>.md`, and that contract must be open. No open contract: ask the user to run `/contract new <feature>`.
- `index.*`, `main.*`, `__init__.py`, `conftest.py` need no entry up to 40 lines.

## Placement rules

Checked for every new code file: by the gate before a Write/Edit, and after the fact for files made
through shell commands (after each Bash call and before you stop). Defaults; optional
`.agent/naming-rules.json` (`maxFilesPerFolder`, `bannedTokens`, `allowedRootFiles`) overrides them.
- R1 The name before the first `.` is lowercase kebab-case: `csv-writer.js`, `csv-writer.test.js`.
- R2 No catch-all word in the name: utils, util, helpers, helper, misc, common, stuff, temp, tmp,
  new, final, old, copy, v2, v3, lib2, general, other, things, shared, manager. Name it after what it does.
- R3 Not in the project root. Use `src/` (`bin/` for an entry script). `*.config.js|mjs|cjs` may stay.
- R4 At most 8 non-test code files directly in one folder (`index.*` not counted; `test/`, `tests/`,
  `__tests__/` uncapped). Past that, group related files into a feature subfolder.
- R5 Judged by the file's folder, never by its name (a name that contains the Feature is fine, not
  required). Directly in `src/`, `lib/`, `app/`, `bin/` or in a role folder (commands, cli, io, core,
  lib, models, services, routes, adapters, domain, ui, api, db, store, storage, bin, scripts): no match
  needed. In any other subfolder (a feature folder, e.g. `src/budgets/x.js`) the folder name must share
  a prefix of 4+ letters with a word of the Feature slug (exact match when either word is shorter than
  4 letters): `monthly-report` fits `src/reports/x.js`. Words that do not identify a feature are ignored:
  words found in half or more of the features already in `.agent/features.json` (once it holds 3+), and
  words of the project name (package.json "name" or the project folder). Else move the file into the
  folder named for its feature, or change the Feature.

## Worked example

```
### src/export/csv.js
**Purpose:** Serialize the task list to CSV text for the export command.
**Feature:** task-export
**Existing code checked:** `grep -rn -e csv -e export src` shows only src/format.js, a JSON printer that cannot emit CSV, so a new writer is needed.
**Failure paths:** An empty task list yields only the header row; a field containing a comma or quote is quoted and escaped.
**Criterion:** AC-001: the export command writes one CSV row per task (only in projects started with /contract new)
```

The example values are rejected if copied verbatim; write your own.
