// PostToolUse registry update (matcher: Write|Edit|MultiEdit).
//
// After a write that left a code file on disk, if ACTIVE.md has a valid "### <path>" entry with a
// kebab-case Feature slug for that file, add the path to .agent/features.json under the slug.
// Idempotent (a path already listed is left alone) and atomic (temp file + rename). It never
// blocks: a corrupt registry is left untouched and reported on one systemMessage line; any
// unexpected error is logged to $CLAUDE_STATE_DIR/plan-gate/errors.log and shown as one line.

import fs from "node:fs";
import path from "node:path";
import { readInput, systemMessage } from "./policy/hookio.mjs";
import { CODE_EXT, FILE_TOOLS, PLAN_REL, REGISTRY_REL, findProjectRoot, logError, runHook, toPosix } from "./plan/core.mjs";
import { SLUG_RE, cleanSlug, findEntry } from "./plan/entries/entry.mjs";
import { addToRegistry } from "./plan/entries/registry.mjs";
import { planFirst } from "./plan/active-mode.mjs";

runHook("plan-registry", () => {
  const input = readInput();
  if (!input || typeof input !== "object") return; // nothing to register; the gate already reports bad input
  const ti = input.tool_input;
  if (!FILE_TOOLS.has(input.tool_name) || !ti || typeof ti.file_path !== "string" || !ti.file_path) return;

  const cwd = typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd();
  const abs = path.resolve(cwd, ti.file_path);
  if (!CODE_EXT.has(path.extname(abs).toLowerCase()) || !fs.existsSync(abs) || !fs.statSync(abs).isFile()) return;

  const root = findProjectRoot(abs, cwd);
  const rel = toPosix(path.relative(root, abs));
  if (rel === ".." || rel.startsWith("../") || !planFirst(root)) return;

  let plan;
  try { plan = fs.readFileSync(path.join(root, PLAN_REL), "utf8"); } catch (e) { if (e.code === "ENOENT") return; throw e; }
  const entry = findEntry(plan, rel);
  const slug = entry.found ? cleanSlug(entry.fields.get("feature")) : "";
  if (!SLUG_RE.test(slug)) return;

  const out = addToRegistry(root, slug, rel);
  if (out.result === "corrupt") {
    logError("plan-registry", `${REGISTRY_REL} not updated for ${rel}: ${out.why}`);
    systemMessage(`plan-registry: ${REGISTRY_REL} is unusable (${out.why}); ${rel} was not registered under "${slug}".`);
  }
});
