// File-system access for the engine: list the files it checks and read project files. The listing
// is every file the built-in line cap covers (all code files) plus every file in the policy scope
// (the scope serves the other checks). Symlinked directories are not followed; node_modules, .git
// and the other tool folders of linecap.mjs are skipped; the listing is capped (the cap is reported,
// not hidden).

import fs from "node:fs";
import path from "node:path";
import { inScope } from "./common.mjs";
import { SKIP_DIRS, capApplies } from "./linecap.mjs";
import { underPrefix } from "./paths.mjs";

const FILE_CAP = 5000;
const trimPrefix = (p) => p.replace(/^\.\//, "").replace(/\/+$/, "");

/** A tool folder is walked only when a policy scope prefix points into it. */
const pruned = (policy, base, rel) => base === "node_modules" || base === ".git" || (SKIP_DIRS.has(base) && !policy.scope.some((p) => underPrefix(trimPrefix(p), rel)));

function listFiles(root, policy, cap = FILE_CAP) {
  const found = new Set();
  let truncated = false;
  const walk = (abs, rel) => {
    if (truncated) return;
    let st;
    try { st = fs.lstatSync(abs); } catch { return; }
    if (st.isDirectory()) {
      if (rel && pruned(policy, path.basename(abs), rel)) return;
      for (const name of fs.readdirSync(abs).sort()) walk(path.join(abs, name), rel ? `${rel}/${name}` : name);
    } else if (st.isFile() && (capApplies(rel) || inScope(policy, rel))) {
      if (found.size >= cap) truncated = true; else found.add(rel);
    }
  };
  walk(root, "");
  return { files: [...found].sort(), truncated };
}

/** Disk-backed context for the engine; list() is computed once per context. */
export function fsContext(policy) {
  const { root } = policy;
  let listing = null;
  return {
    policy,
    root,
    read(rel) {
      try { return fs.readFileSync(path.join(root, rel), "utf8"); } catch { return null; }
    },
    exists(rel) {
      try { return fs.statSync(path.join(root, rel)).isFile(); } catch { return false; }
    },
    list() {
      listing ||= listFiles(root, policy);
      return listing.files;
    },
    truncated() {
      this.list();
      return listing.truncated;
    },
  };
}
