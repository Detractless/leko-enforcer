// The web rules of the write guard, as findings for guard-file.mjs: HTML kept to markup (H3-H6), colors and z-index
// from tokens (H1, X1), one tokens file (H2), and the component folder contract (C1). See the files beside this one.
// Font sizes from tokens are measured by color-tokens.mjs too, but only the /refactor scan reports them (REPORT_ONLY):
// a literal size is a habit, not a mistake, and refusing every new text size cost more than it gave.
//
//   webFindings(root, rel, content, prev) -> findings  ({check, file, key, measure, limit, reason}, like linecap.mjs)
//
// Only NEW problems block: the proposed text is compared with the text on disk (`prev`, null for a new file) and a
// check is refused only when the change makes its count larger, so an old file that already breaks a rule can still be
// edited. Applies to .html, .htm and .css files and to code files under components/; skips dependency, build and
// fixture folders and minified files. Off with <state>/web-rules-off (code-layout-off stops the guard before this runs).

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../../project-root.mjs";
import { componentProblems } from "./component-folder.mjs";
import { cssProblems } from "./color-tokens.mjs";
import { htmlProblems } from "./html-concerns.mjs";
import { SKIP_DIRS, definesTokens, otherDefiners, tokenSourceProblem } from "./token-source.mjs";

const HTML = /\.html?$/i;
const CSS = /\.css$/i;
const REPORT_ONLY = new Set(["webFontSizeLiteral"]);

export const webRulesOff = () => fs.existsSync(path.join(stateRoot(), "web-rules-off"));

function applies(rel) {
  const segs = rel.split("/");
  return !segs.slice(0, -1).some((s) => SKIP_DIRS.has(s)) && !/\.min\.css$/i.test(rel);
}

function problemsOf(root, rel, text, others) {
  const out = componentProblems(rel);
  if (HTML.test(rel)) Object.assign(out, htmlProblems(text));
  if (CSS.test(rel)) {
    const tokens = definesTokens(text);
    Object.assign(out, cssProblems(text, tokens));
    const found = tokens ? tokenSourceProblem(rel, others()) : null;
    if (found) out.webTokenSource = found;
  }
  return out;
}

/** Findings for the proposed full text of `rel` (project-relative, posix). Never throws. */
export function webFindings(root, rel, content, prev) {
  try {
    if (!applies(rel) || webRulesOff()) return [];
    let cached = null;
    const others = () => (cached ??= otherDefiners(root, rel));
    const now = problemsOf(root, rel, content, others);
    if (!Object.keys(now).length) return [];
    const before = prev === null ? {} : problemsOf(root, rel, prev, others);
    return Object.entries(now)
      .filter(([check, p]) => !REPORT_ONLY.has(check) && p.count > (before[check]?.count ?? 0))
      .map(([check, p]) => ({ check, file: rel, key: `${check}:${rel}`, measure: p.measure, limit: p.limit, reason: p.reason }));
  } catch {
    return []; // a failure in these rules must never block a write
  }
}
