// Path helpers: project-root discovery, project-relative paths, globs, and
// "is this the same file" comparison (used by lock matching).

import fs from "node:fs";
import path from "node:path";

export const POLICY_REL = ".claude/project-policy.json";
const CASE_FOLD = process.platform === "darwin" || process.platform === "win32";

/** A real git root: `.git` is a directory holding HEAD, or a file (worktree/submodule pointer). */
export function isGitRoot(dir) {
  const g = path.join(dir, ".git");
  try {
    const st = fs.statSync(g);
    return st.isFile() || fs.existsSync(path.join(g, "HEAD"));
  } catch { return false; }
}

/**
 * The OUTERMOST ancestor of `start` (itself included) that holds the file `rel`, searching up
 * to and including the nearest git root, or up to the filesystem root when there is none.
 */
export function outermostWith(start, rel) {
  let dir = path.resolve(start);
  let found = null;
  for (;;) {
    if (fs.existsSync(path.join(dir, rel))) found = dir;
    if (isGitRoot(dir)) return found;
    const up = path.dirname(dir);
    if (up === dir) return found;
    dir = up;
  }
}

/**
 * The project root: the OUTERMOST directory on the way up (see outermostWith) holding
 * .claude/project-policy.json. Outermost, not nearest: a policy nested below it cannot take
 * over (see shadowPolicies). null when no policy file is found.
 */
export const findProjectRoot = (start) => outermostWith(start, POLICY_REL);

/** Project-relative paths of policy files nested BELOW `root` on the way down to `abs` (ignored for enforcement). */
export function shadowPolicies(root, abs) {
  const out = [];
  let dir = path.dirname(abs);
  while (dir !== root && !path.relative(root, dir).startsWith("..") && dir !== path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, POLICY_REL))) out.push(path.relative(root, path.join(dir, POLICY_REL)).split(path.sep).join("/"));
    dir = path.dirname(dir);
  }
  return out.reverse();
}

/** Absolute path -> posix path relative to root, or null when it lies outside root. */
export function toRel(root, abs) {
  const rel = path.relative(root, abs);
  if (rel === "" || rel.startsWith("..") || path.isAbsolute(rel)) return null;
  return rel.split(path.sep).join("/");
}

/** realpath of the deepest existing ancestor, with the not-yet-existing tail re-appended. */
export function realish(abs) {
  let head = path.resolve(abs);
  const tail = [];
  for (;;) {
    try {
      return path.join(fs.realpathSync(head), ...tail.reverse());
    } catch {
      const up = path.dirname(head);
      if (up === head) return path.resolve(abs);
      tail.push(path.basename(head));
      head = up;
    }
  }
}

function fold(p) {
  return CASE_FOLD ? p.toLowerCase() : p;
}

/** Same file: equal real paths (case-folded on macOS/Windows) or the same device+inode. */
export function sameFile(a, b) {
  const ra = realish(a);
  const rb = realish(b);
  if (fold(ra) === fold(rb)) return true;
  try {
    const sa = fs.statSync(ra);
    const sb = fs.statSync(rb);
    return sa.ino !== 0 && sa.ino === sb.ino && sa.dev === sb.dev;
  } catch {
    return false;
  }
}

/** Glob on project-relative posix paths: `*` within a segment, `**` across segments, `?` one char. */
export function matchesGlob(rel, glob) {
  const g = glob.replace(/^\.\//, "");
  let re = "";
  for (let i = 0; i < g.length; i++) {
    const c = g[i];
    if (c === "*" && g[i + 1] === "*") {
      i++;
      if (g[i + 1] === "/") { i++; re += "(?:.*/)?"; } else re += ".*";
    } else if (c === "*") re += "[^/]*";
    else if (c === "?") re += "[^/]";
    else re += c.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${re}$`).test(rel);
}

/** True when rel is the prefix itself or lies under it (prefix may end with "/"). */
export function underPrefix(rel, prefix) {
  const p = prefix.replace(/^\.\//, "").replace(/\/+$/, "");
  return rel === p || rel.startsWith(`${p}/`);
}
