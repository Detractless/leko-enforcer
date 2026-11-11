// Antigravity PreToolUse guard (hooks.json: { "layout-guard": { "PreToolUse": [{ "matcher": "write_file|edit_file|run_command", ... }] } }).
// Runs the SAME checks as Claude Code's policy-guard, the Codex guard and the OpenCode guard: the 350-line cap, file
// names and folders for new files, project policy, and lock records. It maps Antigravity's tool calls onto the
// OpenCode guard (which maps them onto the Claude checks):
//
//   write_file   { Path, Content }                  -> write
//   edit_file    { Path, Target, Replacement }      -> edit  (AllowMultiple / ReplaceAll = replace every match)
//   run_command  { CommandLine, Cwd? }              -> bash  (best-effort match for writes to locked files)
//
// stdin : { toolCall: { name, args }, workspacePaths: [...], ... }
// stdout: { "decision": "deny", "reason": "..." } to refuse, otherwise NOTHING. Measured in agy 1.5.1: empty output leaves
//         Antigravity's own permission flow in charge, while "{}" or {"decision":""} makes a normal file write fail.
//         Bad input, other tools or an internal error print nothing and let the call go ahead.
// Same off switches (<state>/code-layout-off, locks-off) and log file as for Claude Code.

import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { allOff, currentMode, logLine } from "../codex/codex-guard.mjs";
import { evaluateOpenCode } from "../opencode/opencode-guard.mjs";

export function toOpenCodeCall(input) {
  const call = input && input.toolCall;
  const a = call && call.args && typeof call.args === "object" ? call.args : {};
  const directory = (input.workspacePaths && input.workspacePaths[0]) || process.cwd();
  if (call && call.name === "write_file") return { tool: "write", args: { filePath: a.Path, content: a.Content }, directory };
  if (call && call.name === "edit_file") {
    return { tool: "edit", args: { filePath: a.Path, oldString: a.Target, newString: a.Replacement, replaceAll: a.AllowMultiple === true || a.ReplaceAll === true }, directory };
  }
  if (call && call.name === "run_command") return { tool: "bash", args: { command: a.CommandLine }, directory: a.Cwd || directory };
  return null;
}

function run() {
  const mode = currentMode();
  if (allOff(mode)) return null;
  const call = toOpenCodeCall(JSON.parse(fs.readFileSync(0, "utf8")));
  if (!call) return null;
  const v = evaluateOpenCode(call, mode);
  const target = (v.paths || []).join(",");
  if (v.deny) {
    logLine(call.tool, target, "block", v.deny, "antigravity");
    return { decision: "deny", reason: v.deny };
  }
  if (v.notices && v.notices.length) logLine(call.tool, target, "warn", v.notices.join(" | "), "antigravity");
  return null;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let out = null;
  try {
    out = run();
  } catch (err) { // an internal error is logged and the call is allowed
    try { logLine("guard", "", "error", String(err && err.message), "antigravity"); } catch { /* the log must never break the guard */ }
  }
  if (out) process.stdout.write(JSON.stringify(out));
}
