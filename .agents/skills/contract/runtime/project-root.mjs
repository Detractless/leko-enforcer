// Shared basics: where the harness state lives (stateRoot), a chat's stable
// key (chatKeyFromTranscript), and adding a project to projects.json
// (registerProject, for /organize new). The old resolveProject lookup and its
// command line had no callers and were removed 2026-10-02 (Field Guide 28).

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export function stateRoot() {
  return process.env.CLAUDE_STATE_DIR || path.join(os.homedir(), ".claude", "state");
}

function readJson(file) {
  try { return JSON.parse(fs.readFileSync(file, "utf8")); } catch { return {}; }
}

// The chat key is the ID of the chat's first real message from you. It stays
// the same when the app forks a chat (6.1). Reads only as far as that line.
export function chatKeyFromTranscript(transcriptPath) {
  let fd;
  try { fd = fs.openSync(transcriptPath, "r"); } catch { return null; }
  try {
    const buf = Buffer.alloc(64 * 1024);
    let rest = "";
    for (;;) {
      const n = fs.readSync(fd, buf, 0, buf.length, null);
      if (n === 0) return null;
      const lines = (rest + buf.toString("utf8", 0, n)).split("\n");
      rest = lines.pop();
      for (const line of lines) {
        let d;
        try { d = JSON.parse(line); } catch { continue; }
        if (d.type !== "user" || d.isMeta) continue;
        let c = d.message?.content;
        if (Array.isArray(c)) {
          if (c.some((x) => x.type === "tool_result")) continue;
          c = c.filter((x) => x.type === "text").map((x) => x.text || "").join(" ");
        }
        if (c && d.uuid) return d.uuid;
      }
    }
  } finally {
    fs.closeSync(fd);
  }
}

// Rule 6: a project.json found on disk but missing from projects.json gets
// added. Never replaces an existing entry. Temp file, then rename.
export function registerProject(marker) {
  const file = path.join(stateRoot(), "projects.json");
  const all = readJson(file);
  if (all[marker.name]) return false;
  const { name, ...rest } = marker.json;
  all[marker.name] = { folder: marker.root, ...rest };
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(all, null, 2) + "\n", "utf8");
  fs.renameSync(tmp, file);
  return true;
}
