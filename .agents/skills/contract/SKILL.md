---
name: contract
description: Writes, opens, checks and closes the contract for one new feature: what it must do, how each point is proven, and what is out of scope. While a contract is open, new code files need a plan entry tied to it. Runs only when the user asks for it by name.
disable-model-invocation: true
argument-hint: "new <feature> | open <feature> | verify <feature> [--judge] | done <feature> | drop [<feature>] | status"
---

# /contract

One contract per new feature, written before the code. While one is open the project is plan-first: a new code file needs a plan entry in `.agent/plans/ACTIVE.md` with a `Criterion:` line naming a criterion of that contract. Editing existing files is never gated. There is no on/off; the last contract closing ends plan-first. Arguments: the words the user typed after the skill name.

Use only this tool. Never edit `.agent/contracts/*.state.json`, never lock or unlock a contract by hand, never work around a refusal:
`node .agents/skills/contract/runtime/contractctl.mjs <command> --root <project folder>`
Project folder = the git folder of the current directory (the current directory if there is no git).

This project includes the runner and its required dependencies in
.agents/skills/contract/runtime/. Use hooks.example.json to install the
Claude Code hooks and README.md for the Codex, OpenCode, and Antigravity
adapters. The hooks and adapters call the bundled runner; no copy from
~/.claude is needed after installation.

## new <feature>
1. Run `new <feature>`. Say exactly what it printed it made. Report a refusal as it came (home folder, bad name, name used).
2. Read `.agent/contracts/<feature>.md` and the checklists named in its Domains line (`checklists/`). Draft every part with real text, no `<...>` left:
   - Goal in one or two sentences, and a Domains line.
   - Acceptance criteria `AC-001..`, each one thing a user can check, with `evidence:` (`path "exact test name"` or `path:line`, project-relative) and `breaker:` (the bug the evidence must catch).
   - Every checklist ID tagged `-> criterion:AC-xxx` or `-> out-of-scope: <reason>`. For `Domains: none` write 5 to 10 derived lines.
   - At least 3 edge cases (empty input, wrong input, the thing that fails halfway), at least 1 out-of-scope item, and decisions as `choice | why | REVERSIBLE`.
3. Put the choices only the user can make on ONE decision page: a short list, one recommended answer each. Do not start code.

## open <feature>
Only when the user typed this command. It locks the contract; the words after it are the user's go-ahead.
Run `open <feature> --authorized-by "<the user's words from this message>"`. Never type it yourself, never invent the words. If it lists problems, fix the draft with the user and stop; do not retry in a loop.

## verify <feature> [--judge]
Part 1, code only: `verify <feature>` (add `--run "<the project's test command>"` when the user gave one or the project has an obvious one). Print the output as it came ("N of M criteria have evidence") and name each missing one.
Part 2, only with `--judge`: run `digest <feature>` and give its output to a fresh helper agent (if this agent cannot start one, say the judge was not run), with its two questions and nothing else from this chat. Show the answer unchanged and say it is an AI and can be wrong. Do not start a judge without `--judge`.

## done <feature>
Only when the user typed it. Run `done <feature>` (same `--run` rule). It refuses until part 1 is clean; show what is missing. When it is the last open contract, ACTIVE.md is renamed to `ARCHIVE-<date>.md`. Nothing is deleted.

## drop [<feature>]
Only when the user typed it, with a reason in their words: `drop <feature> --reason "<reason>"`. With no name it archives an old ACTIVE.md that has no open contract.

## status
Run `status` and give the output as printed: plan-first on or off, each contract with "N of M criteria have evidence", and any stale ACTIVE.md.

## A refusal from the plan gate while a contract is open
The new-file gate runs in Claude Code, Codex, OpenCode and Antigravity. The feature registry update and the end-of-turn plan check run only in Claude Code; `status` and `verify` work everywhere.
A denied file write says what is missing: an entry, a contract, a draft still unfinished, or a `Criterion:` line. Fix that (add the entry, ask the user to type `/contract new` or `/contract open`). If no contract fits the work, say so and ask; do not stretch an unrelated criterion to get through.
