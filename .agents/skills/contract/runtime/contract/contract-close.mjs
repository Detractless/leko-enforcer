// The three ways a contract changes state: open (freeze it), done (evidence proven), drop (give it up).
// ACTIVE.md is archived (renamed, never deleted) when the last unfinished contract closes.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { lockCmd } from "../policy/lockcmds.mjs";
import { findProjectRoot as policyRoot } from "../policy/paths.mjs";
import { loadPolicy } from "../policy/policy.mjs";
import { PLAN_REL } from "../plan/core.mjs";
import { validateContract } from "./contract-gate.mjs";
import { contractRel, readState, today, unfinished, archivePlan, writeState } from "./contract-state.mjs";
import { formatVerify, verifyContract } from "./verify-report.mjs";

const LOCKCTL = path.join(import.meta.dirname, "..", "lockctl.mjs");
const MIN_REASON_CHARS = 8;
const refuse = (...err) => ({ code: 1, out: [], err });
const ok = (...out) => ({ code: 0, out, err: [] });

function current(root, feature, allowed) {
  const state = readState(root, feature);
  if (!state) return { bad: refuse(`no contract named "${feature}" (looked for ${contractRel(feature)}).`) };
  if (!allowed.includes(state.status)) return { bad: refuse(`the contract "${feature}" is ${state.status}; this command needs it to be ${allowed.join(" or ")}.`) };
  return { state };
}

/** Archives ACTIVE.md when no draft or open contract is left. @returns {string[]} lines to print. */
function archiveIfLast(root) {
  if (unfinished(root).length) return [`Other contracts are still open, so ${PLAN_REL} stays.`];
  const name = archivePlan(root);
  return name ? [`That was the last unfinished contract: ${PLAN_REL} was renamed to .agent/plans/${name} (plan-first is off; .agent/features.json is kept).`] : [];
}

function freeze(root, feature, authorizedBy) {
  let top = policyRoot(root);
  if (!top) {
    const init = spawnSync(process.execPath, [LOCKCTL, "init", "--root", root], { encoding: "utf8" });
    if (init.status !== 0) return `could not make the project policy file needed for locking: ${(init.stderr || init.stdout).trim()}`;
    top = root;
  }
  const policy = loadPolicy(top);
  if (policy.state !== "ok" || !policy.lockRecords) return `the project policy is unusable (${policy.problems?.[0] || policy.state}); the contract cannot be locked.`;
  const ctx = { root: top, cwd: root, recordsRel: policy.lockRecords, recordsFile: path.join(top, policy.lockRecords) };
  const r = lockCmd(ctx, path.join(root, contractRel(feature)), { reason: `contract "${feature}" opened by the user: ${authorizedBy}`, evidence: "validated: goal, criteria with evidence and breaker, checklist coverage, edge cases" });
  const text = r.err.join(" ");
  return r.code === 0 || /already has a lock entry/.test(text) ? null : `locking the contract failed: ${text}`;
}

export function openContract(root, feature, authorizedBy) {
  const { state, bad } = current(root, feature, ["draft"]);
  if (bad) return bad;
  if (typeof authorizedBy !== "string" || !authorizedBy.trim()) return refuse("missing --authorized-by \"<the user's words>\": only the user opens a contract.");
  const problems = validateContract(fs.readFileSync(path.join(root, contractRel(feature)), "utf8"));
  if (problems.length) return refuse(`The contract is not complete (${problems.length} problem${problems.length === 1 ? "" : "s"}); nothing was opened:`, ...problems.map((p) => `- ${p}`));
  const failed = freeze(root, feature, authorizedBy.trim());
  if (failed) return refuse(failed, "The contract stays a draft.");
  writeState(root, feature, { ...state, status: "open", opened: today(), authorizedBy: authorizedBy.trim() });
  return ok(`Opened "${feature}" and locked ${contractRel(feature)}. New code files for this feature now need an entry whose Criterion names one of its criteria.`);
}

export function doneContract(root, feature, run) {
  const { bad } = current(root, feature, ["open"]);
  if (bad) return bad;
  const v = verifyContract(root, feature, { run });
  if (!v.clean) return refuse("Not done: the evidence is not all there.", formatVerify(v));
  writeState(root, feature, { status: "done", done: today() });
  return ok(`"${feature}" is done: ${v.covered} of ${v.total} criteria have evidence.`, ...archiveIfLast(root));
}

export function dropContract(root, feature, reason) {
  if (typeof reason !== "string" || reason.trim().length < MIN_REASON_CHARS) return refuse(`missing --reason "<why>" (at least ${MIN_REASON_CHARS} characters); it is recorded.`);
  if (!feature) {
    if (unfinished(root).length) return refuse("Contracts are still open, so ACTIVE.md is not stale. Name the contract to drop.");
    const name = archivePlan(root);
    return name ? ok(`Archived the stale plan: ${PLAN_REL} is now .agent/plans/${name} (${reason.trim()}).`) : refuse(`There is no ${PLAN_REL} to archive.`);
  }
  const { bad } = current(root, feature, ["draft", "open"]);
  if (bad) return bad;
  writeState(root, feature, { status: "dropped", dropped: today(), reason: reason.trim() });
  return ok(`Dropped "${feature}" (${reason.trim()}). The contract file is kept.`, ...archiveIfLast(root));
}
