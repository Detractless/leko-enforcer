// Load and validate <project>/.claude/project-policy.json. The 350-line cap is NOT read from it (see
// linecap.mjs); `scopePaths` scopes only the checks configured here.
//
// Outcomes (`state`):
//   absent        no policy file: nothing is configured
//   unreadable    file exists but cannot be read
//   invalid-json  not parseable JSON, or not a JSON object: no check applies
//   ok            parsed; each key is validated on its own. `problems` lists
//                 keys with invalid values; those checks are NOT applied, the
//                 rest are.
//
// A key that is absent is "not configured": it never gets a default and is
// never reported as passed. Keys are read from the top level; the structure
// keys (maxFunctionLines, maxNesting, maxBranches, forbidCycles, layers) may
// instead be grouped in a "structure" object. Setting one key in both places
// is a problem and neither copy is applied.

import fs from "node:fs";
import path from "node:path";
import { FILE_LINE_CAP } from "./linecap.mjs";
import { POLICY_REL } from "./paths.mjs";

export const INT_KEYS = ["fileCharLimit", "maxFunctionLines", "maxNesting", "maxBranches"];
export const STRUCTURE_KEYS = ["maxFunctionLines", "maxNesting", "maxBranches", "forbidCycles", "layers"];
export const CHECKS = [...INT_KEYS, "forbidCycles", "layers"];
export const DEFAULT_SCOPE = ["src/", "bin/"];

// The line cap is built in (linecap.mjs). These keys used to configure it; they are read, reported as
// ignored, and never applied.
const RETIRED_KEYS = ["fileLineLimit", "fileLineLimitRule"];

const DOC_KEYS = ["_about", "lockRecordFormat", "lockRules", "scopePaths", "lockRecords", "exemptions", "structure"];
const KNOWN = new Set([...CHECKS, ...CHECKS.map((k) => `${k}Rule`), ...DOC_KEYS, ...RETIRED_KEYS]);

const isInt = (v) => Number.isInteger(v) && v >= 1;
const isText = (v) => typeof v === "string" && v.trim() !== "";

function validateValue(key, v) {
  if (INT_KEYS.includes(key)) return isInt(v) ? null : "must be an integer >= 1";
  if (key === "forbidCycles") return typeof v === "boolean" ? null : "must be true or false";
  if (key === "layers") {
    const ok = Array.isArray(v) && v.length >= 2 && v.every(isText) && new Set(v).size === v.length;
    return ok ? null : "must be an array of at least 2 distinct non-empty path prefixes";
  }
  return null;
}

function validateExemptions(list, problems) {
  if (!Array.isArray(list)) { problems.push("exemptions: must be an array"); return []; }
  const kept = [];
  list.forEach((e, i) => {
    const bad = !e || typeof e !== "object" ? "must be an object"
      : !isText(e.path) ? "path must be a non-empty glob"
      : !isText(e.reason) ? "reason must be non-empty text"
      : e.checks !== undefined && !(Array.isArray(e.checks) && e.checks.length && e.checks.every((c) => CHECKS.includes(c))) ? `checks must list names from ${CHECKS.join(", ")}`
      : null;
    if (bad) problems.push(`exemptions[${i}]: ${bad} (entry ignored)`);
    else kept.push({ path: e.path, reason: e.reason, checks: e.checks || null });
  });
  return kept;
}

function validateRecordsPath(v) {
  if (!isText(v)) return "must be a non-empty relative path";
  if (path.isAbsolute(v) || path.normalize(v).startsWith("..")) return "must stay inside the project";
  return null;
}

export function loadPolicy(root) {
  const file = path.join(root, POLICY_REL);
  const out = { state: "ok", root, file, problems: [], warnings: [], cfg: {}, rules: {}, exemptions: [], lockRecords: null, scope: DEFAULT_SCOPE, scopeIsDefault: true };
  let raw;
  try {
    raw = fs.readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  } catch (e) {
    out.state = e.code === "ENOENT" ? "absent" : "unreadable";
    if (out.state === "unreadable") out.problems.push(`cannot read policy: ${e.code || e.message}`);
    return out;
  }
  let json;
  try {
    json = JSON.parse(raw);
  } catch (e) {
    return { ...out, state: "invalid-json", problems: [`not valid JSON: ${e.message}`] };
  }
  if (!json || typeof json !== "object" || Array.isArray(json)) {
    return { ...out, state: "invalid-json", problems: ["top level must be a JSON object"] };
  }
  const grouped = json.structure;
  if (grouped !== undefined && (!grouped || typeof grouped !== "object" || Array.isArray(grouped))) {
    out.problems.push("structure: must be an object (grouped structure keys ignored)");
  }
  const groupedObj = grouped && typeof grouped === "object" && !Array.isArray(grouped) ? grouped : {};
  for (const k of Object.keys(groupedObj)) if (!STRUCTURE_KEYS.includes(k) && !k.endsWith("Rule")) out.warnings.push(`structure.${k}: unrecognized key`);
  for (const key of CHECKS) {
    const top = key in json;
    const nested = STRUCTURE_KEYS.includes(key) && key in groupedObj;
    if (top && nested) { out.problems.push(`${key}: set both at top level and in "structure"; not applied`); continue; }
    if (!top && !nested) continue;
    const value = top ? json[key] : groupedObj[key];
    const err = validateValue(key, value);
    if (err) out.problems.push(`${key}: ${err} (got ${JSON.stringify(value)}); not applied`);
    else out.cfg[key] = value;
    const rule = top ? json[`${key}Rule`] : groupedObj[`${key}Rule`];
    if (isText(rule)) out.rules[key] = rule;
  }
  for (const k of RETIRED_KEYS) if (k in json) out.warnings.push(`${k}: ignored - the ${FILE_LINE_CAP}-line cap is built in and cannot be configured`);
  for (const k of Object.keys(json)) if (!KNOWN.has(k) && !k.startsWith("_")) out.warnings.push(`${k}: unrecognized key (ignored)`);
  if ("exemptions" in json) out.exemptions = validateExemptions(json.exemptions, out.problems);
  if ("lockRecords" in json) {
    const err = validateRecordsPath(json.lockRecords);
    if (err) out.problems.push(`lockRecords: ${err}; lock checks not applied`);
    else out.lockRecords = path.posix.normalize(json.lockRecords.split(path.sep).join("/"));
  }
  if ("scopePaths" in json) {
    const ok = Array.isArray(json.scopePaths) && json.scopePaths.length > 0 && json.scopePaths.every(isText);
    if (ok) { out.scope = json.scopePaths; out.scopeIsDefault = false; } else out.problems.push("scopePaths: must be a non-empty array of path prefixes; using src/ and bin/");
  }
  return out;
}

/** Per-check status for reports: configured, not configured, or not applied (invalid). */
export function describeChecks(policy) {
  return CHECKS.map((check) => {
    if (check in policy.cfg) return { check, status: "configured", value: policy.cfg[check] };
    const bad = policy.problems.some((p) => p.startsWith(`${check}:`));
    return { check, status: bad ? "invalid (not applied)" : "not configured" };
  });
}
