// Dependency checks over the import graph of the in-scope JavaScript files:
// import cycles (forbidCycles) and layer direction (layers).
//
// The graph is rebuilt from the project's files on every call. For a
// preflight, the target file's PROPOSED content replaces its disk content and
// every other file is read from disk; only violations that are not already
// present in the current graph are reported (an existing cycle never blocks
// an unrelated edit). If the target's import list did not change, the graph is
// not rebuilt at all (nothing could have changed).

import path from "node:path";
import { JS_EXT, applyExemptions, inScope, newOutcome, reasonFor, tokenizeCached } from "./common.mjs";
import { buildGraph, findCycles, importSpecifiers, layerViolations } from "./imports.mjs";

export const graphConfigured = (policy) => policy.cfg.forbidCycles === true || Boolean(policy.cfg.layers);

/** Graph violations. `overrides`: Map rel -> content replacing disk content. */
export function checkGraph(ctx, overrides = new Map()) {
  const { policy } = ctx;
  const out = newOutcome();
  const specs = new Map();
  const names = new Set([...ctx.list(), ...overrides.keys()]);
  for (const rel of [...names].sort()) {
    if (!inScope(policy, rel) || !JS_EXT.has(path.extname(rel).toLowerCase())) continue;
    const text = overrides.has(rel) ? overrides.get(rel) : ctx.read(rel);
    if (text === null || text.includes("\0")) continue;
    const { tokens, error } = tokenizeCached(text);
    if (error) {
      if (!overrides.has(rel)) out.notices.push({ kind: "parser-failure", file: rel, text: `${rel}: left out of the import graph - parser failure: ${error.message} at line ${error.line}` });
      continue;
    }
    specs.set(rel, importSpecifiers(tokens));
  }
  const { edges, stats } = buildGraph(specs, ctx.exists);
  const scopeNote = `import graph: ${stats.files} JavaScript files under ${policy.scope.join(", ")}, ${stats.edges} resolved relative imports, ${stats.unresolved.length} unresolved, ${stats.outOfScope} to files outside the graph`;
  if (policy.cfg.forbidCycles === true) {
    for (const c of findCycles(edges)) {
      out.findings.push({ check: "forbidCycles", file: c.members[0], files: c.members, key: `cycle:${c.members.join("|")}`, measure: `import cycle ${c.path.join(" -> ")}`, limit: "forbidCycles = true", reason: reasonFor(policy, "forbidCycles"), scope: scopeNote });
    }
  }
  if (policy.cfg.layers) {
    const layers = policy.cfg.layers;
    for (const v of layerViolations(edges, layers)) {
      out.findings.push({ check: "layers", file: v.from, files: [v.from], involves: [v.from, v.to], key: `layer:${v.from}>${v.to}`, measure: `${v.from} (layer ${v.fromLayer + 1} "${layers[v.fromLayer]}") imports ${v.to} (layer ${v.toLayer + 1} "${layers[v.toLayer]}")`, limit: `layers outer to inner = ${layers.join(" > ")}; a file may import its own layer or a later one`, reason: reasonFor(policy, "layers"), scope: scopeNote });
    }
  }
  out.stats = { ...stats, scopeNote };
  return applyExemptions(policy, out);
}

/** Graph violations that the proposed content of `rel` would ADD. */
export function checkGraphPreflight(ctx, rel, content, prev) {
  const { policy } = ctx;
  if (!graphConfigured(policy) || !inScope(policy, rel) || !JS_EXT.has(path.extname(rel).toLowerCase())) return newOutcome();
  const next = tokenizeCached(content);
  if (next.error) return newOutcome();
  if (prev !== null) {
    const old = tokenizeCached(prev);
    const same = !old.error && importSpecifiers(old.tokens).join("\n") === importSpecifiers(next.tokens).join("\n");
    if (same) return newOutcome();
  }
  const after = checkGraph(ctx, new Map([[rel, content]]));
  if (!after.findings.length) return after;
  const known = new Set(checkGraph(ctx).findings.map((f) => f.key));
  after.findings = after.findings.filter((f) => !known.has(f.key));
  return after;
}
