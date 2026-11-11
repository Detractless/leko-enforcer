// Name and placement rules for a NEW code file (R1-R5). Each rule returns a short message that
// starts with its id. R1 and R2 (the name itself) live in name-rules.mjs, R5 (the file's folder fits
// its Feature) in feature-folder.mjs. Rule overrides: <root>/.agent/naming-rules.json (see name-rules.mjs).
// Nothing here throws.

import fs from "node:fs";
import path from "node:path";
import { CODE_EXT, TEST_DIRS, logError } from "../core.mjs";
import { featureFolderProblems } from "./feature-folder.mjs";
import { DEFAULT_RULES, NAMING_RULES_REL, describeName, kebab, loadRules, nameProblems } from "../name-rules.mjs";

export { DEFAULT_RULES, NAMING_RULES_REL, kebab, loadRules };

const TESTISH = /\.(?:test|spec)\./i;
const globToRegExp = (g) => new RegExp(`^${g.replace(/[.+^${}()|[\]\\?]/g, "\\$&").replace(/\*/g, "[^/]*")}$`);

function siblingsOf(root, dir, own) {
  let names;
  try { names = fs.readdirSync(path.join(root, dir), { withFileTypes: true }).filter((e) => e.isFile()).map((e) => e.name); } catch { return []; }
  return names.filter((n) => n !== own && CODE_EXT.has(path.extname(n).toLowerCase()) && !TESTISH.test(n) && n.split(".")[0] !== "index").sort();
}

/**
 * @param {string} root project root
 * @param {string} rel posix path of the new file, relative to `root`
 * @param {string|null} slug the entry's Feature slug (R5 is skipped without a valid one)
 * @returns {string[]} problems, each starting with its rule id; empty = fine
 */
export function namingProblems(root, rel, slug = null) {
  try {
    return judge(root, rel, slug, loadRules(root));
  } catch (err) {
    logError("naming", err);
    return [];
  }
}

function judge(root, rel, slug, rules) {
  const d = describeName(rel);
  if (d.dotfile) return []; // dotfiles (.eslintrc.js) have no stem to judge
  const rootOk = !d.dir && rules.allowedRootFiles.some((g) => globToRegExp(g).test(d.base));
  const inTests = d.segs.slice(0, -1).some((s) => TEST_DIRS.has(s));
  return [
    ...nameProblems(root, rel),
    ...locationProblem(rel, d.dir, rootOk),
    ...(d.dir && !inTests && d.stem !== "index" ? crowdingProblem(root, rel, d.dir, d.base, rules) : []),
    ...(slug ? featureFolderProblems(root, rel, slug) : []),
  ];
}

function locationProblem(rel, dir, rootOk) {
  return dir || rootOk ? [] : [`R3 ${rel}: new code files do not go in the project root; put it under src/ (or bin/ for an entry script).`];
}

function crowdingProblem(root, rel, dir, base, rules) {
  const others = siblingsOf(root, dir, base);
  if (others.length + 1 <= rules.maxFilesPerFolder) return [];
  const shown = others.length > 12 ? `${others.slice(0, 12).join(", ")}, ...` : others.join(", ");
  return [`R4 ${rel}: ${dir}/ would hold ${others.length + 1} code files (max ${rules.maxFilesPerFolder}, tests and index.* not counted). Group related files into a feature subfolder (for example ${dir}/<feature>/). Existing: ${shown}.`];
}
