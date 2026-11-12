// Hook plumbing: read the JSON on stdin, print a deny or a one-line notice,
// and keep a hook from ever crashing the session (internal errors are logged
// to $CLAUDE_STATE_DIR/policy/errors.log and the hook exits 0).

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../project-root.mjs";

export function readInput() {
  try {
    return JSON.parse(fs.readFileSync(0, "utf8").replace(/^\uFEFF/, ""));
  } catch {
    return null;
  }
}

export function denyPreToolUse(reason) {
  process.stdout.write(JSON.stringify({
    hookSpecificOutput: { hookEventName: "PreToolUse", permissionDecision: "deny", permissionDecisionReason: reason },
  }));
}

/** One visible line. Newlines are folded so it stays a single line. */
export function systemMessage(text, max = 700) {
  const line = text.replace(/\s*\n\s*/g, " | ");
  process.stdout.write(JSON.stringify({ systemMessage: line.length > max ? `${line.slice(0, max - 1)}...` : line }));
}

export function logError(tag, err) {
  try {
    const dir = path.join(stateRoot(), "policy");
    fs.mkdirSync(dir, { recursive: true });
    fs.appendFileSync(path.join(dir, "errors.log"), `${new Date().toISOString()}\t${tag}\t${(err && err.stack) || err}\n`);
  } catch { /* logging must not fail the hook */ }
}

/** Run `fn`; any exception is logged and swallowed. The exit code is always 0 (set, not forced, so stdout is flushed). */
export function runGuarded(tag, fn) {
  try {
    fn();
  } catch (err) {
    logError(tag, err);
  }
  process.exitCode = 0;
}
