// The built-in file-size cap: a code file may have at most FILE_LINE_CAP lines.
//
// This is a constant. The project policy, environment variables and files cannot change it: the
// old `fileLineLimit` and `scopePaths` keys of project-policy.json have no effect on it (scopePaths
// still scopes the other checks). It applies to EVERY code file in the project (extensions from the
// plan gate's CODE_EXT), tests included, anywhere in the tree, and no exemption covers it.
// Lines are counted with countLines() (lines.mjs), the engine's one counting rule.
//
// Skipped: dependency and tool folders (SKIP_DIRS) and every non-code file (lockfiles, json, md).
// Preflight keeps the engine's rule for files that are already over the cap: a change that does not
// make such a file longer is allowed with a visible notice, so an oversized file can still be shrunk.
// Everyday mode (strict = false, no .agent/plans/ACTIVE.md) goes one step further: a file that was
// already over the cap before the change may also GROW, with a notice that says so. A new file, or a
// file at or under the cap, can never go over it in any mode.

import path from "node:path";
import { CODE_EXT } from "../plan/core.mjs";
import { newOutcome } from "./common.mjs";
import { countLines } from "./lines.mjs";

export const FILE_LINE_CAP = 350;
export const CAP_CHECK = "fileLineCap";
export const SKIP_DIRS = new Set(["node_modules", ".git", ".agent", ".claude", ".venv", "site-packages"]);

const FIX = "split the file by responsibility";

/** True when the cap applies to this project-relative posix path. */
export function capApplies(rel) {
  const segs = rel.split("/");
  return CODE_EXT.has(path.extname(segs[segs.length - 1]).toLowerCase()) && !segs.slice(0, -1).some((s) => SKIP_DIRS.has(s));
}

/** The outcome ({findings, notices, exempted}) of the cap for one file's complete content. */
export function checkLineCap(mode, rel, content, prev, strict = true) {
  const out = newOutcome();
  if (!capApplies(rel)) return out;
  const lines = countLines(content);
  if (lines <= FILE_LINE_CAP) return out;
  const before = mode === "preflight" && prev !== null ? countLines(prev) : null;
  if (before !== null && lines <= before) {
    out.notices.push({ kind: "over-limit", file: rel, text: `${rel} is still over the ${FILE_LINE_CAP}-line cap (${lines} > ${FILE_LINE_CAP}) but this change does not make it longer, so it is allowed; split it by responsibility` });
    return out;
  }
  if (before !== null && before > FILE_LINE_CAP && !strict) {
    out.notices.push({ kind: "over-limit", file: rel, text: `${rel} was already over the ${FILE_LINE_CAP}-line cap and this change makes it longer (${before} -> ${lines}); allowed in everyday mode. Name it in Status as a file to split` });
    return out;
  }
  out.findings.push({
    check: CAP_CHECK,
    file: rel,
    key: `${CAP_CHECK}:${rel}`,
    measure: `${lines} lines ${mode === "preflight" ? "in the proposed content" : "in the current file"}`,
    limit: `${FILE_LINE_CAP} lines per code file, built in`,
    reason: `A code file may not exceed ${FILE_LINE_CAP} lines, tests included: a longer file holds more than one responsibility. Split the file by responsibility into smaller files, each with one job. The limit is built in; the project policy cannot change it.`,
    fix: FIX,
  });
  return out;
}
