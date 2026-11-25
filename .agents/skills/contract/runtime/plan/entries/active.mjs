// The entry list of ACTIVE.md as text: which "### <path>" entries exist, and moving the entries of deleted
// files under "## Retired entries". Line based on "\n", so every byte we do not touch (CRLF, other text) survives.
// An active entry is a level-3 heading outside code fences. A retired one is "#### <path> (retired)", which
// plan/entries/entry.mjs findEntry never matches, so the gate, plan-scan and plan-registry ignore it.

import { RETIRED_MARK } from "./entry.mjs";

export const RETIRED_SECTION = "Retired entries";
const HEADING = /^(#{1,6})\s+(.*?)\s*$/;
const FENCE = /^\s*(```|~~~)/;

/** Headings outside code fences, in file order: {level, text, i} (i = line index), with `end` = the next heading's index. */
function headings(lines) {
  const found = [];
  let inFence = false;
  lines.forEach((line, i) => {
    if (FENCE.test(line)) { inFence = !inFence; return; }
    if (inFence) return;
    const h = (i === 0 ? line.replace(/^\uFEFF/, "") : line).match(HEADING);
    if (h) found.push({ level: h[1].length, text: h[2], i });
  });
  return found.map((h, k) => ({ ...h, end: found[k + 1]?.i ?? lines.length }));
}

/** Active entries in file order: `rel` is the heading's text, [start, end) its lines up to the next heading. */
export function listEntries(lines) {
  return headings(lines).filter((h) => h.level === 3).map((h) => ({ rel: h.text, start: h.i, end: h.end }));
}

/** Where new retired items go: right after the last non-blank line of the existing section, or -1 when there is none. */
function retiredInsertAt(lines) {
  const heads = headings(lines);
  const k = heads.findIndex((h) => h.level === 2 && h.text === RETIRED_SECTION);
  if (k < 0) return -1;
  const next = heads.slice(k + 1).find((h) => h.level <= 3);
  let at = next ? next.i : lines.length;
  while (at > heads[k].i + 1 && !lines[at - 1].trim()) at--;
  return at;
}

/**
 * Moves the entries whose path is in `gone` under "## Retired entries" (created at the end when missing).
 * Only the heading line changes ("### p" -> "#### p (retired)") plus one note line; the entry's own lines are kept.
 * @returns {{text:string, retired:string[]}} `text` is the input itself when nothing was retired
 */
export function retireEntries(text, gone) {
  const finalEol = text.endsWith("\n");
  const lines = (finalEol ? text.slice(0, -1) : text).split("\n");
  const hits = listEntries(lines).filter((e) => gone.has(e.rel));
  if (!hits.length) return { text, retired: [] };
  const blank = lines.some((l) => l.endsWith("\r")) ? "\r" : "";

  const block = [];
  const drop = new Set();
  for (const e of hits) {
    const body = lines.slice(e.start + 1, e.end);
    while (body.length && !body[body.length - 1].trim()) body.pop();
    block.push(blank, `#### ${e.rel}${RETIRED_MARK}${blank}`, `_File removed; entry kept for history by plan-reconcile._${blank}`, ...body);
    for (let i = e.start; i < e.end; i++) drop.add(i);
  }
  const rest = lines.filter((_, i) => !drop.has(i));

  const at = retiredInsertAt(rest);
  if (at >= 0) {
    rest.splice(at, 0, ...block);
  } else {
    while (rest.length && !rest[rest.length - 1].trim()) rest.pop();
    rest.push(blank, `## ${RETIRED_SECTION}${blank}`, ...block);
  }
  return { text: rest.join("\n") + (finalEol ? "\n" : ""), retired: hits.map((e) => e.rel) };
}
