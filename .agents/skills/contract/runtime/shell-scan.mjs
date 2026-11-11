// Shell-command check (matcher: Bash; PreToolUse and PostToolUse).
//
// The file guard (policy-guard.mjs) only sees Write/Edit/MultiEdit. A Python script, sed, a redirect or
// a build tool can still create or grow a code file. Before the command this hook leaves a start stamp;
// after it, it asks git which code files the command touched and judges them with the same rules as the
// file guard (350-line cap, file names of NEW files). The files are already written, so a rule break is
// reported back to the model (decision: block) and an over-cap notice is a visible one-liner.
// Git projects only: no repository, or git failing, means nothing is checked (and nothing is printed).
// Off switch: <state>/code-layout-off, shared with policy-guard. Each report adds one line to
// <state>/code-layout.log (time, block|warn, Bash, first file, first words; never file content).

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "./project-root.mjs";
import { findProjectRoot } from "./plan/core.mjs";
import { readInput, runGuarded, systemMessage } from "./policy/hookio.mjs";
import { changedCodeFiles } from "./policy/shell-changes.mjs";
import { saveStamp, takeStamp } from "./policy/shell-stamp.mjs";
import { judgeChanges, problemReport } from "./policy/shell-verdict.mjs";

function logLine(kind, target, text) {
  try {
    fs.mkdirSync(stateRoot(), { recursive: true });
    fs.appendFileSync(path.join(stateRoot(), "code-layout.log"), `${new Date().toISOString()}\t${kind}\tBash\t${target}\t${text.split("\n").slice(0, 3).join(" | ").slice(0, 240)}\n`);
  } catch { /* the log must never break the hook */ }
}

runGuarded("shell-scan", () => {
  if (fs.existsSync(path.join(stateRoot(), "code-layout-off"))) return;
  const input = readInput();
  if (!input || input.tool_name !== "Bash") return;
  if (input.hook_event_name === "PreToolUse") return saveStamp(input);
  if (input.hook_event_name !== "PostToolUse") return;

  const cwd = typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd();
  const root = findProjectRoot(path.join(cwd, "scan"), cwd); // a probe path inside cwd: the root search starts at cwd itself
  const files = changedCodeFiles(root, takeStamp(input));
  if (!files || !files.length) return;
  const { problems, notices } = judgeChanges(root, files);
  if (problems.length) {
    const reason = problemReport(problems, notices);
    logLine("block", path.join(root, problems[0].rel), reason.split("\n").slice(1).join("\n"));
    process.stdout.write(JSON.stringify({ decision: "block", reason }));
  } else if (notices.length) {
    logLine("warn", path.join(root, files[0].rel), notices.join(" | "));
    systemMessage(`shell-scan: ${notices.join(" | ")}`);
  }
});
