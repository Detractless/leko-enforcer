// OpenCode guard (called by opencode-guard-plugin.ts before every tool call). Runs the SAME checks as Claude Code's
// policy-guard.mjs and the Codex guard, through the same modules in ../policy/ and ../codex/: the 350-line cap, file
// names and folders for new files, project policy, and lock records.
//
// stdin : { "tool": "write|edit|apply_patch|bash|...", "args": {...OpenCode tool args}, "directory": "<project dir>" }
// stdout: { "deny": "<reason>" | null }      (the plugin throws the reason, which stops the tool and tells the model)
//
//   write        -> checked as a Claude Write      (filePath, content)
//   edit         -> checked as a Claude Edit       (filePath, oldString, newString, replaceAll)
//   apply_patch  -> same patch reader as Codex     (patchText)
//   bash         -> best-effort text match for writes to locked files (a leading `rtk ` is ignored)
// A file that write or edit creates also passes the plan gate (plan/creation-denial.mjs) when the project is plan-first.
// Any other tool, bad input or an internal error is allowed. Off switches and log file are the same as for Claude Code.

import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { evaluateBashCall } from "../policy/guard-bash.mjs";
import { evaluateFileCall } from "../policy/guard-file.mjs";
import { allOff, evaluatePatch, currentMode, logLine, withoutRtk } from "../codex/codex-guard.mjs";
import { creationDenial } from "../plan/creation-denial.mjs";

const NONE = { deny: null, notices: [], paths: [] };

function checkFile(call, mode) {
  const v = evaluateFileCall(call, mode);
  const planDeny = !v.deny && mode.plan !== false ? creationDenial(call) : null;
  return { ...v, deny: v.deny || planDeny, paths: [call.tool_input.file_path] };
}

export function evaluateOpenCode({ tool, args, directory }, mode) {
  const cwd = directory || process.cwd();
  const a = args && typeof args === "object" ? args : {};
  if (tool === "write" && typeof a.filePath === "string") {
    return checkFile({ tool_name: "Write", tool_input: { file_path: a.filePath, content: String(a.content ?? "") }, cwd }, mode);
  }
  if (tool === "edit" && typeof a.filePath === "string") {
    const ti = { file_path: a.filePath, old_string: a.oldString, new_string: a.newString, replace_all: a.replaceAll === true };
    return checkFile({ tool_name: "Edit", tool_input: ti, cwd }, mode);
  }
  if (tool === "apply_patch" && typeof a.patchText === "string") {
    return evaluatePatch({ cwd, tool_input: { command: a.patchText } }, mode);
  }
  if (tool === "bash" && mode.locks && typeof a.command === "string") {
    const v = evaluateBashCall({ tool_name: "Bash", cwd, tool_input: { command: withoutRtk(a.command) } });
    return { ...v, paths: v.paths || [] };
  }
  return NONE;
}

function run() {
  const mode = currentMode();
  if (allOff(mode)) return { deny: null };
  const input = JSON.parse(fs.readFileSync(0, "utf8"));
  const v = evaluateOpenCode(input, mode);
  const target = (v.paths || []).join(",");
  if (v.deny) {
    logLine(input.tool, target, "block", v.deny, "opencode");
    return { deny: v.deny };
  }
  if (v.notices && v.notices.length) logLine(input.tool, target, "warn", v.notices.join(" | "), "opencode");
  return { deny: null };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let out = { deny: null };
  try {
    out = run();
  } catch (err) { // an internal error is logged and the call is allowed
    try { logLine("guard", "", "error", String(err && err.message), "opencode"); } catch { /* the log must never break the guard */ }
  }
  process.stdout.write(JSON.stringify(out));
}
