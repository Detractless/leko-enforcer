// PreToolUse plan gate (matcher: Write|Edit|MultiEdit).
//
// Gates only the CREATION of a new code file (Write to a missing path; Edit/MultiEdit whose empty
// old_string builds a missing file). The project's .agent/plans/ACTIVE.md must hold a valid
// "### <relative path>" entry for it (see plan/entries/evaluate.mjs), and its Feature slug must not
// already be in .agent/features.json unless the entry carries a Split justification.
// Existing files, tests, docs, and .agent/.claude/node_modules paths are never gated.
// Denies with permissionDecision "deny". Unreadable stdin and unexpected errors are logged,
// shown as one systemMessage line, and let the call through (exit 0).

import path from "node:path";
import { proposedContent } from "./policy/proposed.mjs";
import { denyPreToolUse, readInput, systemMessage } from "./policy/hookio.mjs";
import { CODE_EXT, FILE_TOOLS, BOILERPLATE_MAX_LINES, classify, findProjectRoot, runHook, toPosix } from "./plan/core.mjs";
import { evaluateCreation, formatDenial } from "./plan/entries/evaluate.mjs";
import { planFirst } from "./plan/active-mode.mjs";

runHook("plan-gate", () => {
  const input = readInput();
  if (!input || typeof input !== "object") {
    systemMessage("plan-gate: hook input was not valid JSON; this call was not checked.");
    return;
  }
  const ti = input.tool_input;
  if (!FILE_TOOLS.has(input.tool_name) || !ti || typeof ti.file_path !== "string" || !ti.file_path) return;

  const cwd = typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd();
  const abs = path.resolve(cwd, ti.file_path);
  if (!CODE_EXT.has(path.extname(abs).toLowerCase())) return;

  const root = findProjectRoot(abs, cwd);
  if (!planFirst(root)) return; // not plan-first (no ACTIVE.md, or plan-off): silent

  const proposed = proposedContent(input.tool_name, ti, abs);
  if (proposed.status !== "ok" || proposed.prev !== null) return; // the tool reports its own errors; existing files are never gated

  const rel = toPosix(path.relative(root, abs));
  if (rel === ".." || rel.startsWith("../")) {
    systemMessage(`plan-gate: ${abs} is outside the project root ${root}; not gated.`);
    return;
  }
  const kind = classify(rel, proposed.content);
  if (!kind.gated) return;

  const verdict = evaluateCreation(root, rel);
  if (!verdict.problems.length) return;
  const notes = kind.boilerplateLines
    ? [`Note: index/main/__init__.py/conftest.py files are exempt only up to ${BOILERPLATE_MAX_LINES} lines; this one has ${kind.boilerplateLines}.`]
    : [];
  denyPreToolUse(formatDenial(root, rel, verdict, notes));
});
