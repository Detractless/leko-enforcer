// The generic checklists (skills/contract/checklists/<domain>.md) and the check that every item on them is
// answered in the contract: tagged to an acceptance criterion, or set aside with a reason.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const CHECKLIST_DIR = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "skills", "contract", "checklists");
export const KNOWN_DOMAINS = ["ui", "web", "cli", "data"];
export const DERIVED_MIN = 5;
export const DERIVED_MAX = 10;
const MIN_REASON_CHARS = 8;

/** @returns {{id:string, text:string}[]} the items of one domain's checklist; empty when the file is missing. */
export function loadChecklist(domain, dir = CHECKLIST_DIR) {
  let text = "";
  try { text = fs.readFileSync(path.join(dir, `${domain}.md`), "utf8"); } catch { return []; }
  return text.split(/\r?\n/).map((l) => l.match(/^\s*-\s+([A-Z]+-\d+):\s*(.+)$/)).filter(Boolean).map((m) => ({ id: m[1], text: m[2].trim() }));
}

/** @returns {string|null} what is wrong with one tag, or null. */
export function tagProblem(tag, acIds) {
  if (!tag) return "has no tag (end the line with -> criterion:AC-001 or -> out-of-scope: <reason>)";
  if (tag.ac) return acIds.has(tag.ac) ? null : `points at ${tag.ac}, which is not an acceptance criterion in this contract`;
  return tag.outOfScope.length >= MIN_REASON_CHARS ? null : `out-of-scope needs a real reason (at least ${MIN_REASON_CHARS} characters)`;
}

/** @returns {string[]} problems with the Domains line, the checklist coverage and, for "none", the derived list. */
export function coverageProblems(parsed, acIds, dir = CHECKLIST_DIR) {
  const out = [];
  const { domains } = parsed;
  if (/[<>]/.test(parsed.domainsRaw ?? "")) return ["Domains: still has a <placeholder>; write ui, web, cli, data, or none."];
  if (!domains.length) return ["Domains: line is missing (write Domains: ui, web, cli, data, or none)."];
  const bad = domains.filter((d) => d !== "none" && !KNOWN_DOMAINS.includes(d));
  if (bad.length) out.push(`Domains: unknown domain ${bad.join(", ")} (known: ${KNOWN_DOMAINS.join(", ")}, none).`);
  if (domains.includes("none")) {
    if (domains.length > 1) out.push('Domains: "none" cannot be combined with other domains.');
    const list = parsed.derived;
    if (list.length < DERIVED_MIN || list.length > DERIVED_MAX) out.push(`Derived checklist: needs ${DERIVED_MIN}-${DERIVED_MAX} lines of checks you derive for this feature (it has ${list.length}), each as "D-1: <check> -> criterion:AC-001".`);
    for (const d of list) { const why = tagProblem(d.tag, acIds); if (why) out.push(`Derived checklist ${d.id ?? d.text.slice(0, 20)}: ${why}.`); }
    return out;
  }
  const wanted = domains.filter((d) => KNOWN_DOMAINS.includes(d)).flatMap((d) => loadChecklist(d, dir));
  const known = new Set(wanted.map((i) => i.id));
  const seen = new Map(parsed.coverage.filter((c) => c.id).map((c) => [c.id, c]));
  for (const item of wanted) {
    const row = seen.get(item.id);
    if (!row) { out.push(`Checklist coverage: ${item.id} (${item.text}) is not answered.`); continue; }
    const why = tagProblem(row.tag, acIds);
    if (why) out.push(`Checklist coverage ${item.id}: ${why}.`);
  }
  for (const id of seen.keys()) if (!known.has(id)) out.push(`Checklist coverage: ${id} is not on the checklists of the domains you chose.`);
  return out;
}
