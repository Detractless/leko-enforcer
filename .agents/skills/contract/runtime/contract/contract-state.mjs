// Where a feature's contract lives and how far along it is. The contract text is frozen with a lock once the
// user opens it, so its progress (draft, open, done, dropped) is kept in a second file next to it.
// All writes are atomic (temp file + rename).

import fs from "node:fs";
import path from "node:path";

export const CONTRACTS_REL = ".agent/contracts";
export const PLANS_REL = ".agent/plans";
export const STATUSES = ["draft", "open", "done", "dropped"];
export const FEATURE_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
export const MAX_FEATURE_CHARS = 40;

export const contractRel = (feature) => `${CONTRACTS_REL}/${feature}.md`;
export const stateRel = (feature) => `${CONTRACTS_REL}/${feature}.state.json`;
export const featureError = (name) => (typeof name === "string" && FEATURE_RE.test(name) && name.length <= MAX_FEATURE_CHARS
  ? null
  : `"${name ?? ""}" is not a feature name: use lowercase words joined by dashes, at most ${MAX_FEATURE_CHARS} characters (for example task-export).`);

export function writeAtomic(file, text) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, text);
  fs.renameSync(tmp, file);
}

/** @returns {object|null} the state record, or null when there is none or it cannot be read. */
export function readState(root, feature) {
  try {
    const data = JSON.parse(fs.readFileSync(path.join(root, stateRel(feature)), "utf8"));
    return data && typeof data === "object" && STATUSES.includes(data.status) ? data : null;
  } catch { return null; }
}

export function writeState(root, feature, patch) {
  const next = { ...(readState(root, feature) ?? {}), ...patch };
  writeAtomic(path.join(root, stateRel(feature)), `${JSON.stringify(next, null, 2)}\n`);
  return next;
}

/** @returns {{feature:string, status:string}[]} every contract that has a state file, sorted by name. */
export function listContracts(root) {
  let names = [];
  try { names = fs.readdirSync(path.join(root, CONTRACTS_REL)); } catch { return []; }
  return names.filter((n) => n.endsWith(".state.json")).map((n) => n.slice(0, -".state.json".length)).sort()
    .map((feature) => ({ feature, status: readState(root, feature)?.status ?? "unreadable" }));
}

/** Contracts still being worked on (draft or open); only these keep a project plan-first. */
export const unfinished = (root) => listContracts(root).filter((c) => c.status === "draft" || c.status === "open");

export const today = () => new Date().toISOString().slice(0, 10);

/** Renames ACTIVE.md to ARCHIVE-<date>.md (-2, -3 ... when taken). Deletes nothing. @returns {string|null} the new file name. */
export function archivePlan(root) {
  const dir = path.join(root, PLANS_REL);
  const from = path.join(dir, "ACTIVE.md");
  if (!fs.existsSync(from)) return null;
  for (let n = 1; ; n++) {
    const name = `ARCHIVE-${today()}${n > 1 ? `-${n}` : ""}.md`;
    if (fs.existsSync(path.join(dir, name))) continue;
    fs.renameSync(from, path.join(dir, name));
    return name;
  }
}
