// The plan-gate decision for ONE new gated file: reads ACTIVE.md and the registry and returns
// the problems (empty = allowed). Wording names the file, the field and the fix.

import fs from "node:fs";
import path from "node:path";
import { PLAN_REL, REGISTRY_REL } from "../core.mjs";
import { SLUG_RE, cleanSlug, findEntry, weakness } from "./entry.mjs";
import { ownersOf, readRegistry } from "./registry.mjs";
import { namingProblems } from "../naming/naming.mjs";
import { verifySearch } from "./search.mjs";
import { freshFiles } from "../scan/untracked-files.mjs";
import { fixEntry } from "../scan/fix-block.mjs";
import { contractsRequired } from "../active-mode.mjs";
import { contractProblems } from "../../contract/contract-gate.mjs";

/**
 * The "found nothing" claim is judged against the code that existed when the file is created, never against the
 * model's own later work: hits in files the working tree gained since the last commit are dropped (plan-scan passes
 * that set as `opts.ignoreHits`; the gate looks it up with git, only when a contradiction is possible), and
 * `opts.skipSearch` (the file is already registered, so it was judged at its creation) skips the re-run entirely, and the
 * contract check too (a contract closed since then must not condemn files written while it was open).
 * When ACTIVE.md was made by /contract new (it carries the contract marker), the entry also needs a `Criterion:` line and the
 * feature needs an open contract; `contract` tells the fix example to show that line.
 * @returns {{problems:string[], example:boolean, slug:string|null, contract:boolean}} `example` is true when the 6-line example should be shown.
 */
export function evaluateCreation(root, rel, opts = {}) {
  let text;
  try {
    text = fs.readFileSync(path.join(root, PLAN_REL), "utf8");
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
    return { problems: [`${PLAN_REL} does not exist in ${root}. Create it with a first line "# Active plan" and add an entry for ${rel}.`, ...namingProblems(root, rel)], example: true, slug: null, contract: false };
  }
  const contract = contractsRequired(text);
  const entry = findEntry(text, rel);
  if (!entry.found) {
    const near = entry.nearMiss ? ` Found "${entry.nearMiss}", which does not match: the heading must be exactly "### ${rel}" (one path, no backticks, no ./ prefix, no extra text).` : "";
    return { problems: [`${PLAN_REL} has no entry for ${rel}.${near}`, ...namingProblems(root, rel)], example: true, slug: null, contract };
  }

  const f = entry.fields;
  const problems = [];
  const need = (label, key) => {
    if (f.has(key)) return f.get(key);
    problems.push(`${label}: field is missing from the entry for ${rel}.`);
    return null;
  };
  const check = (label, value, opts) => {
    const why = value === null ? null : weakness(value, opts);
    if (why) problems.push(`${label}: ${why}.`);
    return value !== null && !why;
  };

  const purpose = need("Purpose", "purpose");
  check("Purpose", purpose, { minChars: 25, minWords: 5 });

  const featureRaw = need("Feature", "feature");
  const slug = featureRaw === null ? null : cleanSlug(featureRaw);
  const slugOk = slug !== null && SLUG_RE.test(slug) && slug.length <= 60;
  if (slug !== null && !slugOk) problems.push(`Feature: "${slug.slice(0, 40)}" is not a kebab-case slug (lower-case words joined by hyphens, for example task-export).`);

  const searched = need("Existing code checked", "existing code checked");
  if (check("Existing code checked", searched, { minChars: 25, minWords: 5 }) && !opts.skipSearch) {
    problems.push(...verifySearch(searched, root, opts.ignoreHits ?? (() => freshFiles(root))));
  }

  const failure = need("Failure paths", "failure paths");
  check("Failure paths", failure, { minChars: 25 });

  if (contract && slugOk && !opts.skipSearch) problems.push(...contractProblems(root, slug, f.get("criterion")));

  const shape = problems.length > 0;
  problems.push(...namingProblems(root, rel, slugOk ? slug : null));
  if (slugOk) problems.push(...registryProblems(root, rel, slug, f.get("split justification")));
  return { problems, example: shape, slug: slugOk ? slug : null, contract };
}

function registryProblems(root, rel, slug, split) {
  const reg = readRegistry(root);
  if (reg.status === "corrupt") {
    return [`${REGISTRY_REL}: not usable (${reg.why}). It must be a JSON object mapping a feature slug to an array of file paths, for example {"task-export": ["src/export/csv.js"]}; repair it (or reset it to {}) and retry.`];
  }
  const all = ownersOf(reg.data, slug);
  if (all.some((o) => o.files.includes(rel))) return [];
  const owners = all.filter((o) => o.files.length);
  if (!owners.length) return [];
  const listed = owners.map((o) => o.files.map((p) => `${p}${fs.existsSync(path.join(root, p)) ? "" : " (missing on disk)"}`).join(", ") + (o.key === slug ? "" : ` [registered as "${o.key}"]`)).join("; ");
  const why = split === undefined ? null : weakness(split, { minChars: 25 });
  if (split !== undefined && !why) return [];
  const state = split === undefined ? "" : ` The Split justification is ${why}.`;
  return [`Feature: "${slug}" already exists in ${REGISTRY_REL}, implemented by: ${listed}.${state} Extend those files instead of adding another, or add a "Split justification:" line (>= 25 characters) to the entry explaining why this file is a separate responsibility. If a listed file no longer exists, remove it from ${REGISTRY_REL}.`];
}

/** The full denial text. */
export function formatDenial(root, rel, { problems, example, contract }, notes = []) {
  const lines = [`plan-gate: blocked creating ${rel}. New code files need an entry in ${path.join(root, PLAN_REL)}.`, ...notes, "Problems:", ...problems.map((p) => `- ${p}`)];
  if (example) lines.push("", "Add or fix this entry, replacing each <...> part with your own text, then retry the write:", fixEntry(rel, contract));
  return lines.join("\n");
}
