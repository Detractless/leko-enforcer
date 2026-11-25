// Everyday or strict. A project is in strict (refactor) mode while .agent/plans/ACTIVE.md exists;
// without it the everyday rules apply (existing layouts only get warnings). One definition for every hook.

import fs from "node:fs";
import path from "node:path";
import { PLAN_REL } from "../plan/core.mjs";

/** True when `root` holds the active plan file. Never throws. */
export function isStrict(root) {
  try { return fs.existsSync(path.join(root, PLAN_REL)); } catch { return false; }
}
