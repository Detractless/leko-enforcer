// What /contract verify, status and --judge print. Part 1 of verify is code only: every acceptance criterion's
// evidence must resolve (and, with a test command, the suite must pass and the test name must be in its output).
// It proves the evidence exists and ran; it does not prove the test asserts the right thing (that is the judge's job).

import fs from "node:fs";
import path from "node:path";
import { contractsRequired, planFirst } from "../plan/active-mode.mjs";
import { PLAN_REL } from "../plan/core.mjs";
import { parseContract } from "./contract-parse.mjs";
import { contractRel, listContracts, unfinished } from "./contract-state.mjs";
import { resolveEvidence, runTests } from "./criteria-evidence.mjs";

/** @returns {{feature:string, total:number, covered:number, clean:boolean, rows:object[], suite:object|null}} */
export function verifyContract(root, feature, { run } = {}) {
  let text = "";
  try { text = fs.readFileSync(path.join(root, contractRel(feature)), "utf8"); } catch { return { feature, total: 0, covered: 0, clean: false, rows: [], suite: null, missing: true }; }
  const criteria = parseContract(text).criteria;
  const suite = run ? runTests(root, run) : null;
  const rows = criteria.map((ac) => {
    const found = resolveEvidence(root, ac.evidence);
    if (!found.ok) return { id: ac.id, ok: false, why: found.why };
    if (!suite) return { id: ac.id, ok: true };
    if (!suite.ran) return { id: ac.id, ok: false, why: `the test command did not run (${suite.why})` };
    if (!suite.passed) return { id: ac.id, ok: false, why: "the test command failed, so nothing is proven" };
    if (found.ev.name && !suite.output.includes(found.ev.name)) return { id: ac.id, ok: false, why: `"${found.ev.name}" is in the file but not in the test output (the test did not run)` };
    return { id: ac.id, ok: true };
  });
  const covered = rows.filter((r) => r.ok).length;
  return { feature, total: rows.length, covered, clean: rows.length > 0 && covered === rows.length, rows, suite };
}

export function formatVerify(v) {
  if (v.missing) return `no contract for "${v.feature}" (${contractRel(v.feature)}).`;
  const how = v.suite ? (v.suite.ran && v.suite.passed ? " (test command passed)" : " (test command did NOT pass)") : " (files only; pass --run \"<test command>\" to also run the tests)";
  const lines = [`${v.covered} of ${v.total} criteria have evidence${how}`];
  for (const r of v.rows) lines.push(`  ${r.ok ? "ok     " : "MISSING"} ${r.id}${r.ok ? "" : `: ${r.why}`}`);
  return lines.join("\n");
}

/** The /contract status text: plan-first state, every contract, and a stale ACTIVE.md. */
export function formatStatus(root) {
  const lines = [];
  const planText = (() => { try { return fs.readFileSync(path.join(root, PLAN_REL), "utf8"); } catch { return null; } })();
  const on = planFirst(root);
  const why = planText === null ? "no ACTIVE.md" : !on ? "the plan-off switch is on" : contractsRequired(planText) ? "contracts required" : "older plan, no contracts required";
  lines.push(`Plan-first: ${on ? "on" : "off"} (${why})`);
  const all = listContracts(root);
  if (!all.length) lines.push("Contracts: none");
  for (const c of all) {
    const v = c.status === "open" || c.status === "draft" ? verifyContract(root, c.feature) : null;
    lines.push(`  ${c.feature} [${c.status}]${v && v.total ? ` ${v.covered} of ${v.total} criteria have evidence` : ""}`);
  }
  if (planText !== null && !unfinished(root).length) lines.push("Stale: ACTIVE.md exists but no contract is open; /contract drop (no name) archives it.");
  return lines.join("\n");
}

const excerpt = (root, ev) => {
  try {
    const rows = fs.readFileSync(path.join(root, ev.file), "utf8").split(/\r?\n/);
    const at = ev.line ? ev.line - 1 : rows.findIndex((l) => l.includes(ev.name));
    return rows.slice(Math.max(0, at), Math.max(0, at) + 12).join("\n");
  } catch { return "(unreadable)"; }
};

/** Everything a fresh judge needs, and the two questions. The judge is an AI and can be wrong. */
export function formatDigest(root, feature) {
  let text;
  try { text = fs.readFileSync(path.join(root, contractRel(feature)), "utf8"); } catch { return `no contract for "${feature}".`; }
  const c = parseContract(text);
  const out = [`Contract "${feature}". Goal: ${c.goal}`, ""];
  for (const ac of c.criteria) {
    const found = resolveEvidence(root, ac.evidence);
    out.push(`${ac.id}: ${ac.text}`, `  evidence: ${ac.evidence}`, `  breaker: ${ac.breaker}`, found.ok ? `  evidence code:\n${excerpt(root, found.ev).replace(/^/gm, "    ")}` : `  evidence does not resolve: ${found.why}`, "");
  }
  out.push(
    "Judge, in this order:",
    "1. For each criterion above whose evidence resolves: what would break in real use even though this evidence passes? Name the concrete case, or say none.",
    "2. Once for the whole contract: what would a user expect from this feature that no criterion covers?",
  );
  return out.join("\n");
}
