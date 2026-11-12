// Wording of every message the guard and audit print.

import path from "node:path";

const LOCKCTL = path.join(import.meta.dirname, "..", "lockctl.mjs");
export const lockctlCmd = `node "${LOCKCTL}"`;

export function dedupeNotices(notices) {
  const seen = new Set();
  return notices.filter((n) => {
    const key = `${n.kind}|${n.file}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function exemptionLine(x) {
  return `exempt: ${x.file} over ${x.check} (${x.measure}; limit ${x.limit}) by exemption "${x.exemption.path}" - ${x.exemption.reason}`;
}

/** The four facts of one finding, one line each. */
function findingLines(f) {
  const lines = [`file: ${f.file}`, `measurement: ${f.measure}`, `limit: ${f.limit}`, `reason: ${f.reason}`];
  if (f.scope) lines.push(`scope: ${f.scope}`);
  return lines;
}

export function denyForFindings(tool, rel, findings, notices, what = "the project policy") {
  const head = `policy-guard BLOCKED ${tool} on ${rel}: the proposed content breaks ${what}. Nothing was written; the file is unchanged.`;
  const shown = findings.slice(0, 4).flatMap(findingLines);
  const more = findings.length > 4 ? [`(+${findings.length - 4} more findings)`] : [];
  const notes = dedupeNotices(notices).map((n) => `note: ${n.text}`);
  return [head, ...shown, ...more, ...notes].join("\n");
}

export function denyForLock(tool, rel, entry, hashState) {
  const stale = hashState === "stale" ? " The file on disk no longer matches the recorded hash (it changed while locked)." : "";
  return [
    `policy-guard BLOCKED ${tool} on ${rel}: the file is locked. Nothing was written.${stale}`,
    `file: ${rel}`,
    `locked since: ${entry.updatedAt || "unknown"} (sha256 ${String(entry.sha256).slice(0, 12)})`,
    `reason: ${entry.reason || "(none recorded)"}`,
    `request: ask the user to authorize an unlock of ${rel} for a stated purpose, then run: ${lockctlCmd} unlock ${rel} --authorized-by "<the user's words>" --purpose "<purpose>"`,
  ].join("\n");
}

export function denyForRecords(tool, rel, recordsRel) {
  return [
    `policy-guard BLOCKED ${tool} on ${rel}: lock records are changed only through lockctl. Nothing was written.`,
    `file: ${rel}`,
    "reason: hand edits could unlock a file without the user's authorization or leave the records corrupt",
    `use: ${lockctlCmd} lock|unlock|relock|status|recover (records file: ${recordsRel})`,
  ].join("\n");
}

export function denyForCorruptRecords(tool, rel, recordsRel, problem) {
  return [
    `policy-guard BLOCKED ${tool} on ${rel}: the lock records are corrupt, so it cannot be known which files are locked. Nothing was written.`,
    `file: ${recordsRel}`,
    `problem: ${problem}`,
    `recover: ${lockctlCmd} recover  (restores the last good copy; direct edits of the records file are blocked)`,
  ].join("\n");
}
