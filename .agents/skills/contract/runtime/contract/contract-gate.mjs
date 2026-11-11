// The two questions the contract answers for the plan gate and for `/contract open`:
//   validateContract(text)            is the contract complete enough to build against?
//   contractProblems(root, feature, criterion)   may a new code file for this feature be written now?
// Wording names the section, the item and the fix, like the plan gate's own messages.

import fs from "node:fs";
import path from "node:path";
import { weakness } from "../plan/entries/entry.mjs";
import { CHECKLIST_DIR, coverageProblems, tagProblem } from "./checklist-coverage.mjs";
import { parseContract } from "./contract-parse.mjs";
import { contractRel, listContracts, readState } from "./contract-state.mjs";
import { parseEvidence } from "./criteria-evidence.mjs";

export const MIN_EDGE_CASES = 3;
const MAX_SHOWN = 6;
const PLACEHOLDER = /<[^<>\n]{2,}>/;

const weak = (label, value, minChars) => {
  if (PLACEHOLDER.test(value)) return `${label}: still has a <placeholder>; write the real text.`;
  const why = weakness(value, { minChars });
  return why ? `${label}: ${why}.` : null;
};

/** @returns {string[]} everything missing or weak; empty means the contract can be opened. */
export function validateContract(text, checklistDir = CHECKLIST_DIR) {
  const c = parseContract(text);
  const out = [];
  const add = (p) => { if (p) out.push(p); };
  add(weak("Goal", c.goal, 25));
  const ids = new Set();
  if (!c.criteria.length) out.push("Acceptance criteria: none found; add lines like \"- AC-001: <what a user can check>\" with evidence: and breaker: under them.");
  for (const ac of c.criteria) {
    if (ids.has(ac.id)) out.push(`${ac.id}: listed twice.`);
    ids.add(ac.id);
    add(weak(ac.id, ac.text, 15));
    if (!ac.evidence) out.push(`${ac.id}: evidence: is missing (a test file and test name, or file:line).`);
    else if (PLACEHOLDER.test(ac.evidence)) out.push(`${ac.id}: evidence: still has a <placeholder>.`);
    else { const ev = parseEvidence(ac.evidence); if (!ev.ok) out.push(`${ac.id}: evidence ${ev.why}.`); }
    if (!ac.breaker) out.push(`${ac.id}: breaker: is missing (the bug this evidence must catch).`);
    else add(weak(`${ac.id} breaker`, ac.breaker, 15));
  }
  const lists = [["Checklist coverage", c.coverage.map((r) => r.text)], ["Derived checklist", c.derived.map((r) => r.text)], ["Edge case", c.edges.map((e) => e.text)], ["Out of scope", c.outOfScope], ["Decisions", c.decisions]];
  for (const [label, rows] of lists) for (const row of rows) if (PLACEHOLDER.test(row)) out.push(`${label}: "${row.slice(0, 30)}" still has a <placeholder>.`);
  if (!c.outOfScope.length) out.push("Out of scope: name at least one thing this feature will not do.");
  out.push(...coverageProblems(c, ids, checklistDir));
  if (c.edges.length < MIN_EDGE_CASES) out.push(`Edge cases: needs at least ${MIN_EDGE_CASES} (it has ${c.edges.length}), each ending in -> AC-001 or -> out-of-scope: <reason>.`);
  for (const [i, e] of c.edges.entries()) { const why = tagProblem(e.tag, ids); if (why) out.push(`Edge case ${i + 1} (${e.text.slice(0, 30)}): ${why}.`); }
  for (const d of c.decisions) if (d.split("|").length < 3) out.push(`Decisions: "${d.slice(0, 40)}" must read "choice | why | REVERSIBLE" (or IRREVERSIBLE).`);
  return out;
}

const shown = (problems) => problems.slice(0, MAX_SHOWN).join(" ") + (problems.length > MAX_SHOWN ? ` (+${problems.length - MAX_SHOWN} more)` : "");

/** @returns {string[]} reasons a new code file of `feature` may not be written yet; empty = allowed. */
export function contractProblems(root, feature, criterionRaw) {
  const state = readState(root, feature);
  let text = null;
  try { text = fs.readFileSync(path.join(root, contractRel(feature)), "utf8"); } catch { /* handled below */ }
  if (!state || text === null) {
    const open = listContracts(root).filter((c) => c.status === "open").map((c) => c.feature);
    const hint = open.length ? ` Open contracts: ${open.join(", ")}; use one of those names as the Feature, or` : "";
    return [`Feature: "${feature}" has no contract (${contractRel(feature)}).${hint} ask the user to run /contract new ${feature}.`];
  }
  if (state.status === "draft") {
    const left = validateContract(text);
    return [`Feature: the contract for "${feature}" is still a draft and the user has not opened it. ${left.length ? `Finish it first: ${shown(left)}` : `It looks complete: ask the user to type /contract open ${feature}.`}`];
  }
  if (state.status !== "open") return [`Feature: the contract for "${feature}" is ${state.status}. Ask the user to run /contract new <feature> for new work.`];
  const have = parseContract(text).criteria.map((a) => a.id);
  const named = String(criterionRaw ?? "").match(/\bAC-\d{3,}\b/gi);
  if (!named) return [`Criterion: field is missing or names no acceptance criterion. Name the one this file serves; contract "${feature}" has ${have.join(", ") || "none"}.`];
  const unknown = named.map((n) => n.toUpperCase()).filter((n) => !have.includes(n));
  return unknown.length ? [`Criterion: ${unknown.join(", ")} is not in contract "${feature}", which has ${have.join(", ") || "none"}.`] : [];
}
