// Plan scan (PostToolUse matcher: Bash, and Stop).
//
// The plan gate only sees Write/Edit/MultiEdit. This hook closes the shell hole after the fact: it asks
// git which code files are new (untracked or added), runs the gate's checks on each (entry, registry,
// naming rules R1-R5), registers the ones that pass in .agent/features.json, and blocks the turn
// (PostToolUse) or the stop (Stop, unless stop_hook_active) while any file breaks the rules.
// The same set of violations is reported at most 3 times; then one systemMessage line replaces the block.
// Not a git repository, or git failing: one systemMessage per session, nothing checked.
// Errors are logged to $CLAUDE_STATE_DIR/plan-gate/errors.log, shown on one line, and exit 0.

import path from "node:path";
import { readInput, systemMessage } from "./policy/hookio.mjs";
import { findProjectRoot, runHook } from "./plan/core.mjs";
import { planFirst } from "./plan/active-mode.mjs";
import { MAX_REPORTS, clearReports, countReport, shouldWarnNotGit, signatureOf } from "./plan/scan/loop-guard.mjs";
import { newFiles } from "./plan/scan/untracked-files.mjs";
import { blockReason, checkNewFiles, exhaustedNotice } from "./plan/scan/scan-report.mjs";

const NOT_GIT = "plan-scan: not a git repository; files created through shell commands are not checked.";

runHook("plan-scan", () => {
  const input = readInput();
  if (!input || typeof input !== "object") {
    systemMessage("plan-scan: hook input was not valid JSON; this call was not checked.");
    return;
  }
  const event = input.hook_event_name;
  if (event !== "PostToolUse" && event !== "Stop") return;

  const cwd = typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd();
  const root = findProjectRoot(path.join(cwd, "scan"), cwd); // a probe path inside cwd: the root search starts at cwd itself
  if (!planFirst(root)) return; // not plan-first: silent, no git call
  const found = newFiles(root);
  if (!found.ok) {
    if (shouldWarnNotGit(root, input.session_id)) systemMessage(NOT_GIT);
    return;
  }

  const violations = checkNewFiles(root, found.gated, found.fresh);
  if (!violations.length) {
    clearReports(root);
    return;
  }
  if (event === "Stop" && input.stop_hook_active === true) {
    systemMessage(`plan-scan: ${violations.length} new code file(s) still break the plan rules (first: ${violations[0].rel}); the stop is not blocked a second time.`);
    return;
  }
  const times = countReport(root, signatureOf(violations));
  if (times > MAX_REPORTS) systemMessage(exhaustedNotice(violations, MAX_REPORTS));
  else process.stdout.write(JSON.stringify({ decision: "block", reason: blockReason(root, violations) }));
});
