// Repair-only bookkeeping: entries and registry rows whose file was deleted or moved away are retired, not left to rot.
//   * ACTIVE.md: the "### <path>" entry of a file that is registered in features.json but no longer on disk moves under
//     "## Retired entries" (plan/entries/active.mjs). An entry whose file was never registered is a plan for a file not created yet
//     and stays where it is.
//   * features.json: every path that no longer exists is removed; a feature that loses its last file is dropped.
// Nothing else is rewritten. Runs under the registry's own lock (.features.lock); touches nothing when there is nothing to repair.

import fs from "node:fs";
import path from "node:path";
import { PLAN_REL, REGISTRY_REL } from "../core.mjs";
import { listEntries, retireEntries } from "./active.mjs";
import { readRegistry, withLock } from "./registry.mjs";

/** True only when the path is a plain project-relative one and the file is certainly absent. */
function missing(root, rel) {
  if (!rel || path.isAbsolute(rel) || rel.split(/[\\/]/).includes("..")) return false;
  try { fs.lstatSync(path.join(root, rel)); return false; } catch (e) { return e.code === "ENOENT" || e.code === "ENOTDIR"; }
}

function readPlan(root) {
  try { return fs.readFileSync(path.join(root, PLAN_REL), "utf8"); } catch (e) { if (e.code === "ENOENT") return null; throw e; }
}

function writeAtomic(file, text) {
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, text, "utf8");
  fs.renameSync(tmp, file);
}

/** What is stale right now: {plan: new ACTIVE.md text or null, retired: paths, registry: new features object or null, removed: paths}. */
export function findStale(root) {
  const reg = readRegistry(root);
  const out = { plan: null, retired: [], registry: null, removed: [] };
  if (reg.status === "corrupt") return out; // never guess: without a readable registry nothing is known to have existed
  const registered = new Set(Object.values(reg.data).flat());

  const text = readPlan(root);
  if (text !== null) {
    const gone = new Set(listEntries(text.split("\n")).map((e) => e.rel).filter((rel) => registered.has(rel) && missing(root, rel)));
    const r = retireEntries(text, gone);
    if (r.retired.length) { out.plan = r.text; out.retired = r.retired; }
  }

  const next = {};
  for (const [slug, files] of Object.entries(reg.data)) {
    const kept = files.filter((f) => !missing(root, f));
    if (kept.length !== files.length) out.removed.push(...files.filter((f) => !kept.includes(f)));
    if (kept.length || files.length === 0) next[slug] = kept; // only a feature that just lost its last file is dropped
  }
  if (out.removed.length) out.registry = next;
  return out;
}

/** @returns {{retired:string[], removed:string[]}} what was repaired (both empty: nothing changed, nothing written, no lock taken) */
export function reconcile(root) {
  const agent = path.join(root, ".agent");
  const none = { retired: [], removed: [] };
  if (!fs.statSync(agent, { throwIfNoEntry: false })?.isDirectory()) return none;
  const first = findStale(root);
  if (!first.plan && !first.registry) return none;
  return withLock(agent, () => {
    const now = findStale(root); // state may have moved while we waited for the lock
    if (now.plan) writeAtomic(path.join(root, PLAN_REL), now.plan);
    if (now.registry) writeAtomic(path.join(root, REGISTRY_REL), `${JSON.stringify(now.registry, null, 2)}\n`);
    return { retired: now.retired, removed: now.removed };
  });
}
