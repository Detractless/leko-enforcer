// Rule R5: a new file lives in a folder that fits its Feature. Folder-based, not name-based: the
// file's own name is never judged here (a name that contains the Feature is fine, never required).
//
//   * Directly in a source root (src, lib, app, bin) or in a role folder (ROLE_FOLDERS): no match
//     needed. Test folders and test files are exempt. The project root is R3's business.
//   * Any other folder is a "feature folder": its name must share a 4-letter prefix (an exact match
//     when either word is shorter than 4 letters) with an INFORMATIVE word of the Feature slug.
//   * A slug word is not informative when (a) it appears in the slugs of at least half of the
//     features already in .agent/features.json (only once the registry holds 3 or more features), or
//     (b) it is a word of package.json "name" or of the project folder's name (only when
//     package.json exists). The message names each ignored word and why.

import fs from "node:fs";
import path from "node:path";
import { TEST_DIRS } from "../core.mjs";
import { readRegistry } from "../entries/registry.mjs";

export const SOURCE_ROOTS = new Set(["src", "lib", "app", "bin"]);
export const ROLE_FOLDERS = new Set(["commands", "cli", "io", "core", "lib", "models", "services", "routes", "adapters", "domain", "ui", "api", "db", "store", "storage", "bin", "scripts"]);
export const GENERIC_SHARE = 0.5;
export const GENERIC_MIN_FEATURES = 3;
const PREFIX = 4;

const TESTISH = /\.(?:test|spec)\./i;
export const wordsOf = (text) => String(text).toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
const prefixLen = (a, b) => { let n = 0; while (n < a.length && n < b.length && a[n] === b[n]) n++; return n; };
export const related = (a, b) => a === b || prefixLen(a, b) >= PREFIX;

/** Words of package.json "name" and of the project folder name; empty without a package.json. */
function projectWords(root) {
  let pkg;
  try { pkg = fs.readFileSync(path.join(root, "package.json"), "utf8"); } catch { return new Set(); }
  let name = "";
  try { name = JSON.parse(pkg.replace(/^\uFEFF/, "")).name; } catch { /* a broken package.json still has a folder name */ }
  return new Set([...wordsOf(typeof name === "string" ? name : ""), ...wordsOf(path.basename(root))]);
}

/** Words that occur in the slugs of at least GENERIC_SHARE of the registered features. */
function widespreadWords(root) {
  const reg = readRegistry(root);
  const keys = reg.status === "ok" ? Object.keys(reg.data) : [];
  if (keys.length < GENERIC_MIN_FEATURES) return new Set();
  const counts = new Map();
  for (const key of keys) for (const w of new Set(wordsOf(key))) counts.set(w, (counts.get(w) || 0) + 1);
  return new Set([...counts].filter(([, n]) => n >= keys.length * GENERIC_SHARE).map(([w]) => w));
}

/** Map word -> reason for the words of `slugWords` that do not identify a feature. */
function ignoredWords(root, slugWords) {
  const project = projectWords(root);
  const wide = widespreadWords(root);
  const ignored = new Map();
  for (const w of slugWords) {
    if (project.has(w)) ignored.set(w, "is part of the project name, so it does not identify this one");
    else if (wide.has(w)) ignored.set(w, "appears in most features, so it does not identify this one");
  }
  return ignored;
}

const exempt = (segs) => {
  const folder = segs[segs.length - 2];
  return !folder || SOURCE_ROOTS.has(folder.toLowerCase()) || ROLE_FOLDERS.has(folder.toLowerCase())
    || segs.slice(0, -1).some((s) => TEST_DIRS.has(s.toLowerCase())) || TESTISH.test(segs[segs.length - 1]);
};

/** @returns {string[]} R5 problems for the new file `rel` of Feature `slug` (empty = fine). */
export function featureFolderProblems(root, rel, slug) {
  const segs = rel.split("/");
  if (exempt(segs)) return [];
  const folder = segs[segs.length - 2];
  const words = wordsOf(slug);
  const ignored = ignoredWords(root, words);
  const useful = words.filter((w) => !ignored.has(w));
  const folderWords = wordsOf(folder);
  if (useful.some((w) => folderWords.some((f) => related(w, f)))) return [];
  const skipped = [...ignored].map(([w, why]) => `"${w}" ${why}`);
  const hint = useful.length ? `Words that identify this Feature: ${useful.join(", ")}.` : "No word of this Feature identifies it; use a Feature that says what the code does.";
  return [`R5 ${rel}: folder "${folder}" does not match Feature "${slug}"${skipped.length ? ` (ignored: ${skipped.join("; ")})` : ""}. ${hint} Put the file in the folder named for its feature, or change the Feature.`];
}
