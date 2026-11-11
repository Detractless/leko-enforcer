// Reads a contract file into plain data. Judging it is contract-gate's job; this module only finds the parts.
// Sections are "## <name>" headings; list items start with "- ". Nothing here throws on odd input.

const HEADING = /^##\s+(.*?)\s*$/;
const ITEM = /^\s*[-*]\s+(.*)$/;
const AC_ITEM = /^(AC-\d{3,}):\s*(.*)$/;
const TAG = /\s*->\s*(criterion:\s*AC-\d{3,}|AC-\d{3,}|out-of-scope:\s*.+)\s*$/;
const FIELD = /^(evidence|breaker):\s*(.*)$/i;

/** Splits "text -> criterion:AC-001" into the text and a normalised tag ({ac} or {outOfScope}); tag is null when absent. */
export function splitTag(line) {
  const m = line.match(TAG);
  if (!m) return { text: line.trim(), tag: null };
  const raw = m[1].replace(/^criterion:\s*/i, "");
  const tag = /^AC-\d+$/i.test(raw) ? { ac: raw.toUpperCase() } : { outOfScope: raw.replace(/^out-of-scope:\s*/i, "").trim() };
  return { text: line.slice(0, m.index).trim(), tag };
}

function sections(text) {
  const out = new Map();
  let name = null;
  let inFence = false;
  for (const line of text.replace(/^\uFEFF/, "").split(/\r?\n/)) {
    if (/^\s*(```|~~~)/.test(line)) inFence = !inFence;
    const h = !inFence && line.match(HEADING);
    if (h) { name = h[1].toLowerCase(); if (!out.has(name)) out.set(name, []); continue; }
    if (name && !/^\s*<!--.*-->\s*$/.test(line)) out.get(name).push(line);
  }
  return out;
}

const items = (lines) => lines.map((l) => l.match(ITEM)?.[1].trim()).filter(Boolean);
const prose = (lines) => lines.map((l) => l.trim()).filter(Boolean).join(" ");

function criteria(lines) {
  const list = [];
  for (const line of lines) {
    const item = line.match(ITEM);
    if (!item) continue;
    const text = item[1].trim();
    const ac = text.match(AC_ITEM);
    if (ac && !/^\s{2,}|^\t/.test(line)) { list.push({ id: ac[1], text: ac[2].trim(), evidence: "", breaker: "" }); continue; }
    const field = text.match(FIELD);
    if (field && list.length) list[list.length - 1][field[1].toLowerCase()] = field[2].trim();
  }
  return list;
}

function tagged(lines, idPattern) {
  return items(lines).map((item) => {
    const { text, tag } = splitTag(item);
    const id = text.match(idPattern);
    return { id: id ? id[1] : null, text, tag };
  });
}

/** @returns {{goal:string, domainsRaw:string, domains:string[], criteria:object[], coverage:object[], derived:object[], edges:object[], outOfScope:string[], decisions:string[]}} */
export function parseContract(text) {
  const s = sections(String(text ?? ""));
  const get = (name) => s.get(name) ?? [];
  const domainLine = get("domains").map((l) => l.match(/^\s*domains:\s*(.*)$/i)?.[1]).find((v) => v !== undefined) ?? "";
  return {
    goal: prose(get("goal")),
    domainsRaw: domainLine,
    domains: domainLine.toLowerCase().split(/[,\s]+/).map((d) => d.replace(/[^a-z]/g, "")).filter(Boolean),
    criteria: criteria(get("acceptance criteria")),
    coverage: tagged(get("checklist coverage"), /^([A-Z]+-\d+)\b/),
    derived: tagged(get("derived checklist"), /^(D-\d+)\b/),
    edges: tagged(get("edge cases"), /^$/),
    outOfScope: items(get("out of scope")),
    decisions: items(get("decisions")),
  };
}
