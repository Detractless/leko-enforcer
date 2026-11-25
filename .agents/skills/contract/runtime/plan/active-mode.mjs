// The first question every plan hook asks: is this project plan-first right now?
// Yes only while .agent/plans/ACTIVE.md exists and the global switch <state>/plan-off is absent.
// Anything else: the hook leaves at once, without output and without asking git.

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../project-root.mjs";
import { PLAN_REL } from "./core.mjs";

export const PLAN_OFF_NAME = "plan-off";
/** ACTIVE.md made by /contract new carries this line; only then do new code files also need an open contract. */
export const CONTRACT_MARK = "<!-- contract-required -->";

export const planSwitchedOff = () => fs.existsSync(path.join(stateRoot(), PLAN_OFF_NAME));

/** True when `root` is plan-first and the plan hooks should run. Never throws. */
export function planFirst(root) {
  try { return !planSwitchedOff() && fs.existsSync(path.join(root, PLAN_REL)); } catch { return false; }
}

/** True when the plan text asks for contracts. Older hand-made ACTIVE.md files keep the plain plan rules. */
export const contractsRequired = (planText) => planText.split(/\r?\n/, 12).some((line) => line.trim() === CONTRACT_MARK);
