// The plan gate as a function, for the guards of Codex, OpenCode and Antigravity (Claude Code runs plan-gate.mjs, which
// makes the same decision). Only the CREATION of a new code file is gated: the project must be plan-first (an
// .agent/plans/ACTIVE.md and no plan-off switch), and ACTIVE.md needs a valid "### <path>" entry for the file, tied to an
// open contract when ACTIVE.md was made by /contract new. Existing files, tests, docs and .agent/.claude/node_modules
// paths are never gated.
//
//   creationDenial({ tool_name: "Write" | "Edit" | "MultiEdit", tool_input: { file_path, ... }, cwd }) -> denial text | null

import path from "node:path";
import { proposedContent } from "../policy/proposed.mjs";
import { BOILERPLATE_MAX_LINES, CODE_EXT, FILE_TOOLS, classify, findProjectRoot, toPosix } from "./core.mjs";
import { evaluateCreation, formatDenial } from "./entries/evaluate.mjs";
import { planFirst } from "./active-mode.mjs";

export function creationDenial({ tool_name: tool, tool_input: ti, cwd: dir }) {
  if (!FILE_TOOLS.has(tool) || !ti || typeof ti.file_path !== "string" || !ti.file_path) return null;
  const cwd = dir || process.cwd();
  const abs = path.resolve(cwd, ti.file_path);
  if (!CODE_EXT.has(path.extname(abs).toLowerCase())) return null;

  const root = findProjectRoot(abs, cwd);
  if (!planFirst(root)) return null;

  const proposed = proposedContent(tool, ti, abs);
  if (proposed.status !== "ok" || proposed.prev !== null) return null; // the tool reports its own errors; existing files are never gated

  const rel = toPosix(path.relative(root, abs));
  if (rel === ".." || rel.startsWith("../")) return null; // outside the project root: not gated
  const kind = classify(rel, proposed.content);
  if (!kind.gated) return null;

  const verdict = evaluateCreation(root, rel);
  if (!verdict.problems.length) return null;
  const notes = kind.boilerplateLines
    ? [`Note: index/main/__init__.py/conftest.py files are exempt only up to ${BOILERPLATE_MAX_LINES} lines; this one has ${kind.boilerplateLines}.`]
    : [];
  return formatDenial(root, rel, verdict, notes);
}
