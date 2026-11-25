#!/usr/bin/env node
// lockctl: the only supported way to change lock records.
//
//   lockctl lock   <path> --reason "<text>" --evidence "<text>"
//   lockctl unlock <path> --authorized-by "<the user's words>" --purpose "<text>"
//   lockctl relock <path> --reason "<text>" --evidence "<text>" --verify "<command>" [--timeout <seconds>]
//   lockctl status [<path>] [--json]
//   lockctl recover
//   lockctl init                   makes the project policy file (only _about and lockRecords) when no policy exists yet
//   (any command) [--root <dir>]   default: nearest parent with .claude/project-policy.json
//
// Records live where the project policy's `lockRecords` says. Exit codes:
// 0 done / healthy, 1 refused, failed, or problems found, 2 usage or setup error.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { lockCmd, recoverCmd, relockCmd, statusCmd, unlockCmd } from "./policy/lockcmds.mjs";
import { POLICY_REL, findProjectRoot } from "./policy/paths.mjs";
import { loadPolicy } from "./policy/policy.mjs";
import { findProjectRoot as projectFolder } from "./plan/core.mjs";

const USAGE = `usage:
  lockctl lock   <path> --reason "<text>" --evidence "<text>"
  lockctl unlock <path> --authorized-by "<the user's words>" --purpose "<text>"
  lockctl relock <path> --reason "<text>" --evidence "<text>" --verify "<command>" [--timeout <seconds>]
  lockctl status [<path>] [--json]
  lockctl recover
  lockctl init   (makes ${POLICY_REL} if the project has no policy yet)
  options: --root <dir>  (default: nearest parent holding ${POLICY_REL})`;

const POLICY_TEXT = `${JSON.stringify({
  _about: "Project policy. The 350-line cap for every code file (tests included) is built in and is not configurable: no key in this file changes it. Split a longer file by responsibility. Locks are recorded in .claude/locks.json.",
  lockRecords: ".claude/locks.json",
}, null, 2)}\n`;

// init: the one way to start locking in a project. Never overwrites, never nests a second policy, never runs in the home folder.
function initPolicy(cwd, rootOpt) {
  const folder = typeof rootOpt === "string" ? path.resolve(cwd, rootOpt) : projectFolder(path.join(cwd, "scan"), cwd);
  const home = os.homedir();
  const claudeDir = path.join(home, ".claude");
  if (folder === home || folder === claudeDir || folder.startsWith(claudeDir + path.sep)) fail(1, `refusing to make a policy file in ${folder}: that is not a project folder`);
  if (!fs.existsSync(folder) || !fs.statSync(folder).isDirectory()) fail(1, `${folder} is not a folder`);
  const existing = findProjectRoot(folder);
  if (existing) fail(1, `${path.join(existing, POLICY_REL)} already exists, so nothing was made. If it has no "lockRecords" key, the user decides whether to add it.`);
  fs.mkdirSync(path.join(folder, ".claude"), { recursive: true });
  fs.writeFileSync(path.join(folder, POLICY_REL), POLICY_TEXT, { flag: "wx" });
  process.stdout.write(`created ${POLICY_REL} in ${folder}\n`);
}

function parse(argv) {
  const positional = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith("--")) { positional.push(a); continue; }
    const eq = a.indexOf("=");
    if (eq > 0) { opts[a.slice(2, eq)] = a.slice(eq + 1); continue; }
    const key = a.slice(2);
    const next = argv[i + 1];
    if (key === "json" || next === undefined || next.startsWith("--")) opts[key] = true;
    else { opts[key] = next; i++; }
  }
  return { positional, opts };
}

function fail(code, ...lines) {
  process.stderr.write(`${lines.join("\n")}\n`);
  process.exit(code);
}

const { positional, opts } = parse(process.argv.slice(2));
const [command, target] = positional;
if (!command || command === "help" || opts.help) fail(command ? 0 : 2, USAGE);
const cwd = process.cwd();
if (command === "init") { initPolicy(cwd, opts.root); process.exit(0); }
const root = typeof opts.root === "string" ? path.resolve(cwd, opts.root) : findProjectRoot(cwd);
if (!root) fail(2, `no ${POLICY_REL} found from ${cwd}; run inside the project or pass --root <dir>`);
const policy = loadPolicy(root);
if (policy.state !== "ok") fail(2, `policy file is unusable (${policy.problems[0] || policy.state}); lockctl needs a valid policy naming lockRecords`);
if (!policy.lockRecords) fail(2, `lock records are not configured: ${POLICY_REL} has no "lockRecords" key`);
const ctx = { root, cwd, recordsRel: policy.lockRecords, recordsFile: path.join(root, policy.lockRecords) };
const commands = { lock: lockCmd, unlock: unlockCmd, relock: relockCmd, status: statusCmd, recover: recoverCmd };
if (!commands[command]) fail(2, `unknown command "${command}"`, USAGE);
try {
  const result = commands[command](ctx, target, opts);
  if (result.out.length) process.stdout.write(`${result.out.join("\n")}\n`);
  if (result.err.length) process.stderr.write(`${result.err.join("\n")}\n`);
  process.exitCode = result.code;
} catch (err) {
  fail(1, `lockctl ${command} failed: ${err.message}`);
}
