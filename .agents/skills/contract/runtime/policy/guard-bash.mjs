// PreToolUse decision for Bash: BEST EFFORT. Denies a command whose text
// clearly writes to a locked file or to the lock records (see shellwrites.mjs
// for exactly which forms are recognised and which are not). Never claims
// more: a write target given by variable/glob/substitution is reported as
// "could not check", and everything else a shell can do is simply not seen.
// The Stop audit catches changes to locked files that this check missed.

import fs from "node:fs";
import path from "node:path";
import { lockctlCmd } from "./format.mjs";
import { bulkDenyText, bulkHit } from "./git-bulk.mjs";
import { readRecords, recordFiles } from "./locks.mjs";
import { POLICY_REL, findProjectRoot, realish, sameFile } from "./paths.mjs";
import { loadPolicy } from "./policy.mjs";
import { findShellWrites } from "./shellwrites.mjs";

const NONE = { deny: null, notices: [] };
const COVERAGE = "coverage: Bash detection is best effort text matching (redirection, tee, sed -i, perl -i, cp/mv/rm, dd of=, git checkout/restore/clean/reset --hard/stash); python/node scripts, variables, globs and other routes are not seen";
const POLICY_HIT = { kind: "policy", rel: POLICY_REL };

function covers(target, protectedAbs) {
  if (sameFile(target.abs, protectedAbs)) return true;
  if (!target.dirCovers) return false;
  try {
    if (!fs.statSync(target.abs).isDirectory()) return false;
  } catch { return false; }
  const dir = realish(target.abs);
  return realish(protectedAbs).startsWith(dir + path.sep);
}

function denyText(hit, target) {
  if (hit.kind === "policy") {
    return [
      "policy-guard BLOCKED Bash: the command removes, moves or rewrites the project policy file (best-effort text match). Nothing was run.",
      `file: ${hit.rel}`,
      `route: ${target.via} (${target.word})`,
      "reason: the policy decides which checks and locks apply; deleting or rewriting it by shell would switch them off without the user's say",
      "request: ask the user to change the project policy (an Edit or Write of the file by itself is allowed and shows what changed); do not weaken it to make a check pass",
      COVERAGE,
    ].join("\n");
  }
  if (hit.kind === "records") {
    return [
      "policy-guard BLOCKED Bash: the command writes to the lock records (best-effort text match). Nothing was run.",
      `file: ${hit.rel}`,
      `route: ${target.via} (${target.word})`,
      `use: ${lockctlCmd} lock|unlock|relock|status|recover`,
      COVERAGE,
    ].join("\n");
  }
  return [
    "policy-guard BLOCKED Bash: the command writes to a locked file (best-effort text match). Nothing was run.",
    `file: ${hit.rel}`,
    `route: ${target.via} (${target.word})`,
    `reason: ${hit.entry.reason || "(none recorded)"}`,
    `request: ask the user to authorize an unlock of ${hit.rel} for a stated purpose, then run: ${lockctlCmd} unlock ${hit.rel} --authorized-by "<the user's words>" --purpose "<purpose>"`,
    COVERAGE,
  ].join("\n");
}

export function evaluateBashCall(input) {
  const command = input.tool_input && input.tool_input.command;
  if (typeof command !== "string" || command === "") return NONE;
  const cwd = input.cwd || process.cwd();
  const root = findProjectRoot(cwd);
  if (!root) return NONE;
  const writes = findShellWrites(command, cwd);
  if (!writes.targets.length && !writes.unresolved.length && !writes.bulk.length) return NONE;
  const policyFile = path.join(root, POLICY_REL);
  const onPolicy = writes.targets.find((t) => covers(t, policyFile));
  if (onPolicy) return { deny: denyText(POLICY_HIT, onPolicy), notices: [] }; // whatever the policy's own state
  const policy = loadPolicy(root);
  if (policy.state !== "ok") {
    return { deny: null, notices: [`policy file ${POLICY_REL} is unusable (${policy.problems[0]}); the Bash lock check was NOT applied`] };
  }
  if (!policy.lockRecords) return NONE;
  const [recFile, bakFile] = recordFiles(root, policy.lockRecords);
  const rec = readRecords(recFile);
  const protectedList = [recFile, bakFile].map((f) => ({ kind: "records", abs: f, rel: path.relative(root, f).split(path.sep).join("/") }));
  const locked = rec.state === "ok" ? rec.data.locks.filter((e) => e.state === "locked") : [];
  for (const entry of locked) protectedList.push({ kind: "lock", abs: path.join(root, entry.path), rel: entry.path, entry });
  for (const target of writes.targets) {
    const hit = protectedList.find((p) => covers(target, p.abs));
    if (hit) return { deny: denyText(hit, target), notices: [] };
  }
  const notices = [];
  if (writes.bulk.length && locked.length) {
    const { hit, failed } = bulkHit(writes.bulk, locked.map((e) => ({ rel: e.path, abs: path.join(root, e.path), entry: e })));
    if (hit) return { deny: bulkDenyText(hit, lockctlCmd, COVERAGE), notices: [] };
    if (failed) notices.push(failed);
  }
  if (rec.state === "corrupt") notices.push(`lock records are corrupt (${rec.problems[0]}); the Bash lock check covers only the records file itself. Run: ${lockctlCmd} recover`);
  else if (writes.unresolved.length && locked.length) {
    notices.push(`Bash write target ${writes.unresolved.map((u) => `"${u.word}"`).join(", ")} uses a variable, glob or substitution; it could not be checked against ${locked.length} locked file(s) and the policy file`);
  }
  return { deny: null, notices };
}
