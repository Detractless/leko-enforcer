// Judges code files a shell command created or changed, with the same rules as the Write/Edit guard
// (the 350-line cap; the file-name and folder rules for NEW files; the web rules of web/web-check.mjs). The files are already on disk, so the
// answer is a report to the model, not a refusal: { problems, notices } where a problem is
// { rel, findings } and a notice is one line of text (an over-cap file that did not get longer, or grew
// in everyday mode; a root or folder that already broke a folder rule and got more).

import { dedupeNotices } from "./format.mjs";
import { folderVerdict } from "./folder-check.mjs";
import { checkLineCap } from "./linecap.mjs";
import { isStrict } from "./mode.mjs";
import { nameFindings } from "./namecheck.mjs";
import { webFindings } from "./web/web-check.mjs";

const MAX_LISTED = 8;

export function judgeChanges(root, files) {
  const strict = isStrict(root);
  const problems = [];
  const notices = [];
  for (const f of files) {
    const out = checkLineCap("preflight", f.rel, f.after, f.before, strict);
    const findings = [...out.findings.map((x) => ({ ...x, measure: x.measure.replace("in the proposed content", "in the file now") })), ...nameFindings(root, f.rel, f.before), ...webFindings(root, f.rel, f.after, f.before)];
    if (findings.length) problems.push({ rel: f.rel, findings });
    notices.push(...dedupeNotices(out.notices).map((n) => n.text));
  }
  const folder = folderVerdict(root, files.filter((f) => f.created).map((f) => f.rel), strict, true);
  for (const x of folder.findings) {
    const own = problems.find((p) => p.rel === x.file);
    if (own) own.findings.push(x);
    else problems.push({ rel: x.file, findings: [x] });
  }
  notices.push(...folder.notices);
  return { problems, notices };
}

const finding = (f) => `  - ${f.measure}; limit: ${f.limit}. ${f.reason}`;

/** The text handed back to the model after a command that left rule-breaking files. */
export function problemReport(problems, notices) {
  const shown = problems.slice(0, MAX_LISTED);
  const lines = [
    `shell-scan: a shell command left ${problems.length} code file(s) that break a code-layout rule. The files are already on disk, so nothing was refused.`,
    ...shown.flatMap((p) => [`- ${p.rel}`, ...p.findings.map(finding)]),
  ];
  if (problems.length > shown.length) lines.push(`- ... and ${problems.length - shown.length} more`);
  lines.push("", "Fix them now: split an oversized file by responsibility into smaller files, rename a badly named new file, or move new files out of the root or into a feature subfolder (mv). Make the changes with Write/Edit, which the guard checks before it writes.");
  lines.push(...notices.map((n) => `note: ${n}`));
  return lines.join("\n");
}
