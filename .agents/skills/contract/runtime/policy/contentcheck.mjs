// Per-file checks on one file's complete content: the built-in line cap (linecap.mjs, every code
// file, not configurable), the configurable character limit, and JavaScript function metrics.
//
// mode "preflight": `prev` is the file as it is on disk (null = new file). A
//   violation blocks unless the file was already at least as bad before this
//   change (shrinking or holding an oversized file stays possible; that case
//   becomes a visible notice instead).
// mode "audit": the file is judged as it is; `prev` is ignored.
//
// The line cap applies to every code file. The character limit and the structure checks apply only
// to files inside the policy scope (default src/ and bin/). A file containing a NUL byte is treated
// as binary and skipped.

import path from "node:path";
import { countChars } from "./lines.mjs";
import { checkLineCap } from "./linecap.mjs";
import { measureFunctions } from "./metrics.mjs";
import { JS_EXT, findExemption, inScope, mergeOutcome, newOutcome, reasonFor, tokenizeCached } from "./common.mjs";

const STRUCT_FIELDS = {
  maxFunctionLines: ["lines", "lines"],
  maxNesting: ["nesting", "levels"],
  maxBranches: ["branches", "branches"],
};

function judge(policy, out, mode, f, measured, limit, before) {
  if (measured <= limit) return;
  const finding = { check: f.check, file: f.file, key: f.key, measure: f.measure, limit: `${f.check} = ${limit}`, reason: reasonFor(policy, f.check) };
  const ex = findExemption(policy, f.file, f.check);
  if (ex) out.exempted.push({ ...finding, exemption: ex });
  else if (mode === "preflight" && before !== null && measured <= before) {
    out.notices.push({ kind: "over-limit", file: f.file, text: `${f.file} is still over ${f.check} (${measured} > ${limit}) but this change does not increase it, so it is allowed` });
  } else out.findings.push(finding);
}

function keyedFunctions(tokens) {
  const seen = new Map();
  return new Map(measureFunctions(tokens).map((fn) => {
    const n = (seen.get(fn.name) || 0) + 1;
    seen.set(fn.name, n);
    return [`${fn.name}#${n}`, fn];
  }));
}

function structureChecks(policy, out, mode, rel, content, prev) {
  const metricKeys = Object.keys(STRUCT_FIELDS).filter((k) => k in policy.cfg);
  const graphKeys = [policy.cfg.forbidCycles === true && "forbidCycles", policy.cfg.layers && "layers"].filter(Boolean);
  const keys = [...metricKeys, ...graphKeys];
  if (!keys.length) return;
  const ext = path.extname(rel).toLowerCase();
  if (!JS_EXT.has(ext)) {
    out.notices.push({ kind: "unsupported", file: rel, text: `${rel}: ${keys.join(", ")} not applied - unsupported language ${ext ? `"${ext}"` : "(no extension)"}; structure checks support JavaScript (.js .mjs .cjs) only` });
    return;
  }
  const cur = tokenizeCached(content);
  if (cur.error) {
    out.notices.push({ kind: "parser-failure", file: rel, text: `${rel}: ${keys.join(", ")} not applied - parser failure: ${cur.error.message} at line ${cur.error.line}` });
    return;
  }
  if (!metricKeys.length) return;
  const old = mode === "preflight" && prev !== null ? tokenizeCached(prev) : null;
  const before = old && !old.error ? keyedFunctions(old.tokens) : new Map();
  for (const [fkey, fn] of keyedFunctions(cur.tokens)) {
    for (const k of metricKeys) {
      const [field, unit] = STRUCT_FIELDS[k];
      const measure = `function ${fn.name} (lines ${fn.startLine}-${fn.endLine}) has ${fn[field]} ${unit}`;
      const was = before.get(fkey);
      judge(policy, out, mode, { check: k, file: rel, key: `${k}:${rel}:${fkey}`, measure }, fn[field], policy.cfg[k], was ? was[field] : null);
    }
  }
}

export function checkContent(policy, mode, rel, content, prev) {
  const out = newOutcome();
  if (content.includes("\0")) return out;
  mergeOutcome(out, checkLineCap(mode, rel, content, prev));
  if (!inScope(policy, rel)) return out;
  if ("fileCharLimit" in policy.cfg) {
    const subject = mode === "preflight" ? "in the proposed content" : "in the current file";
    const before = mode === "preflight" && prev !== null ? countChars(prev) : null;
    judge(policy, out, mode, { check: "fileCharLimit", file: rel, key: `fileCharLimit:${rel}`, measure: `${countChars(content)} characters ${subject}` }, countChars(content), policy.cfg.fileCharLimit, before);
  }
  structureChecks(policy, out, mode, rel, content, prev);
  return out;
}
