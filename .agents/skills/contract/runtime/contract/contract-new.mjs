// /contract new <feature>: the contract file from the template, its state file, and (once per project) the plan files.
// Refuses the home folder, a name already used and a bad name. Says exactly what it made.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { CONTRACT_MARK } from "../plan/active-mode.mjs";
import { PLAN_REL, REGISTRY_REL } from "../plan/core.mjs";
import { CONTRACTS_REL, PLANS_REL, contractRel, featureError, stateRel, today, writeAtomic, writeState } from "./contract-state.mjs";

export const TEMPLATES_DIR = path.join(import.meta.dirname, "..", "..", "skills", "contract", "templates");
const refuse = (...err) => ({ code: 1, out: [], err });

function homeFolder(root) {
  const home = os.homedir();
  const claudeDir = path.join(home, ".claude");
  return root === home || root === claudeDir || root.startsWith(claudeDir + path.sep);
}

/** Puts the contract marker on the second line of an ACTIVE.md that lacks it. @returns {boolean} true when it changed the file. */
function addMarker(file) {
  const text = fs.readFileSync(file, "utf8");
  if (text.split(/\r?\n/, 12).some((l) => l.trim() === CONTRACT_MARK)) return false;
  const [first, ...rest] = text.split("\n");
  writeAtomic(file, [first, CONTRACT_MARK, ...rest].join("\n"));
  return true;
}

export function newContract(root, feature, templates = TEMPLATES_DIR) {
  const bad = featureError(feature);
  if (bad) return refuse(bad);
  if (homeFolder(root)) return refuse(`refusing to start a contract in ${root}: that is not a project folder (run it inside the project, or pass --root <dir>).`);
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) return refuse(`${root} is not a folder.`);
  if (fs.existsSync(path.join(root, contractRel(feature))) || fs.existsSync(path.join(root, stateRel(feature)))) {
    return refuse(`${contractRel(feature)} already exists, so nothing was made. Pick another name or continue that contract.`);
  }
  const made = [];
  const read = (name) => fs.readFileSync(path.join(templates, name), "utf8");
  writeAtomic(path.join(root, contractRel(feature)), read("CONTRACT.md").replace("<feature>", feature));
  made.push(contractRel(feature));
  writeState(root, feature, { status: "draft", created: today() });
  made.push(stateRel(feature));
  const plan = path.join(root, PLAN_REL);
  if (!fs.existsSync(plan)) { writeAtomic(plan, read("ACTIVE.md")); made.push(`${PLAN_REL} (plan-first is now on)`); }
  else if (addMarker(plan)) made.push(`${PLAN_REL} (added the line "${CONTRACT_MARK}": new code files now need an open contract)`);
  const template = path.join(root, PLANS_REL, "TEMPLATE.md");
  if (!fs.existsSync(template)) { writeAtomic(template, read("TEMPLATE.md")); made.push(`${PLANS_REL}/TEMPLATE.md`); }
  const registry = path.join(root, REGISTRY_REL);
  if (!fs.existsSync(registry)) { writeAtomic(registry, "{}\n"); made.push(REGISTRY_REL); }
  return { code: 0, out: [`Made in ${root}:`, ...made.map((m) => `  ${m}`), `Next: fill in ${contractRel(feature)}; nothing is built until the user types /contract open ${feature}.`, `(${CONTRACTS_REL}/ is saved with the project; nothing else was touched.)`], err: [] };
}
