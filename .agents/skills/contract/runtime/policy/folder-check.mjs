// Where a NEW code file may go: no loose code in the project root, and at most N code files directly in
// one folder (N = maxFilesPerFolder from .agent/naming-rules.json, default 8). Index, __init__.py,
// conftest.py and test files never count and are never judged.
//
// Everyday mode (no .agent/plans/ACTIVE.md) blocks only a layout that newly breaks a rule: the first loose
// code file in a root that has none, or a folder going from N to N+1. Adding to a root that already has
// loose code, or to a folder already over N, is a notice. Strict mode blocks both, always.
// The root rule needs a real project root (a folder holding .git or .agent): without one (a scratch
// folder, the home folder) the working folder is no project root, so only the folder count applies.
// Not judged: tool folders (SKIP_DIRS), test folders, third-party or generated output (GENERATED_DIRS).
// Nothing here throws: an error is logged and the files pass.

import fs from "node:fs";
import path from "node:path";
import { CODE_EXT, TEST_DIRS, logError } from "../plan/core.mjs";
import { loadRules } from "../plan/name-rules.mjs";
import { SKIP_DIRS } from "./linecap.mjs";
import { GENERATED_DIRS } from "./shell-changes.mjs";

export const ROOT_CHECK = "projectRoot";
export const FOLDER_CHECK = "filesPerFolder";
const ROOT_ENTRIES = [/^main\./, /^index\./, /\.config\.[^.]+$/, /^(?:setup|manage|conftest)\.py$/];
const ROOT_ALLOWED_TEXT = "main.*, index.*, *.config.*, setup.py, manage.py and conftest.py";
const TEST_FILE = /\.(?:test|spec)\.[^.]+$|^test_.+\.py$|_test\.(?:py|go)$|Tests?\.(?:kt|java)$/;
const MAX_SHOWN = 12;

const isCode = (name) => CODE_EXT.has(path.extname(name).toLowerCase());
const uncounted = (name) => name.startsWith(".") || /^index\./.test(name) || name === "__init__.py" || name === "conftest.py" || TEST_FILE.test(name);
const globToRegExp = (g) => new RegExp(`^${g.replace(/[.+^${}()|[\]\\?]/g, "\\$&").replace(/\*/g, "[^/]*")}$`);
const isProjectRoot = (root) => fs.existsSync(path.join(root, ".git")) || fs.existsSync(path.join(root, ".agent"));

function judged(rel) {
  const segs = rel.split("/");
  const base = segs.pop();
  return isCode(base) && !uncounted(base) && !segs.some((s) => SKIP_DIRS.has(s) || TEST_DIRS.has(s) || GENERATED_DIRS.has(s));
}

/** Counted code files directly in `dir` on disk, leaving out the new ones. */
function existingIn(root, dir, fresh) {
  let names;
  try { names = fs.readdirSync(path.join(root, dir), { withFileTypes: true }).filter((e) => e.isFile()).map((e) => e.name); } catch { return []; }
  return names.filter((n) => isCode(n) && !uncounted(n) && !fresh.has(n)).sort();
}

const listed = (names) => (names.length > MAX_SHOWN ? `${names.slice(0, MAX_SHOWN).join(", ")}, ...` : names.join(", ") || "none");

function rootVerdict(root, names, strict, rules, out) {
  const allowed = (n) => ROOT_ENTRIES.some((re) => re.test(n)) || rules.allowedRootFiles.some((g) => globToRegExp(g).test(n));
  const loose = names.filter((n) => !allowed(n));
  if (!loose.length || !isProjectRoot(root)) return;
  const before = existingIn(root, "", new Set(names)).filter((n) => !allowed(n));
  if (!strict && before.length) {
    out.notices.push(`${loose.join(", ")}: new code in the project root, which already had ${before.length} loose code file(s); allowed in everyday mode, but new code belongs under src/ (bin/ for a start-up script). Name it in Status`);
    return;
  }
  for (const n of loose) {
    out.findings.push({
      check: ROOT_CHECK,
      file: n,
      key: `${ROOT_CHECK}:${n}`,
      measure: `new code file "${n}" in the project root${before.length ? "" : ", which has no loose code yet"}`,
      limit: `no loose code files in the project root; ${ROOT_ALLOWED_TEXT} are allowed`,
      reason: `Code goes under src/ (bin/ for a start-up script), so the root stays a short list of entry and config files and everything else is easy to find. Move it, for example to src/${n}.`,
    });
  }
}

function folderCount(root, dir, names, strict, rules, written, out) {
  const max = rules.maxFilesPerFolder;
  const before = existingIn(root, dir, new Set(names));
  const after = before.length + names.length;
  if (after <= max) return;
  if (!strict && before.length > max) {
    out.notices.push(`${dir}/ already held ${before.length} code files (max ${max}) and now gets ${names.length} more (${names.join(", ")}); allowed in everyday mode. Name it in Status as a folder to split into feature subfolders`);
    return;
  }
  const rel = `${dir}/${names[0]}`;
  out.findings.push({
    check: FOLDER_CHECK,
    file: rel,
    key: `${FOLDER_CHECK}:${dir}`,
    measure: `${dir}/ ${written ? "now holds" : "would hold"} ${after} code files (${before.length} before${names.length > 1 ? `; new: ${names.join(", ")}` : ""})`,
    limit: `at most ${max} code files directly in one folder (index and test files not counted)`,
    reason: `A crowded folder hides which files belong together. Group related files into a feature subfolder named after what they do (for example ${dir}/<feature>/). Existing: ${listed(before)}.`,
  });
}

/**
 * @param {string} root project root
 * @param {string[]} rels new code files (posix, project-relative)
 * @param {boolean} strict refactor mode
 * @param {boolean} written true when the files are already on disk (shell commands), false before a write
 * @returns {{findings: object[], notices: string[]}} findings in the guard's shape (finding.file = a new file)
 */
export function folderVerdict(root, rels, strict, written = false) {
  const out = { findings: [], notices: [] };
  try {
    const byDir = new Map();
    for (const rel of rels.filter(judged)) {
      const dir = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
      byDir.set(dir, [...(byDir.get(dir) || []), rel.slice(dir ? dir.length + 1 : 0)]);
    }
    if (!byDir.size) return out;
    const rules = loadRules(root);
    for (const [dir, names] of byDir) {
      if (dir) folderCount(root, dir, names.sort(), strict, rules, written, out);
      else rootVerdict(root, names.sort(), strict, rules, out);
    }
  } catch (err) {
    logError("folder-check", err);
  }
  return out;
}
