// Pieces shared by the content checks, the graph checks, and the entry points:
// scope, exemptions, reasons, outcome objects, and a memoized tokenizer.

import { matchesGlob, underPrefix } from "./paths.mjs";
import { tokenize } from "./tokenizer.mjs";

export const JS_EXT = new Set([".js", ".mjs", ".cjs"]);

const DEFAULT_REASONS = {
  fileCharLimit: "The project policy limits the number of characters per source file.",
  maxFunctionLines: "Function length is counted from the header line to the closing brace line, inclusive.",
  maxNesting: "Nesting counts braced control blocks (if/for/while/switch/try/catch) inside one function.",
  maxBranches: "Branches count if, for, while, catch, case, &&, ||, ??, ?: inside one function.",
  forbidCycles: "The project policy forbids circular imports among project files.",
  layers: "Layers are listed outer to inner; a file may import its own layer or a later one, never an earlier one.",
};

/** Project-relative path is under a scope prefix and not inside node_modules. */
export function inScope(policy, rel) {
  return !rel.split("/").includes("node_modules") && policy.scope.some((p) => underPrefix(rel, p));
}

export const reasonFor = (policy, check) => policy.rules[check] || DEFAULT_REASONS[check];

export function findExemption(policy, rel, check) {
  return policy.exemptions.find((e) => matchesGlob(rel, e.path) && (!e.checks || e.checks.includes(check))) || null;
}

export const newOutcome = () => ({ findings: [], notices: [], exempted: [] });

export function mergeOutcome(into, from) {
  into.findings.push(...from.findings);
  into.notices.push(...from.notices);
  into.exempted.push(...from.exempted);
  return into;
}

const memo = new Map();
/** tokenize() with a small per-process memo (the same text is often needed twice). */
export function tokenizeCached(text) {
  let r = memo.get(text);
  if (!r) {
    if (memo.size > 100) memo.clear();
    r = tokenize(text);
    memo.set(text, r);
  }
  return r;
}

/** Move findings that an exemption covers into `exempted`. `coveredBy(f)` -> exemption|null. */
export function applyExemptions(policy, out) {
  const kept = [];
  for (const f of out.findings) {
    const files = f.files || [f.file];
    const exes = files.map((file) => findExemption(policy, file, f.check));
    if (exes.every(Boolean)) out.exempted.push({ ...f, exemption: exes[0] });
    else kept.push(f);
  }
  out.findings = kept;
  return out;
}
