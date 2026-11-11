// File-name rules for a NEW code file: R1 (the name's form, by language) and R2 (no catch-all words).
// Used by the everyday guard (policy/guard-file.mjs) and, through naming.mjs, by the plan gate.
// Defaults can be overridden by <root>/.agent/naming-rules.json:
//   { "maxFilesPerFolder": 8, "bannedTokens": ["utils", ...], "allowedRootFiles": ["*.config.js"] }
// Unknown keys and wrongly typed values are ignored; an unreadable or corrupt file is logged and
// the defaults apply. Nothing here throws.
//
// R1 by extension: .py .rs .go snake_case (modules cannot contain hyphens); .kt .java CapitalizedNames
// (Java needs the file name to match the class); .tsx .jsx either lowercase-with-dashes or CapitalizedNames
// (React components are usually capitalized); everything else lowercase-with-dashes.

import fs from "node:fs";
import path from "node:path";
import { logError } from "./core.mjs";

export const NAMING_RULES_REL = ".agent/naming-rules.json";
export const DEFAULT_RULES = {
  maxFilesPerFolder: 8,
  bannedTokens: ["utils", "util", "helpers", "helper", "misc", "common", "stuff", "temp", "tmp", "new", "final", "old", "copy", "v2", "v3", "lib2", "general", "other", "things", "shared", "manager"],
  allowedRootFiles: ["*.config.js", "*.config.mjs", "*.config.cjs"],
};

const KEBAB = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const SNAKE = /^[a-z0-9]+(?:[-_][a-z0-9]+)*$/;
const PASCAL = /^[A-Z][A-Za-z0-9]*$/;
const SNAKE_EXT = new Set([".py", ".rs", ".go"]);
const PASCAL_EXT = new Set([".kt", ".java"]);
const EITHER_EXT = new Set([".tsx", ".jsx"]);
const JS_LIKE = new Set([".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx"]);
const CONVENTIONAL_STEMS = new Set(["index", "main"]); // exempt from R1 and R2
const CONVENTIONAL_FILES = new Set(["__init__.py", "conftest.py", "package-info.java", "module-info.java"]);
const isStrings = (v) => Array.isArray(v) && v.every((x) => typeof x === "string");

export function loadRules(root) {
  const rules = { ...DEFAULT_RULES };
  const file = path.join(root, NAMING_RULES_REL);
  let data;
  try {
    data = JSON.parse(fs.readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
    if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("the top level is not an object");
  } catch (e) {
    if (e.code !== "ENOENT") logError("naming-rules", `${NAMING_RULES_REL} ignored, defaults used: ${e.message}`);
    return rules;
  }
  if (Number.isInteger(data.maxFilesPerFolder) && data.maxFilesPerFolder >= 1) rules.maxFilesPerFolder = data.maxFilesPerFolder;
  if (isStrings(data.bannedTokens)) rules.bannedTokens = data.bannedTokens.map((t) => t.toLowerCase());
  if (isStrings(data.allowedRootFiles)) rules.allowedRootFiles = data.allowedRootFiles;
  return rules;
}

/** Lower-case kebab-case form of a name: fooBar, FooBar, foo_bar and "foo bar" all become foo-bar. */
export function kebab(name) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/([A-Z]+)([A-Z][a-z])/g, "$1-$2").replace(/[^A-Za-z0-9]+/g, "-").replace(/^-+|-+$/g, "").toLowerCase();
}

/** CapitalizedNames form: csv-writer, csv_writer and csvWriter all become CsvWriter. */
export function pascal(name) {
  return kebab(name).split("-").filter(Boolean).map((w) => w[0].toUpperCase() + w.slice(1)).join("");
}

export const tokensOf = (name) => name.toLowerCase().split(/[-_]+/).filter(Boolean);

/** The parts of a posix project-relative path the rules look at. */
export function describeName(rel) {
  const segs = rel.split("/");
  const base = segs[segs.length - 1];
  const stem = base.replace(/\.(?:test|spec)\.[^.]+$/i, "").split(".")[0];
  const ext = base.slice(stem.length);
  const lastExt = path.extname(base).toLowerCase();
  const framework = JS_LIKE.has(lastExt) && (/^\[.*\]\.[a-z]+$/i.test(base) || /^_[a-z0-9_-]+\./i.test(base)); // [id].tsx, _app.tsx
  const conventional = CONVENTIONAL_STEMS.has(stem) || CONVENTIONAL_FILES.has(base) || framework;
  return { segs, base, dir: segs.slice(0, -1).join("/"), stem, ext, lastExt, conventional, dotfile: base.startsWith(".") };
}

function formProblem({ lastExt, stem, ext, dir }, rel) {
  const where = dir ? `${dir}/` : "";
  if (PASCAL_EXT.has(lastExt)) {
    if (PASCAL.test(stem)) return [];
    return [`R1 ${rel}: ${lastExt} file names are CapitalizedNames (one per class); rename to ${where}${pascal(stem) || "AName"}${ext}. One naming style per language keeps files predictable to find.`];
  }
  if (EITHER_EXT.has(lastExt) && (PASCAL.test(stem) || KEBAB.test(stem))) return [];
  if ((SNAKE_EXT.has(lastExt) ? SNAKE : KEBAB).test(stem)) return [];
  const fixed = SNAKE_EXT.has(lastExt) ? kebab(stem).replace(/-/g, "_") : kebab(stem);
  const what = SNAKE_EXT.has(lastExt) ? "lowercase snake_case" : "lowercase kebab-case";
  return [`R1 ${rel}: file name must be ${what}; rename to ${where}${fixed || "a-name-that-says-what-it-does"}${ext}. One naming style per language keeps files predictable to find.`];
}

/** A well-formed example name in the style of the file's own language, for the message. */
const exampleName = (lastExt) => (SNAKE_EXT.has(lastExt) ? `csv_writer${lastExt}` : PASCAL_EXT.has(lastExt) ? `CsvWriter${lastExt}` : `csv-writer${lastExt}`);

function bannedProblem({ stem, lastExt }, rel, rules) {
  const hit = tokensOf(kebab(stem)).find((t) => rules.bannedTokens.includes(t));
  if (!hit) return [];
  return [`R2 ${rel}: "${hit}" is a catch-all name: it does not say what the file is for, so it gets misread or turns into a dumping ground. `
    + `Name the file after what it does, its one job (for example ${exampleName(lastExt)}); if you cannot name it that specifically, it probably does too much, so split it. `
    + `Catch-all words: ${rules.bannedTokens.join(", ")}.`];
}

/** R1 + R2 for one new file (the part of the rules that needs no folder scan). Never throws. */
export function nameProblems(root, rel) {
  try {
    const d = describeName(rel);
    if (d.dotfile || d.conventional) return [];
    return [...formProblem(d, rel), ...bannedProblem(d, rel, loadRules(root))];
  } catch (err) {
    logError("name-rules", err);
    return [];
  }
}
