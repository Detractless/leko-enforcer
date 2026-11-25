// Reads a Codex apply_patch text and works out what each touched file WOULD contain afterwards,
// so the same checks Claude Code runs on Write/Edit can run on it before anything is written.
//
//   *** Begin Patch / *** Add File: p / *** Update File: p (+ *** Move to: q) / *** Delete File: p / *** End Patch
//
// proposedFiles(patch, cwd) -> [{ kind, abs, from?, content?, skipped? }]
//   kind "add" | "update" | "move" | "delete".  For "move", abs is the new path and from the old one.
//   skipped is set (with a reason) when the new content cannot be computed; the caller then checks
//   locks only, and Codex reports the patch error itself.

import fs from "node:fs";
import path from "node:path";

const HEADER = /^\*\*\* (Add File|Delete File|Update File|Move to|End Patch|End of File)(?:: (.*))?$/;

/** Splits the patch text into one record per file section. Lines outside a section are ignored. */
export function parsePatch(text) {
  const sections = [];
  let cur = null;
  let hunk = null;
  for (const line of String(text).replace(/\r\n/g, "\n").split("\n")) {
    const h = HEADER.exec(line);
    if (h && h[1] === "End Patch") break;
    if (h && h[1] === "End of File") { if (hunk) hunk.eof = true; continue; }
    if (h && h[1] === "Move to") { if (cur) cur.moveTo = h[2].trim(); continue; }
    if (h) {
      cur = { kind: h[1] === "Add File" ? "add" : h[1] === "Delete File" ? "delete" : "update", path: h[2].trim(), lines: [], hunks: [] };
      sections.push(cur);
      hunk = null;
      continue;
    }
    if (!cur || cur.kind === "delete") continue;
    if (cur.kind === "add") { if (line.startsWith("+")) cur.lines.push(line.slice(1)); continue; }
    if (line.startsWith("@@")) { hunk = { old: [], next: [], eof: false }; cur.hunks.push(hunk); continue; }
    if (!hunk) { hunk = { old: [], next: [], eof: false }; cur.hunks.push(hunk); }
    const mark = line[0];
    if (mark === "+") hunk.next.push(line.slice(1));
    else if (mark === "-") hunk.old.push(line.slice(1));
    else if (mark === " " || line === "") { hunk.old.push(line.slice(1)); hunk.next.push(line.slice(1)); }
  }
  return sections;
}

const same = (a, b) => a === b || a.trimEnd() === b.trimEnd() || a.trim() === b.trim();

function indexOfRun(lines, run, from) {
  for (let i = from; i + run.length <= lines.length; i++) {
    if (run.every((l, k) => same(lines[i + k], l))) return i;
  }
  return -1;
}

/** Applies the hunks in order to the current text; null when one cannot be placed. */
export function applyHunks(prev, hunks) {
  const endsNl = prev.endsWith("\n");
  const lines = prev === "" ? [] : prev.replace(/\n$/, "").split("\n");
  let cursor = 0;
  for (const h of hunks) {
    let at = h.old.length === 0 ? lines.length : indexOfRun(lines, h.old, cursor);
    if (at === -1 && cursor > 0) at = indexOfRun(lines, h.old, 0);
    if (at === -1) return null;
    lines.splice(at, h.old.length, ...h.next);
    cursor = at + h.next.length;
  }
  return lines.length ? lines.join("\n") + (endsNl || prev === "" ? "\n" : "") : "";
}

function readText(abs) {
  try { return fs.readFileSync(abs, "utf8"); } catch { return null; }
}

export function proposedFiles(patchText, cwd) {
  return parsePatch(patchText).map((s) => {
    const abs = path.resolve(cwd, s.path);
    if (s.kind === "delete") return { kind: "delete", abs };
    if (s.kind === "add") return { kind: "add", abs, content: s.lines.length ? s.lines.join("\n") + "\n" : "" };
    const prev = readText(abs);
    const content = prev === null ? null : applyHunks(prev, s.hunks);
    const to = s.moveTo ? path.resolve(cwd, s.moveTo) : abs;
    const out = { kind: s.moveTo ? "move" : "update", abs: to, from: abs };
    if (content === null) return { ...out, skipped: prev === null ? "the file does not exist" : "a hunk did not match the file" };
    return { ...out, content };
  });
}
