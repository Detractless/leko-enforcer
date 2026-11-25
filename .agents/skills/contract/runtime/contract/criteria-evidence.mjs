// Evidence for one acceptance criterion: where it points and whether the pointer holds up.
// Shapes: `path:line` (the line exists and is not empty) or `path "test name"` (the file contains that text).
// With a test command the test name must also show up in the command's output, and the command must exit 0.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const LINE = /^(\S+?):(\d+)$/;
const NAMED = /^(\S+)\s+["'`“](.+?)["'`”]$/;
export const RUN_TIMEOUT_MS = 120000;

/** @returns {{ok:true, file:string, line?:number, name?:string}|{ok:false, why:string}} */
export function parseEvidence(raw) {
  const text = String(raw ?? "").trim().replace(/^`+|`+$/g, "").trim();
  if (!text) return { ok: false, why: "is empty" };
  const named = text.match(NAMED);
  const lined = named ? null : text.match(LINE);
  if (!named && !lined) return { ok: false, why: `is not path:line or path "test name" (${text.slice(0, 40)})` };
  const file = (named || lined)[1];
  if (path.isAbsolute(file) || file.split("/").includes("..")) return { ok: false, why: `must be a path inside the project (${file})` };
  return named ? { ok: true, file, name: named[2] } : { ok: true, file, line: Number(lined[2]) };
}

/** Does the pointer resolve in the files on disk right now? */
export function resolveEvidence(root, raw) {
  const ev = parseEvidence(raw);
  if (!ev.ok) return { ok: false, why: ev.why };
  let text;
  try { text = fs.readFileSync(path.join(root, ev.file), "utf8"); } catch { return { ok: false, why: `${ev.file} does not exist` }; }
  if (ev.line !== undefined) {
    const line = text.split(/\r?\n/)[ev.line - 1];
    return line !== undefined && line.trim() ? { ok: true, ev } : { ok: false, why: `${ev.file} has no code at line ${ev.line}` };
  }
  return text.includes(ev.name) ? { ok: true, ev } : { ok: false, why: `"${ev.name}" is not in ${ev.file}` };
}

/** Runs the project's test command once. @returns {{ran:boolean, passed:boolean, output:string, why?:string}} */
export function runTests(root, command) {
  const r = spawnSync(command, { cwd: root, shell: true, encoding: "utf8", timeout: RUN_TIMEOUT_MS, maxBuffer: 32 * 1024 * 1024 });
  const output = `${r.stdout ?? ""}\n${r.stderr ?? ""}`;
  if (r.error) return { ran: false, passed: false, output, why: r.error.code === "ETIMEDOUT" ? `the test command took longer than ${RUN_TIMEOUT_MS / 1000} s` : r.error.message };
  return { ran: true, passed: r.status === 0, output };
}
