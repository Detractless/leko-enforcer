// When did a shell command start? The after-command check only judges files the command touched,
// so the before-command hook leaves a stamp and the after-command hook takes it.
// One tiny file per command: <state>/shell-scan/<tool_use_id, else session id>.stamp holding epoch ms.
// A missing stamp (hook added mid-command, parallel commands sharing a session id) gives a 60 s look-back.

import fs from "node:fs";
import path from "node:path";
import { stateRoot } from "../project-root.mjs";

export const FALLBACK_LOOKBACK_MS = 60 * 1000;
const STALE_MS = 60 * 60 * 1000; // a stamp older than this belongs to a command that never finished

const dir = () => path.join(stateRoot(), "shell-scan");
const stampFile = (input) => path.join(dir(), `${String(input.tool_use_id || input.session_id || "none").replace(/[^A-Za-z0-9_-]/g, "_").slice(0, 80)}.stamp`);

export function saveStamp(input, now = Date.now()) {
  fs.mkdirSync(dir(), { recursive: true });
  fs.writeFileSync(stampFile(input), String(now));
  for (const name of fs.readdirSync(dir())) {
    const old = path.join(dir(), name);
    try { if (now - fs.statSync(old).mtimeMs > STALE_MS) fs.rmSync(old); } catch { /* another hook removed it */ }
  }
}

/** Epoch ms when this command started. Removes the stamp; a missing or unreadable one gives now - 60 s. */
export function takeStamp(input, now = Date.now()) {
  const file = stampFile(input);
  try {
    const ms = Number(fs.readFileSync(file, "utf8"));
    fs.rmSync(file, { force: true });
    if (Number.isFinite(ms) && ms > 0) return ms;
  } catch { /* no stamp */ }
  return now - FALLBACK_LOOKBACK_MS;
}
