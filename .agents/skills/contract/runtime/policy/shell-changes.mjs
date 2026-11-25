// Which code files did a shell command leave new or changed since the last saved version?
// Asks git (no shell, 10 s timeout) for new and modified files, keeps the ones whose modification time
// is at or after the command's start, and reads each file's text now and (for a changed file) at the last
// save (HEAD). Deleted, renamed and conflicted files are not judged, and neither is anything under a
// folder of third-party or generated output (GENERATED_DIRS): a build tool writes those, nobody can
// split them by responsibility, and Write/Edit are not affected (the file guard still checks them).
// Not a git repository, or git failing: null, and nothing is checked.

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { capApplies } from "./linecap.mjs";

export const GENERATED_DIRS = new Set(["vendor", "dist", "build", "coverage", ".next", "target"]);
const GIT_TIMEOUT_MS = 10_000;
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MTIME_SLACK_MS = 1500; // file timestamps are coarser than the clock, and two processes read the clock a little apart

const git = (root, args) => execFileSync("git", args, {
  cwd: root, encoding: "utf8", timeout: GIT_TIMEOUT_MS, maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "pipe"],
  env: { ...process.env, GIT_OPTIONAL_LOCKS: "0", GIT_PAGER: "cat" },
});

/** `git status --porcelain -z` entries are "XY path"; a rename or copy is followed by one extra field (the old path). */
function statusEntries(output) {
  const fields = output.split("\0");
  const entries = [];
  for (let i = 0; i < fields.length; i++) {
    const xy = fields[i].slice(0, 2);
    if (fields[i].length < 4) continue;
    if (/[RC]/.test(xy)) i++;
    else if (!xy.includes("D") && !xy.includes("U")) entries.push({ gitPath: fields[i].slice(3), isNew: xy === "??" || xy.includes("A") });
  }
  return entries;
}

/**
 * @returns {{rel:string, isNew:boolean, created:boolean, before:string|null, after:string}[]|null} code files a
 * command touched (sorted by git), `before` being the text at the last save (null for a new file). `created`:
 * new to git AND made during the command (creation time at or after the stamp; where the file system keeps
 * no creation time, every file new to git counts), so an older unsaved file that a command edits is not.
 */
export function changedCodeFiles(root, sinceMs) {
  let prefix, status;
  try {
    prefix = git(root, ["rev-parse", "--show-prefix"]).trim(); // the root may sit below the repository top
    status = git(root, ["status", "--porcelain", "-z", "--untracked-files=all", "--", "."]);
  } catch {
    return null;
  }
  const files = [];
  for (const { gitPath, isNew } of statusEntries(status)) {
    if (!gitPath.startsWith(prefix)) continue;
    const rel = gitPath.slice(prefix.length);
    if (!capApplies(rel) || rel.split("/").slice(0, -1).some((seg) => GENERATED_DIRS.has(seg))) continue;
    try {
      const abs = path.join(root, rel);
      const st = fs.statSync(abs);
      if (!st.isFile() || st.size > MAX_FILE_BYTES || st.mtimeMs < sinceMs - MTIME_SLACK_MS) continue;
      const created = isNew && (!(st.birthtimeMs > 0) || st.birthtimeMs >= sinceMs - MTIME_SLACK_MS);
      files.push({ rel, isNew, created, before: isNew ? null : git(root, ["show", `HEAD:${gitPath}`]), after: fs.readFileSync(abs, "utf8") });
    } catch { /* deleted again, unreadable, or not in the last save: not judged */ }
  }
  return files;
}
