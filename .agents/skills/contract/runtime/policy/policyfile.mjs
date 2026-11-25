// Protection of the policy file itself (.claude/project-policy.json):
//   * policyChange(oldText, newText) - which keys an edit changes (shown in the guard's
//     notice and in the Stop audit's "policy changed since last audit"),
//   * shadowNotice(...) - "shadow policy ignored", reported once per (project, shadow file, content).
// The policy file stays editable through Edit/Write (a user-requested change must be possible);
// the point is that a change is never silent.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../project-root.mjs";
import { POLICY_REL } from "./paths.mjs";

function parseObject(text) {
  if (typeof text !== "string") return undefined;
  try {
    const j = JSON.parse(text.replace(/^\uFEFF/, ""));
    return j && typeof j === "object" && !Array.isArray(j) ? j : undefined;
  } catch { return undefined; }
}

const isObj = (v) => v && typeof v === "object" && !Array.isArray(v);
const show = (v) => {
  const t = JSON.stringify(v);
  return t === undefined ? "undefined" : t.length > 28 ? `${t.slice(0, 27)}...` : t;
};

function flatten(obj) {
  const flat = {};
  for (const [k, v] of Object.entries(obj)) {
    if (isObj(v)) for (const [k2, v2] of Object.entries(v)) flat[`${k}.${k2}`] = v2;
    else flat[k] = v;
  }
  return flat;
}

/**
 * Compare two policy texts. -> { newValid, keys: string[], lockRecordsChange: string|null }
 * `oldText` null = the file did not exist. Keys one level deep are dotted (structure.maxNesting).
 */
export function policyChange(oldText, newText) {
  const next = parseObject(newText);
  if (!next) return { newValid: false, keys: [], lockRecordsChange: null };
  const prev = parseObject(oldText);
  const a = flatten(prev || {});
  const b = flatten(next);
  const keys = [];
  for (const k of Object.keys(b)) {
    if (!(k in a)) keys.push(`${k} (added: ${show(b[k])})`);
    else if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) keys.push(`${k} (${show(a[k])} -> ${show(b[k])})`);
  }
  for (const k of Object.keys(a)) if (!(k in b)) keys.push(`${k} (removed)`);
  let lockRecordsChange = null;
  if (prev && prev.lockRecords !== undefined && JSON.stringify(prev.lockRecords) !== JSON.stringify(next.lockRecords)) {
    lockRecordsChange = next.lockRecords === undefined
      ? "lockRecords is gone, so lock denies stop applying"
      : `lockRecords now points at ${show(next.lockRecords)}; locks recorded in ${show(prev.lockRecords)} stop being enforced`;
  }
  return { newValid: true, keys, lockRecordsChange };
}

/** One-line description of a change for the guard's notice. */
export function describeChange(oldText, newText) {
  const c = policyChange(oldText, newText);
  if (!c.newValid) return "the new content is NOT a valid JSON object: ALL policy checks and lock denies would stop applying";
  const keys = c.keys.length ? c.keys.join(", ") : "no key values change";
  return `keys changed: ${keys}${c.lockRecordsChange ? `; WARNING ${c.lockRecordsChange}` : ""}`;
}

function shadowStateFile() {
  return path.join(stateRoot(), "policy", "shadow-reported.json");
}

/**
 * Notice for nested policy files that are ignored, only for ones not reported before
 * (key = project + shadow path + content hash). Remembers what it returns. [] when nothing is new.
 */
export function shadowNotices(root, shadows) {
  if (!shadows.length) return [];
  let seen = {};
  try { seen = JSON.parse(fs.readFileSync(shadowStateFile(), "utf8")) || {}; } catch { /* nothing remembered */ }
  const fresh = [];
  for (const rel of shadows) {
    let content = "";
    try { content = fs.readFileSync(path.join(root, rel)); } catch { /* not created yet */ }
    const key = crypto.createHash("sha1").update(`${root}\n${rel}\n`).update(content).digest("hex");
    if (!seen[key]) { fresh.push(rel); seen[key] = new Date().toISOString(); }
  }
  if (!fresh.length) return [];
  try {
    fs.mkdirSync(path.dirname(shadowStateFile()), { recursive: true });
    fs.writeFileSync(shadowStateFile(), JSON.stringify(seen));
  } catch { /* cannot remember: it will be reported again, which is visible rather than silent */ }
  return [`shadow policy ignored: ${fresh.join(", ")} is nested below the policy at ${POLICY_REL}; only the outermost policy is enforced, so the nested one has no effect`];
}
