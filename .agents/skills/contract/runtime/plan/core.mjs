// Shared pieces of the plan gate and the feature registry: which files are
// gated, where the project root is, logging, and the never-crash wrapper.
// Logs go to $CLAUDE_STATE_DIR/plan-gate/errors.log (stateRoot() honours the env).

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../project-root.mjs";
import { systemMessage } from "../policy/hookio.mjs";

export const CODE_EXT = new Set([".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".py", ".rs", ".go", ".kt", ".java", ".sh", ".html", ".css"]);
export const FILE_TOOLS = new Set(["Write", "Edit", "MultiEdit"]);
export const BOILERPLATE_MAX_LINES = 40;
export const PLAN_REL = ".agent/plans/ACTIVE.md";
export const REGISTRY_REL = ".agent/features.json";

export const TEST_DIRS = new Set(["test", "tests", "__tests__"]);
const SKIP_DIRS = new Set([".agent", ".claude", "node_modules"]);

export const logPath = () => path.join(stateRoot(), "plan-gate", "errors.log");

export function logError(tag, err) {
  try {
    fs.mkdirSync(path.dirname(logPath()), { recursive: true });
    fs.appendFileSync(logPath(), `${new Date().toISOString()}\t${tag}\t${(err && err.stack) || err}\n`);
  } catch { /* logging must not fail the hook */ }
}

/** Run `fn`. An unexpected exception is logged, shown as one visible line, and the call goes through (exit 0). */
export function runHook(tag, fn) {
  try {
    fn();
  } catch (err) {
    logError(tag, err);
    try { systemMessage(`${tag}: internal error, this call was not checked (logged in ${logPath()})`); } catch { /* ignore */ }
  }
  process.exitCode = 0;
}

export const toPosix = (p) => p.split(path.sep).join("/");

export function isInside(parent, child) {
  const r = path.relative(parent, child);
  return r === "" || (r !== ".." && !r.startsWith(`..${path.sep}`) && !path.isAbsolute(r));
}

/**
 * The project root is the nearest ancestor of the target that holds `.git`; with none, the cwd.
 * One refinement: a `.git` found above the cwd that has no `.agent/` while the cwd does (an
 * unversioned project nested in another repository) resolves to the cwd.
 */
export function findProjectRoot(abs, cwd) {
  let gitRoot = null;
  for (let dir = path.dirname(abs); ; dir = path.dirname(dir)) {
    if (fs.existsSync(path.join(dir, ".git"))) { gitRoot = dir; break; }
    if (path.dirname(dir) === dir) break;
  }
  if (!gitRoot) return cwd;
  const nested = gitRoot !== cwd && isInside(gitRoot, cwd) && isInside(cwd, abs);
  if (nested && !fs.existsSync(path.join(gitRoot, ".agent")) && fs.existsSync(path.join(cwd, ".agent"))) return cwd;
  return gitRoot;
}

export function lineCount(text) {
  return text ? text.replace(/\r?\n$/, "").split("\n").length : 0;
}

/** Decide whether creating `rel` (project-relative, forward slashes) is subject to the plan gate. */
export function classify(rel, content) {
  const segs = rel.split("/");
  const base = segs[segs.length - 1];
  const dirs = segs.slice(0, -1);
  if (!CODE_EXT.has(path.extname(base).toLowerCase())) return { gated: false, why: "not a code extension" };
  if (dirs.some((s) => SKIP_DIRS.has(s))) return { gated: false, why: "under .agent/, .claude/ or node_modules/" };
  if (dirs.some((s) => TEST_DIRS.has(s)) || /\.(test|spec)\./i.test(base)) return { gated: false, why: "test file" };
  const stem = base.split(".")[0];
  if (stem === "index" || stem === "main" || base === "__init__.py" || base === "conftest.py") {
    const n = lineCount(content);
    return n <= BOILERPLATE_MAX_LINES ? { gated: false, why: "short boilerplate" } : { gated: true, boilerplateLines: n };
  }
  return { gated: true };
}
