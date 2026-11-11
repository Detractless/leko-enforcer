// Feature registry: .agent/features.json maps a feature slug to the files that implement it.
//   { "task-export": ["src/export/csv.js"] }
// Reads distinguish absent (treated as empty) from corrupt (reported, never silently reset).
// Writes are atomic (temp file + rename) under a short lock; a lock that cannot be taken is
// ignored after about a second, because losing one update is better than failing the session.

import fs from "node:fs";
import path from "node:path";
import { REGISTRY_REL } from "../core.mjs";

/** Lower-case; repeatedly strip a trailing `2`, `-2`, `-v2`, `-new`, `-old` so near-duplicates share one identity. */
export function normalizeSlug(slug) {
  let s = slug.toLowerCase();
  for (let prev = null; prev !== s;) {
    prev = s;
    const stripped = s.replace(/(?:-v2|-2|-new|-old|2)$/, "");
    if (stripped) s = stripped;
  }
  return s;
}

/** @returns {{status:"ok"|"missing", data:Record<string,string[]>}|{status:"corrupt", why:string}} */
export function readRegistry(root) {
  let text;
  try {
    text = fs.readFileSync(path.join(root, REGISTRY_REL), "utf8");
  } catch (e) {
    if (e.code === "ENOENT") return { status: "missing", data: {} };
    return { status: "corrupt", why: `cannot read it (${e.code || e.message})` };
  }
  let data;
  try { data = JSON.parse(text.replace(/^\uFEFF/, "")); } catch (e) { return { status: "corrupt", why: `invalid JSON (${e.message})` }; }
  if (!data || typeof data !== "object" || Array.isArray(data)) return { status: "corrupt", why: "the top level is not an object" };
  const bad = Object.entries(data).find(([, v]) => !Array.isArray(v) || v.some((f) => typeof f !== "string"));
  if (bad) return { status: "corrupt", why: `the value of "${bad[0]}" is not an array of path strings` };
  return { status: "ok", data };
}

/** Registry entries whose key is the same feature as `slug`: [{key, files}]. */
export function ownersOf(data, slug) {
  const target = normalizeSlug(slug);
  return Object.entries(data).filter(([key]) => normalizeSlug(key) === target).map(([key, files]) => ({ key, files }));
}

function sleep(ms) { Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms); }

/** Runs `fn` while holding `<dir>/.features.lock` (the registry's lock; plan-reconcile takes the same one). */
export function withLock(dir, fn) {
  const lock = path.join(dir, ".features.lock");
  let held = false;
  for (let i = 0; i < 40 && !held; i++) {
    try { fs.closeSync(fs.openSync(lock, "wx")); held = true; } catch (e) {
      if (e.code !== "EEXIST") break;
      try { if (Date.now() - fs.statSync(lock).mtimeMs > 5000) fs.rmSync(lock, { force: true }); } catch { /* raced */ }
      sleep(25);
    }
  }
  try { return fn(); } finally { if (held) fs.rmSync(lock, { force: true }); }
}

/** @returns {{result:"added"|"present"}|{result:"corrupt", why:string}} */
export function addToRegistry(root, slug, rel) {
  const file = path.join(root, REGISTRY_REL);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  return withLock(path.dirname(file), () => {
    const reg = readRegistry(root);
    if (reg.status === "corrupt") return { result: "corrupt", why: reg.why };
    const files = Object.hasOwn(reg.data, slug) ? reg.data[slug] : [];
    if (files.includes(rel)) return { result: "present" };
    reg.data[slug] = [...files, rel];
    const tmp = `${file}.${process.pid}.tmp`;
    fs.writeFileSync(tmp, `${JSON.stringify(reg.data, null, 2)}\n`, "utf8");
    fs.renameSync(tmp, file);
    return { result: "added" };
  });
}
