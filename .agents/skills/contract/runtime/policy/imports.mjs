// Import graph among project files, cycle detection, and layer direction.
//
// Edges come from tokens, so imports inside comments, strings, or templates do
// not count. Recognised forms (relative specifiers only, i.e. starting with
// "."): require('./x'), import x from './x', import './x', export ... from
// './x', import('./x'). NOT seen: require(variable), computed specifiers, bare
// package names, aliases/paths mappings, package.json "imports"/"exports".
//
// Resolution: exact file, then .js .mjs .cjs .json, then /index.js .mjs .cjs.
// A target outside the graph's file set (not in scope, not JavaScript) is
// "out of scope" and makes no edge; a target that exists nowhere is
// "unresolved" and is reported in the graph statistics.

import path from "node:path";
import { underPrefix } from "./paths.mjs";

const TRY_EXT = ["", ".js", ".mjs", ".cjs", ".json", "/index.js", "/index.mjs", "/index.cjs"];
const unquote = (t) => t.v.slice(1, -1);
const isP = (t, v) => t && t.t === "p" && t.v === v;

/** Raw string specifiers imported by this token stream (relative or not). */
export function importSpecifiers(tokens) {
  const out = [];
  tokens.forEach((t, i) => {
    if (t.t !== "id" || isP(tokens[i - 1], ".")) return;
    const next = tokens[i + 1];
    const str = tokens[i + 2];
    if (t.v === "require" && isP(next, "(") && str && str.t === "str") out.push(unquote(str));
    else if (t.v === "import" && isP(next, "(") && str && str.t === "str") out.push(unquote(str));
    else if (t.v === "import" && next && next.t === "str") out.push(unquote(next));
    else if (t.v === "from" && next && next.t === "str") out.push(unquote(next));
  });
  return out;
}

/** Resolve `spec` imported from `fromRel`. -> {rel}|{outOfScope}|{unresolved}|null (not relative) */
export function resolveSpec(fromRel, spec, nodes, existsOnDisk) {
  if (!spec.startsWith(".")) return null;
  const base = path.posix.normalize(path.posix.join(path.posix.dirname(fromRel), spec));
  if (base.startsWith("..")) return { unresolved: spec };
  for (const ext of TRY_EXT) if (nodes.has(base + ext)) return { rel: base + ext };
  for (const ext of TRY_EXT) if (existsOnDisk(base + ext)) return { outOfScope: base + ext };
  return { unresolved: spec };
}

/**
 * @param {Map<string, string[]>} specsByFile rel -> raw specifiers (nodes = its keys)
 * @returns {{edges: Map<string, Set<string>>, stats: {files:number, edges:number, unresolved:string[], outOfScope:number}}}
 */
export function buildGraph(specsByFile, existsOnDisk) {
  const edges = new Map([...specsByFile.keys()].map((k) => [k, new Set()]));
  const stats = { files: specsByFile.size, edges: 0, unresolved: [], outOfScope: 0 };
  for (const [from, specs] of specsByFile) {
    for (const spec of specs) {
      const r = resolveSpec(from, spec, specsByFile, existsOnDisk);
      if (!r) continue;
      if (r.rel) edges.get(from).add(r.rel);
      else if (r.outOfScope) stats.outOfScope++;
      else stats.unresolved.push(`${from}: ${spec}`);
    }
    stats.edges += edges.get(from).size;
  }
  return { edges, stats };
}

/** Strongly connected components with a cycle (size > 1 or a self-import). -> [{members, path}] */
export function findCycles(edges) {
  const index = new Map();
  const low = new Map();
  const onStack = new Set();
  const stack = [];
  const found = [];
  let counter = 0;
  function visit(v) {
    index.set(v, counter);
    low.set(v, counter++);
    stack.push(v);
    onStack.add(v);
    for (const w of edges.get(v) || []) {
      if (!index.has(w)) { visit(w); low.set(v, Math.min(low.get(v), low.get(w))); }
      else if (onStack.has(w)) low.set(v, Math.min(low.get(v), index.get(w)));
    }
    if (low.get(v) !== index.get(v)) return;
    const comp = [];
    let w;
    do { w = stack.pop(); onStack.delete(w); comp.push(w); } while (w !== v);
    if (comp.length > 1 || edges.get(v).has(v)) found.push(comp.sort());
  }
  for (const v of [...edges.keys()].sort()) if (!index.has(v)) visit(v);
  return found.map((members) => ({ members, path: cyclePath(edges, members) }));
}

function cyclePath(edges, members) {
  const set = new Set(members);
  const start = members[0];
  const prev = new Map([[start, null]]);
  const queue = [start];
  while (queue.length) {
    const v = queue.shift();
    for (const w of [...(edges.get(v) || [])].sort()) {
      if (!set.has(w)) continue;
      if (w === start) {
        const p = [];
        for (let x = v; x !== null; x = prev.get(x)) p.unshift(x);
        return [...p, start];
      }
      if (!prev.has(w)) { prev.set(w, v); queue.push(w); }
    }
  }
  return [...members, start];
}

/** Layer index of a path (longest matching prefix), or -1 when unlayered. */
export function layerOf(rel, layers) {
  let best = -1;
  let bestLen = -1;
  layers.forEach((prefix, idx) => {
    if (underPrefix(rel, prefix) && prefix.length > bestLen) { best = idx; bestLen = prefix.length; }
  });
  return best;
}

/**
 * `layers` is ordered outer -> inner. A file may import its own layer or a
 * LATER layer; importing an EARLIER layer is a violation. Unlayered files are
 * not checked.
 */
export function layerViolations(edges, layers) {
  const out = [];
  for (const [from, targets] of edges) {
    const lf = layerOf(from, layers);
    if (lf < 0) continue;
    for (const to of targets) {
      const lt = layerOf(to, layers);
      if (lt >= 0 && lt < lf) out.push({ from, to, fromLayer: lf, toLayer: lt });
    }
  }
  return out;
}
