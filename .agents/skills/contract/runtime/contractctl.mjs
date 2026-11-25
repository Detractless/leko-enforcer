#!/usr/bin/env node
// contractctl: the only supported way to make, open, check and close a feature contract.
//
//   contractctl new <feature>
//   contractctl open <feature> --authorized-by "<the user's words>"
//   contractctl verify <feature> [--run "<test command>"]
//   contractctl digest <feature>          what a fresh judge reads (for /contract verify --judge)
//   contractctl done <feature> [--run "<test command>"]
//   contractctl drop [<feature>] --reason "<why>"     no name: archive a stale ACTIVE.md
//   contractctl status
//   (any command) [--root <dir>]          default: the nearest parent holding .git, else the current folder
//
// Exit codes: 0 done, 1 refused or evidence missing, 2 usage error.

import path from "node:path";
import { findProjectRoot } from "./plan/core.mjs";
import { doneContract, dropContract, openContract } from "./contract/contract-close.mjs";
import { newContract } from "./contract/contract-new.mjs";
import { featureError } from "./contract/contract-state.mjs";
import { formatDigest, formatStatus, formatVerify, verifyContract } from "./contract/verify-report.mjs";

const USAGE = `usage:
  contractctl new <feature>
  contractctl open <feature> --authorized-by "<the user's words>"
  contractctl verify <feature> [--run "<test command>"]
  contractctl digest <feature>
  contractctl done <feature> [--run "<test command>"]
  contractctl drop [<feature>] --reason "<why>"
  contractctl status
  options: --root <dir>`;

function parse(argv) {
  const positional = [];
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith("--")) { positional.push(a); continue; }
    const key = a.slice(2);
    const next = argv[i + 1];
    if (next === undefined || next.startsWith("--")) opts[key] = true;
    else { opts[key] = next; i++; }
  }
  return { positional, opts };
}

function finish(code, out = [], err = []) {
  if (out.length) process.stdout.write(`${out.join("\n")}\n`);
  if (err.length) process.stderr.write(`${err.join("\n")}\n`);
  process.exit(code);
}

const { positional, opts } = parse(process.argv.slice(2));
const [command, feature] = positional;
if (!command || command === "help" || opts.help) finish(command ? 0 : 2, [], [USAGE]);
const cwd = process.cwd();
const root = typeof opts.root === "string" ? path.resolve(cwd, opts.root) : findProjectRoot(path.join(cwd, "scan"), cwd);
const str = (v) => (typeof v === "string" ? v : undefined);
if (["new", "open", "verify", "digest", "done"].includes(command)) {
  const bad = feature ? featureError(feature) : "missing <feature>";
  if (bad) finish(2, [], [bad]);
}
try {
  let r;
  if (command === "new") r = newContract(root, feature);
  else if (command === "open") r = openContract(root, feature, str(opts["authorized-by"]));
  else if (command === "done") r = doneContract(root, feature, str(opts.run));
  else if (command === "drop") r = dropContract(root, feature, str(opts.reason));
  else if (command === "verify") { const v = verifyContract(root, feature, { run: str(opts.run) }); r = { code: v.clean ? 0 : 1, out: [formatVerify(v)], err: [] }; }
  else if (command === "digest") r = { code: 0, out: [formatDigest(root, feature)], err: [] };
  else if (command === "status") r = { code: 0, out: [formatStatus(root)], err: [] };
  else finish(2, [], [`unknown command "${command}"`, USAGE]);
  finish(r.code, r.out, r.err);
} catch (err) {
  finish(1, [], [`contractctl ${command} failed: ${err.message}`]);
}
