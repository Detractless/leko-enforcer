// Codex PreToolUse guard (hooks.json matcher "apply_patch|Bash"). Runs the SAME checks as Claude Code's
// policy-guard.mjs, through the same modules in ../policy/: the 350-line cap, file names and folders for new
// files, project policy, and lock records.
//
// apply_patch: each touched file's complete new content is worked out (patch-files.mjs) and checked as a Write;
//   deleted and moved-away files get the lock check only. One failing file refuses the whole patch.
//   A file the patch creates also passes the plan gate (plan/creation-denial.mjs) when the project is plan-first.
// Bash: the same best-effort text match for writes to locked files (a leading `rtk ` is ignored).
// Output is the shape Codex reads: a deny JSON, or one systemMessage line. An internal error is logged and the
// call is allowed. Off switches are the same files as for Claude Code: <state>/code-layout-off, <state>/locks-off.
// Each block or warning adds one line to <state>/code-layout.log (never file content).

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { stateRoot } from "../project-root.mjs";
import { evaluateBashCall } from "../policy/guard-bash.mjs";
import { evaluateFileCall } from "../policy/guard-file.mjs";
import { denyPreToolUse, readInput, runGuarded, systemMessage } from "../policy/hookio.mjs";
import { creationDenial } from "../plan/creation-denial.mjs";
import { planSwitchedOff } from "../plan/active-mode.mjs";
import { proposedFiles } from "./patch-files.mjs";

const REFUSED = "(apply_patch: the whole patch was refused; none of its files were changed.)";

function check(abs, cwd, mode, content) {
  const lockOnly = content === undefined;
  const input = { tool_name: "Write", tool_input: { file_path: abs, content: content ?? "" }, cwd };
  return evaluateFileCall(input, lockOnly ? { layout: false, locks: mode.locks } : mode);
}

export function evaluatePatch(input, mode) {
  const cwd = input.cwd || process.cwd();
  const text = input.tool_input && input.tool_input.command;
  if (typeof text !== "string") return { deny: null, notices: [], paths: [] };
  const notices = [];
  const paths = [];
  for (const f of proposedFiles(text, cwd)) {
    paths.push(f.abs);
    const calls = [];
    if (f.kind === "move") calls.push([f.from]);
    if (f.kind === "delete") calls.push([f.abs]);
    else if (f.skipped) {
      calls.push([f.abs]);
      notices.push(`${path.relative(cwd, f.abs) || f.abs}: size/name checks not applied - ${f.skipped} (lock check was applied)`);
    } else calls.push([f.abs, f.content]);
    for (const [abs, content] of calls) {
      const v = check(abs, cwd, mode, content);
      const planDeny = !v.deny && content !== undefined && mode.plan !== false ? creationDenial({ tool_name: "Write", tool_input: { file_path: abs, content }, cwd }) : null;
      if (v.deny || planDeny) return { deny: `${v.deny || planDeny}\n${REFUSED}`, notices: [], paths };
      notices.push(...v.notices);
    }
  }
  return { deny: null, notices, paths };
}

export const withoutRtk = (cmd) => cmd.replace(/(^|[;&|(]\s*)rtk\s+/g, "$1");

export function logLine(tool, target, kind, text, agent = "codex") {
  try {
    const dir = stateRoot();
    fs.mkdirSync(dir, { recursive: true });
    fs.appendFileSync(path.join(dir, "code-layout.log"), `${new Date().toISOString()}\t${kind}\t${agent}:${tool}\t${target}\t${text.split("\n").slice(0, 3).join(" | ").slice(0, 240)}\n`);
  } catch { /* the log must never break the guard */ }
}

/** The same three off switches as Claude Code: <state>/code-layout-off, <state>/locks-off and <state>/plan-off. */
export function currentMode() {
  return { layout: !fs.existsSync(path.join(stateRoot(), "code-layout-off")), locks: !fs.existsSync(path.join(stateRoot(), "locks-off")), plan: !planSwitchedOff() };
}

/** True when every check is switched off, so a guard can leave without reading its input. */
export const allOff = (mode) => !mode.layout && !mode.locks && !mode.plan;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runGuarded("codex-guard", () => {
    const mode = currentMode();
    if (allOff(mode)) return;
    const input = readInput();
    if (!input) return;
    let verdict = { deny: null, notices: [], paths: [] };
    if (input.tool_name === "apply_patch") verdict = evaluatePatch(input, mode);
    else if (input.tool_name === "Bash" && mode.locks && typeof input.tool_input?.command === "string") {
      verdict = evaluateBashCall({ ...input, tool_input: { ...input.tool_input, command: withoutRtk(input.tool_input.command) } });
    }
    const target = (verdict.paths || []).join(",");
    if (verdict.deny) { logLine(input.tool_name, target, "block", verdict.deny); denyPreToolUse(verdict.deny); }
    else if (verdict.notices.length) { logLine(input.tool_name, target, "warn", verdict.notices.join(" | ")); systemMessage(`policy-guard: ${verdict.notices.join(" | ")}`); }
  });
}
