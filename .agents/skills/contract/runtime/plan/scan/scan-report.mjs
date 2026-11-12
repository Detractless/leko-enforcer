// Judges files that appeared without passing through the Write/Edit gate (created by shell
// commands): the same decision as the gate, then registers the ones that pass, and words the block.

import path from "node:path";
import { PLAN_REL, logError } from "../core.mjs";
import { evaluateCreation } from "../entries/evaluate.mjs";
import { fixEntry } from "./fix-block.mjs";
import { addToRegistry, readRegistry } from "../entries/registry.mjs";

const MAX_FILES_LISTED = 10;

/** @returns {{rel:string, problems:string[], example:boolean}[]} files that break the rules; passing files are added to the registry. */
export function checkNewFiles(root, files, fresh) {
  const violations = [];
  const reg = readRegistry(root);
  const judged = new Set(reg.status === "ok" ? Object.values(reg.data).flat() : []); // registered = judged when it was created
  for (const rel of files) {
    const verdict = evaluateCreation(root, rel, { ignoreHits: fresh, skipSearch: judged.has(rel) });
    if (verdict.problems.length) {
      violations.push({ rel, problems: verdict.problems, example: verdict.example, contract: verdict.contract });
      continue;
    }
    const out = addToRegistry(root, verdict.slug, rel);
    if (out.result === "corrupt") logError("plan-scan", `registry not updated for ${rel}: ${out.why}`);
  }
  return violations;
}

const indent = (text) => text.split("\n").join("\n    ");

/** The text handed back to the model. The files are already on disk, so the fix is an entry, a rename or a move. */
export function blockReason(root, violations) {
  const shown = violations.slice(0, MAX_FILES_LISTED);
  const lines = [
    `plan-scan: ${violations.length} new code file(s) found on disk (created by a shell command, not Write/Edit) break the plan rules.`,
    ...shown.flatMap((v) => [`- ${v.rel}`, ...v.problems.map((p) => `  - ${indent(p)}`)]),
  ];
  if (violations.length > shown.length) lines.push(`- ... and ${violations.length - shown.length} more`);
  lines.push(
    "",
    `Fix each file now: add a "### <path>" entry to ${path.join(root, PLAN_REL)}, or rename/move the file (mv; the entry heading must then use the new path). Do not delete the plan.`,
  );
  const withExample = violations.find((v) => v.example);
  if (withExample) lines.push("Entry to add, replacing each <...> part with your own text:", fixEntry(withExample.rel, withExample.contract));
  return lines.join("\n");
}

export function exhaustedNotice(violations, times) {
  const names = violations.slice(0, 3).map((v) => v.rel).join(", ");
  return `plan-scan: ${violations.length} new code file(s) still break the plan rules (${names}${violations.length > 3 ? ", ..." : ""}); reported ${times} times, not blocking again until the set changes.`;
}
