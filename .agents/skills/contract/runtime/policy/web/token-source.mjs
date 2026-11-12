// H2: the design tokens (:root custom properties) are defined in ONE stylesheet.
// A stylesheet that defines :root variables while another stylesheet in the project already does is refused, so the
// palette cannot drift apart. The exception is a file called tokens.css: it is THE tokens file, and may be created
// while an older stylesheet still holds tokens (that is how the tokens are moved over), but only if no other
// tokens.css exists. The project is scanned only when the proposed file defines tokens at all, to depth 6 and
// 400 stylesheets; minified files and dependency, build and fixture folders are skipped.

import fs from "node:fs";
import path from "node:path";

export const TOKEN_DEF = /:root\s*\{[^}]*--[a-zA-Z0-9-]+\s*:/;
export const SKIP_DIRS = new Set(["node_modules", ".git", ".agent", ".claude", ".venv", "site-packages", "dist", "build", "vendor", "coverage", "fixtures", "__fixtures__", "test-fixtures", "third-party", ".next", ".nuxt"]);
const MAX_DEPTH = 6;
const MAX_FILES = 400;
const MAX_BYTES = 1 << 20;

export const definesTokens = (css) => TOKEN_DEF.test(css.replace(/\/\*[\s\S]*?\*\//g, ""));
const isTokensFile = (rel) => path.posix.basename(rel).toLowerCase() === "tokens.css";

function stylesheets(root) {
  const found = [];
  const walk = (dir, depth) => {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (found.length >= MAX_FILES) return;
      if (e.isDirectory()) {
        if (depth < MAX_DEPTH && !SKIP_DIRS.has(e.name)) walk(path.join(dir, e.name), depth + 1);
      } else if (/\.css$/i.test(e.name) && !/\.min\.css$/i.test(e.name)) {
        found.push(path.relative(root, path.join(dir, e.name)).split(path.sep).join("/"));
      }
    }
  };
  walk(root, 0);
  return found;
}

/** The other stylesheets of the project that already define tokens (project-relative paths, sorted). */
export function otherDefiners(root, rel) {
  return stylesheets(root).filter((f) => f.toLowerCase() !== rel.toLowerCase()).filter((f) => {
    try {
      const abs = path.join(root, f);
      return fs.statSync(abs).size <= MAX_BYTES && definesTokens(fs.readFileSync(abs, "utf8"));
    } catch { return false; }
  }).sort();
}

/** The H2 problem for a stylesheet that defines tokens, or null. `others` is otherDefiners(root, rel). */
export function tokenSourceProblem(rel, others) {
  if (!others.length) return null;
  if (isTokensFile(rel) && !others.some(isTokensFile)) return null;
  return {
    count: 1,
    measure: `${rel} defines :root tokens, and so ${others.length === 1 ? "does" : "do"} ${others.slice(0, 3).join(", ")}${others.length > 3 ? ` (+${others.length - 3} more)` : ""}`,
    limit: "one stylesheet defines the tokens",
    reason: "Tokens have one source of truth: a second file defining :root variables splits the palette and the two drift apart. Keep every --token in ONE stylesheet (styles/tokens.css) and use var(--...) everywhere else.",
  };
}
