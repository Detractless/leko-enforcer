// Keeps plan-scan from nagging forever: per project root, remembers which set of violations was
// last reported and how often. State lives in $CLAUDE_STATE_DIR/plan-scan/<hash of root>.json.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../../project-root.mjs";

export const MAX_REPORTS = 3;

const stateFile = (root) => path.join(stateRoot(), "plan-scan", `${crypto.createHash("sha1").update(root).digest("hex").slice(0, 16)}.json`);

function readState(root) {
  try { return JSON.parse(fs.readFileSync(stateFile(root), "utf8")) || {}; } catch { return {}; }
}

function writeState(root, state) {
  const file = stateFile(root);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(state), "utf8");
  fs.renameSync(tmp, file);
}

/** Identity of a set of violations: the same files with the same problems give the same signature. */
export function signatureOf(violations) {
  const lines = violations.map((v) => `${v.rel}\n${v.problems.join("\n")}`).sort();
  return crypto.createHash("sha1").update(lines.join("\n\n")).digest("hex");
}

/** Records one report of `signature`; `count` is how many times this exact set has now been reported. */
export function countReport(root, signature) {
  const state = readState(root);
  const count = state.signature === signature ? (state.count || 0) + 1 : 1;
  writeState(root, { ...state, signature, count });
  return count;
}

export function clearReports(root) {
  const { signature, count, ...rest } = readState(root);
  if (signature !== undefined || count !== undefined) writeState(root, rest);
}

/** True once per root and session: the "not a git repository" notice is shown once, not on every Bash call. */
export function shouldWarnNotGit(root, sessionId) {
  const state = readState(root);
  const session = String(sessionId ?? "");
  if (state.notGitSession === session) return false;
  writeState(root, { ...state, notGitSession: session });
  return true;
}
