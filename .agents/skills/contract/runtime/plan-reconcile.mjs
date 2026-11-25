// Stop hook: repair-only plan bookkeeping (see plan/entries/reconcile.mjs). Retires the ACTIVE.md entries and drops the
// features.json paths of files that were deleted or moved. It never blocks, never prints anything to the model
// (no stdout at all), and exits 0 in every case; a failure is only written to $CLAUDE_STATE_DIR/plan-gate/errors.log.
// Silent and read-only when the project has no .agent/ folder or nothing is stale.

import path from "node:path";
import { readInput } from "./policy/hookio.mjs";
import { findProjectRoot, logError } from "./plan/core.mjs";
import { reconcile } from "./plan/entries/reconcile.mjs";
import { planFirst } from "./plan/active-mode.mjs";

try {
  const input = readInput();
  const cwd = input && typeof input.cwd === "string" && input.cwd ? input.cwd : process.cwd();
  const root = findProjectRoot(path.join(cwd, "scan"), cwd); // a probe path inside cwd: the root search starts at cwd itself
  if (planFirst(root)) reconcile(root);
} catch (err) {
  logError("plan-reconcile", err);
}
process.exitCode = 0;
