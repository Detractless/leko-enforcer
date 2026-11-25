// Lock records: read, validate, write atomically, and compare with the files.
//
// File shape: { "locks": [ { path, sha256, state: "locked"|"unlocked", reason,
// evidence, authorizedBy, updatedAt, ...extra } ] }. Extra fields written by
// lockctl: purpose, unlockedAt, unlockedFromSha256, staleAtUnlock, verified,
// debt, history.
//
// Writes: temp file in the same directory, fsync, rename. The previous good
// file is copied to <file>.bak first. A corrupt records file is never
// overwritten (writeRecords refuses); `lockctl recover` is the way back.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { sameFile } from "./paths.mjs";

export const sha256Of = (buf) => crypto.createHash("sha256").update(buf).digest("hex");

export function sha256File(abs) {
  try {
    return sha256Of(fs.readFileSync(abs));
  } catch {
    return null;
  }
}

export function recordsProblems(data) {
  if (!data || typeof data !== "object" || Array.isArray(data)) return ["top level must be an object"];
  if (!Array.isArray(data.locks)) return ['"locks" must be an array'];
  const problems = [];
  const seen = new Set();
  data.locks.forEach((e, i) => {
    if (!e || typeof e !== "object") return problems.push(`locks[${i}] is not an object`);
    if (typeof e.path !== "string" || e.path === "") problems.push(`locks[${i}].path missing`);
    else if (seen.has(e.path)) problems.push(`locks[${i}].path duplicated: ${e.path}`);
    else seen.add(e.path);
    if (typeof e.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(e.sha256)) problems.push(`locks[${i}].sha256 is not 64 hex chars`);
    if (e.state !== "locked" && e.state !== "unlocked") problems.push(`locks[${i}].state must be "locked" or "unlocked"`);
  });
  return problems;
}

/** @returns {{state:"missing"|"corrupt"|"ok", data:object|null, problems:string[]}} */
export function readRecords(file) {
  let raw;
  try {
    raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  } catch (e) {
    if (e.code === "ENOENT") return { state: "missing", data: null, problems: [] };
    return { state: "corrupt", data: null, problems: [`cannot read: ${e.code || e.message}`] };
  }
  let data;
  try {
    data = JSON.parse(raw);
  } catch (e) {
    return { state: "corrupt", data: null, problems: [`not valid JSON: ${e.message}`] };
  }
  const problems = recordsProblems(data);
  return problems.length ? { state: "corrupt", data: null, problems } : { state: "ok", data, problems: [] };
}

/** Atomic write with a .bak of the previous good file. Throws if the current file is corrupt. */
export function writeRecords(file, data) {
  const cur = readRecords(file);
  if (cur.state === "corrupt") throw new Error(`refusing to overwrite corrupt lock records ${file}: ${cur.problems[0]}`);
  const bad = recordsProblems(data);
  if (bad.length) throw new Error(`refusing to write invalid lock records: ${bad[0]}`);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  if (cur.state === "ok") fs.copyFileSync(file, `${file}.bak`);
  const tmp = `${file}.tmp-${process.pid}-${Date.now()}`;
  const fd = fs.openSync(tmp, "w");
  try {
    fs.writeSync(fd, `${JSON.stringify(data, null, 2)}\n`);
    fs.fsyncSync(fd);
  } finally {
    fs.closeSync(fd);
  }
  fs.renameSync(tmp, file);
}

/** "ok" (hash matches the record), "stale" (file differs), or "missing". */
export function hashState(root, entry) {
  const abs = path.join(root, entry.path);
  const sha = sha256File(abs);
  if (sha === null) return "missing";
  return sha === entry.sha256 ? "ok" : "stale";
}

/** The records entry that refers to `abs` (by real path, case fold, or inode), or null. */
export function findEntry(root, records, abs) {
  return (records?.locks || []).find((e) => sameFile(path.join(root, e.path), abs)) || null;
}

/** Absolute paths of the files lockctl owns: records and its .bak. */
export function recordFiles(root, lockRecordsRel) {
  const f = path.join(root, lockRecordsRel);
  return [f, `${f}.bak`];
}
