// Reads one `### <path>` entry out of ACTIVE.md and judges its fields.
// Fields are `**Name:** value` or `Name: value` lines (an optional list bullet is fine);
// a value may continue on following non-blank lines until the next field or heading.

export const EXAMPLE = {
  purpose: "Serialize the task list to CSV text for the export command.",
  feature: "task-export",
  searched: "`grep -rn -e csv -e export src` shows only src/format.js, a JSON printer that cannot emit CSV, so a new writer is needed.",
  failure: "An empty task list yields only the header row; a field containing a comma or quote is quoted and escaped.",
};
export const EXAMPLE_PATH = "src/export/csv.js";

const norm = (t) => t.toLowerCase().replace(/\s+/g, " ").trim();
const EXAMPLE_TEXT = new Set([EXAMPLE.purpose, EXAMPLE.searched, EXAMPLE.failure].map(norm));

/** The 6-line example shown in every denial about a missing or weak entry. */
export function exampleEntry(rel) {
  return [
    `### ${rel}`,
    `**Purpose:** ${EXAMPLE.purpose}`,
    `**Feature:** ${EXAMPLE.feature}`,
    `**Existing code checked:** ${EXAMPLE.searched}`,
    `**Failure paths:** ${EXAMPLE.failure}`,
    "**Split justification:** <only if the Feature is already in .agent/features.json: why this file is a separate responsibility>",
  ].join("\n");
}

/** plan-reconcile files an entry of a deleted file under "#### <path> (retired)": never an active entry, never a near miss. */
export const RETIRED_MARK = " (retired)";

const FIELD_START = /^\s*(?:[-*+]\s+)?\*{0,2}([A-Za-z][A-Za-z ]{1,40}?)\*{0,2}\s*:\s*\*{0,2}\s*(.*)$/;
const HEADING = /^(#{1,6})\s+(.*?)\s*$/;
const FENCE = /^\s*(```|~~~)/;

/** @returns {{found:boolean, fields:Map<string,string>, nearMiss:string|null}} fields are keyed by lower-case name. */
export function findEntry(planText, rel) {
  const lines = planText.replace(/^\uFEFF/, "").split(/\r?\n/);
  let inFence = false;
  let current = null;
  let nearMiss = null;
  const loose = (t) => t.replace(/[`'"]/g, "").replace(/\\/g, "/").replace(/^\.\//, "").trim().toLowerCase();
  const exactSections = [];
  for (const line of lines) {
    if (FENCE.test(line)) { inFence = !inFence; continue; }
    if (inFence) continue;
    const h = line.match(HEADING);
    if (h) {
      current = null;
      if (h[1] === "###" && h[2] === rel) {
        current = [];
        exactSections.push(current);
      } else if (!h[2].endsWith(RETIRED_MARK) && (loose(h[2]) === rel.toLowerCase() || h[2].includes(rel))) {
        nearMiss ??= line.trim();
      }
      continue;
    }
    if (current) current.push(line);
  }
  const body = exactSections[exactSections.length - 1];
  return body ? { found: true, fields: parseFields(body), nearMiss: null } : { found: false, fields: new Map(), nearMiss };
}

function parseFields(bodyLines) {
  const fields = new Map();
  let name = null;
  for (const line of bodyLines) {
    const m = line.match(FIELD_START);
    if (m) {
      name = m[1].trim().replace(/\s+/g, " ").toLowerCase();
      if (!fields.has(name)) fields.set(name, m[2].trim());
      else name = null;
    } else if (!line.trim()) {
      name = null;
    } else if (name) {
      fields.set(name, `${fields.get(name)} ${line.trim()}`.trim());
    }
  }
  return fields;
}

const FILLER = "(?:n\\/?a|tbd|tba|todo|fixme|none|nil|null|unknown|pending|placeholder|stub|etc|yes|no|ok|\\?+|-+|_+|\\.+|\\u2026+|x+)";
const FILLER_RE = new RegExp(`^${FILLER}(?:[\\s,.;/]+${FILLER})*[.]*$`, "i");

export function isPlaceholder(value) {
  const t = value.trim().toLowerCase();
  if (!t || /^<.*>$/.test(t) || FILLER_RE.test(t)) return true;
  const toks = t.split(/\s+/);
  if (toks.length >= 3 && new Set(toks).size <= 2) return true;
  return new Set(t.replace(/\s/g, "")).size < 5; // "aaaaaaaa...", "abababab..."
}

const realWords = (t) => t.split(/\s+/).map((w) => w.replace(/[^A-Za-z0-9]/g, "")).filter((w) => w.length >= 2);
const clip = (t, n = 40) => (t.length > n ? `${t.slice(0, n)}...` : t);

/** @returns {string|null} why the value is not acceptable, or null. */
export function weakness(value, { minChars = 25, minWords = 0 } = {}) {
  const v = (value ?? "").trim();
  if (!v) return "empty";
  if (isPlaceholder(v)) return `placeholder ("${clip(v)}"); answer it for real`;
  if (EXAMPLE_TEXT.has(norm(v))) return "copied verbatim from the example; write your own";
  const words = realWords(v).length;
  if (v.length < minChars || words < minWords) {
    const need = minWords ? `${minChars} chars and >= ${minWords} words` : `${minChars} chars`;
    return `too thin (${v.length} chars, ${words} ${words === 1 ? "word" : "words"}; need >= ${need})`;
  }
  return null;
}

export const SLUG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const cleanSlug = (v) => (v ?? "").trim().replace(/^[`'"]+|[`'"]+$/g, "");
