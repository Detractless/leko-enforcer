// The file-name rules (R1 form by language, R2 no catch-all words) as guard findings.
// Only a NEW file is judged: an existing file is never blocked for its name, and nothing is renamed.

import { nameProblems } from "../plan/name-rules.mjs";
import { capApplies } from "./linecap.mjs";

export const NAME_CHECK = "fileName";
const LIMIT = "lowercase-with-dashes (snake_case for .py .rs .go, CapitalizedNames for .kt .java), no catch-all words";

/** Findings for a Write that creates `rel` (prev === null). Empty for an existing file or a non-code file. */
export function nameFindings(root, rel, prev) {
  if (prev !== null || !capApplies(rel)) return [];
  return nameProblems(root, rel).map((text) => ({
    check: NAME_CHECK,
    file: rel,
    key: `${NAME_CHECK}:${rel}`,
    measure: `new file name "${rel.split("/").pop()}"`,
    limit: LIMIT,
    reason: text.replace(/^R\d /, ""),
  }));
}
