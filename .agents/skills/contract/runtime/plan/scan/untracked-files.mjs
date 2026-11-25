// Which files did the working tree gain since the last commit? Asks git (no shell, 10 s timeout):
// untracked files and files added to the index. Committed files are never reported; a renamed
// file is not new. Paths in the result are relative to the project root, forward slashes.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { CODE_EXT, classify } from "../core.mjs";

const GIT_TIMEOUT_MS = 10_000;

const git = (root, args) => execFileSync("git", args, {
  cwd: root, encoding: "utf8", timeout: GIT_TIMEOUT_MS, maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "pipe"],
  env: { ...process.env, GIT_OPTIONAL_LOCKS: "0", GIT_PAGER: "cat" },
});

/** `git status --porcelain -z` entries are "XY path"; a rename or copy is followed by one extra field (the old path). */
function addedPaths(statusOutput) {
  const fields = statusOutput.split("\0");
  const added = [];
  for (let i = 0; i < fields.length; i++) {
    const xy = fields[i].slice(0, 2);
    if (fields[i].length < 4) continue;
    if (/[RC]/.test(xy)) i++;
    else if (xy === "??" || xy.includes("A")) added.push(fields[i].slice(3));
  }
  return added;
}

/** Every file the working tree gained since the last commit (see addedPaths), project-relative; null when git cannot say. */
export function freshFiles(root) {
  let prefix, out;
  try {
    prefix = git(root, ["rev-parse", "--show-prefix"]).trim(); // the root may sit below the repository top
    out = git(root, ["status", "--porcelain", "-z", "--untracked-files=all", "--", "."]);
  } catch {
    return null;
  }
  return new Set(addedPaths(out).filter((p) => p.startsWith(prefix)).map((p) => p.slice(prefix.length)));
}

/**
 * @returns {{ok:false}|{ok:true, fresh:Set<string>, gated:string[]}} `fresh` is every new file; `gated` the
 * ones the plan gate would have gated (code, not a test/doc/.agent path, not short boilerplate), sorted.
 */
export function newFiles(root) {
  const fresh = freshFiles(root);
  if (!fresh) return { ok: false };
  const gated = [...fresh].sort().filter((rel) => isGated(root, rel));
  return { ok: true, fresh, gated };
}

function isGated(root, rel) {
  if (!CODE_EXT.has(path.extname(rel).toLowerCase())) return false;
  try {
    const abs = path.join(root, rel);
    return fs.lstatSync(abs).isFile() && classify(rel, fs.readFileSync(abs, "utf8")).gated;
  } catch {
    return false; // deleted again, or unreadable
  }
}
