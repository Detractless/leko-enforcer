// PreToolUse decision for Write / Edit / MultiEdit / NotebookEdit.
// -> { deny: string|null, notices: string[] }   (deny blocks; notices are shown, not blocking)
//
// Order: policy loads (the OUTERMOST policy up to the git root; nested ones are
// "shadow policy ignored") -> lock records (direct edits of the records file, corrupt
// records, locked target) -> complete proposed content -> engine.
// An edit of the policy file itself is allowed with a notice naming the keys it changes.

import path from "node:path";
import { fsContext } from "./scan.mjs";
import { denyForCorruptRecords, denyForFindings, denyForLock, denyForRecords, dedupeNotices, exemptionLine } from "./format.mjs";
import { inScope } from "./common.mjs";
import { preflightFile } from "./engine.mjs";
import { folderVerdict } from "./folder-check.mjs";
import { checkLineCap } from "./linecap.mjs";
import { isStrict } from "./mode.mjs";
import { nameFindings } from "./namecheck.mjs";
import { findProjectRoot as gitProjectRoot } from "../plan/core.mjs";
import { findEntry, hashState, readRecords, recordFiles } from "./locks.mjs";
import { POLICY_REL, findProjectRoot, sameFile, shadowPolicies, toRel } from "./paths.mjs";
import { loadPolicy } from "./policy.mjs";
import { describeChange, shadowNotices } from "./policyfile.mjs";
import { proposedContent } from "./proposed.mjs";
import { webFindings } from "./web/web-check.mjs";

const NONE = { deny: null, notices: [] };

function lockGate(tool, policy, rel, abs) {
  if (!policy.lockRecords) return null;
  const { root } = policy;
  const [recFile] = recordFiles(root, policy.lockRecords);
  if (recordFiles(root, policy.lockRecords).some((f) => sameFile(f, abs))) return denyForRecords(tool, rel, policy.lockRecords);
  const rec = readRecords(recFile);
  if (rec.state === "corrupt") return denyForCorruptRecords(tool, rel, policy.lockRecords, rec.problems[0]);
  const entry = rec.state === "ok" ? findEntry(root, rec.data, abs) : null;
  return entry && entry.state === "locked" ? denyForLock(tool, rel, entry, hashState(root, entry)) : null;
}

// No project policy: the built-in 350-line cap, and for a NEW file the file-name and folder rules, still
// apply to Write/Edit/MultiEdit (nothing else does). Everyday mode (no ACTIVE.md) only warns about growth
// of a file that was already over the cap, or a root or folder that already broke a folder rule; see
// linecap.mjs and folder-check.mjs.
function capOnly(tool, ti, abs, root) {
  const rel = toRel(root, abs);
  if (rel === null || tool === "NotebookEdit") return NONE;
  const prop = proposedContent(tool, ti, abs);
  if (prop.status !== "ok") return NONE;
  const strict = isStrict(root);
  const out = checkLineCap("preflight", rel, prop.content, prop.prev, strict);
  out.findings.push(...nameFindings(root, rel, prop.prev));
  out.findings.push(...webFindings(root, rel, prop.content, prop.prev));
  const folder = prop.prev === null ? folderVerdict(root, [rel], strict) : { findings: [], notices: [] };
  out.findings.push(...folder.findings);
  const notices = [...dedupeNotices(out.notices).map((n) => n.text), ...folder.notices];
  if (out.findings.length) return { deny: denyForFindings(tool, rel, out.findings, notices.map((text) => ({ kind: "note", file: text, text })), "a code-layout rule"), notices: [] };
  return { deny: null, notices };
}

function contentNotices(out) {
  return [...dedupeNotices(out.notices).map((n) => n.text), ...out.exempted.map(exemptionLine)];
}

// mode.layout false (code-layout-off): only lock checks run. mode.locks false (locks-off): lock checks are skipped.
function evaluate(input, found, mode) {
  const tool = input.tool_name;
  const ti = input.tool_input || {};
  const target = tool === "NotebookEdit" ? ti.notebook_path : ti.file_path;
  if (typeof target !== "string" || target === "") return NONE;
  const cwd = input.cwd || process.cwd();
  const abs = path.resolve(cwd, target);
  const root = findProjectRoot(abs) || findProjectRoot(cwd);
  if (!root) return mode.layout ? capOnly(tool, ti, abs, gitProjectRoot(abs, cwd)) : NONE;
  const policy = loadPolicy(root);
  if (policy.state === "absent") return mode.layout ? capOnly(tool, ti, abs, root) : NONE;
  if (policy.state !== "ok") {
    return { deny: null, notices: [`policy file ${POLICY_REL} is unusable (${policy.problems[0]}); NO policy checks or lock denies were applied to this ${tool}`] };
  }
  const rel = toRel(root, abs);
  if (rel === null) return NONE;
  found.root = root;
  found.shadows.push(...shadowPolicies(root, abs));
  if (rel.endsWith(`/${POLICY_REL}`)) found.shadows.push(rel); // creating or editing a nested policy
  const notices = policy.problems.map((p) => `policy value invalid - ${p}`);
  const locked = mode.locks ? lockGate(tool, policy, rel, abs) : null;
  if (locked) return { deny: locked, notices: [] };
  if (!mode.layout) return NONE;
  const isPolicy = sameFile(abs, policy.file);
  if (isPolicy && tool === "NotebookEdit") notices.push(`${rel} is being changed by ${tool}; its new values apply from the next call`);
  if (tool === "NotebookEdit") {
    if (inScope(policy, rel) && Object.keys(policy.cfg).length) notices.push(`${rel}: policy size/structure checks not applied - NotebookEdit content is not computed (lock check was applied)`);
    return { deny: null, notices };
  }
  const prop = proposedContent(tool, ti, abs);
  if (isPolicy) {
    const what = prop.status === "ok" ? `${describeChange(prop.prev, prop.content)}; new values apply from the next call` : "its new values apply from the next call, and an invalid result disables the checks";
    notices.push(`${rel} is being changed by ${tool}; ${what}`);
  }
  if (prop.status === "skip") return { deny: null, notices };
  const out = preflightFile(fsContext(policy), rel, prop.content, prop.prev);
  out.findings.push(...nameFindings(root, rel, prop.prev));
  out.findings.push(...webFindings(root, rel, prop.content, prop.prev));
  if (out.findings.length) {
    return { deny: denyForFindings(tool, rel, out.findings, [...policy.problems.map((text) => ({ kind: "invalid-policy", file: text, text })), ...out.notices]), notices: [] };
  }
  return { deny: null, notices: [...notices, ...contentNotices(out)] };
}

export function evaluateFileCall(input, mode = { layout: true, locks: true }) {
  const found = { root: null, shadows: [] };
  const verdict = evaluate(input, found, mode);
  if (verdict.deny || !found.shadows.length) return verdict;
  return { deny: null, notices: [...verdict.notices, ...shadowNotices(found.root, [...new Set(found.shadows)])] };
}
