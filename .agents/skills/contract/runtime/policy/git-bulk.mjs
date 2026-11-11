// Bulk git commands (clean, reset --hard, stash, forced checkout/switch) name no
// path, so the path check in shellwrites.mjs cannot see which file they hit.
// bulkGitKind() recognises them from the command words; bulkHit() asks git
// whether a locked file would really be affected. Only these commands cost a git
// call. BEST EFFORT, like the rest of the Bash check: aliases and scripts that
// run git inside are not seen, and a branch switch that rewrites an unchanged
// locked file is not caught (the end-of-turn audit reports it afterwards).

import { spawnSync } from "node:child_process";
import path from "node:path";
import { realish } from "./paths.mjs";

const NOT_DESTRUCTIVE_STASH = new Set(["list", "show", "pop", "apply", "drop", "clear", "branch", "create", "store"]);
const letter = (flags, chars) => flags.some((f) => /^-[^-]/.test(f) && [...f.slice(1)].some((c) => chars.includes(c)));
const long = (flags, ...names) => flags.some((f) => names.includes(f));

/** `sub` is the git subcommand, `rest` the words after it (text only). -> null or what the command can hit. */
export function bulkGitKind(sub, rest) {
  const flags = rest.filter((w) => w.startsWith("-") && w !== "-" && w !== "--");
  const words = rest.filter((w, i) => !w.startsWith("-") && rest[i - 1] !== "-e" && rest[i - 1] !== "--exclude");
  const pathScoped = rest.includes("--") || (sub === "clean" && words.length > 0);
  if (sub === "clean") {
    if (long(flags, "--dry-run") || letter(flags, "n") || pathScoped) return null;
    const only = letter(flags, "X");
    return { label: "git clean", tracked: false, untracked: !only, ignored: only || letter(flags, "x") };
  }
  if (sub === "reset") {
    return long(flags, "--hard", "--merge") ? { label: `git reset ${flags.find((f) => f === "--hard" || f === "--merge")}`, tracked: true, untracked: false, ignored: false } : null;
  }
  if (sub === "stash") {
    if (NOT_DESTRUCTIVE_STASH.has(words[0])) return null;
    const all = long(flags, "--all") || letter(flags, "a");
    return { label: "git stash", tracked: true, untracked: all || long(flags, "--include-untracked") || letter(flags, "u"), ignored: all };
  }
  if ((sub === "checkout" || sub === "switch") && !pathScoped && (long(flags, "--force", "--discard-changes") || letter(flags, "f"))) {
    return { label: `git ${sub} --force`, tracked: true, untracked: false, ignored: false };
  }
  return null;
}

function git(dir, args) {
  const r = spawnSync("git", ["-C", dir, ...args], { encoding: "utf8", timeout: 5000, maxBuffer: 1 << 24 });
  return { error: r.error, status: r.status, out: r.stdout || "" };
}

/** Porcelain -z entries: "XY path\0", and a renamed or copied entry is followed by its old path. */
function statusEntries(out) {
  const parts = out.split("\0").filter(Boolean);
  const entries = [];
  for (let i = 0; i < parts.length; i++) {
    entries.push({ code: parts[i].slice(0, 2), rel: parts[i].slice(3) });
    if (/[RC]/.test(parts[i][0])) i++;
  }
  return entries;
}

/**
 * bulk: [{ label, tracked, untracked, ignored, cwds }]; locked: [{ rel, abs, entry }].
 * -> { hit: { entry, label, code } | null, failed: string | null }
 */
export function bulkHit(bulk, locked) {
  let failed = null;
  for (const b of bulk) {
    for (const cwd of new Set(b.cwds)) {
      const top = git(cwd, ["rev-parse", "--show-toplevel"]);
      if (top.error) { failed = `${b.label} could not be checked against the locked files (${top.error.code || top.error.message})`; continue; }
      if (top.status !== 0) continue;
      const base = realish(top.out.trim());
      const inside = locked.map((l) => ({ ...l, inRepo: path.relative(base, realish(l.abs)).split(path.sep).join("/") })).filter((l) => !l.inRepo.startsWith("../") && l.inRepo !== "");
      if (!inside.length) continue;
      const st = git(base, ["status", "--porcelain", "-z", "--ignored", "--untracked-files=all", "--", ...inside.map((l) => l.inRepo)]);
      if (st.error || st.status !== 0) { failed = `${b.label} could not be checked against the locked files (git status failed)`; continue; }
      for (const e of statusEntries(st.out)) {
        const kind = e.code === "??" ? b.untracked : e.code === "!!" ? b.ignored : b.tracked;
        const match = kind && inside.find((l) => l.inRepo === e.rel);
        if (match) return { hit: { entry: match.entry, rel: match.rel, label: b.label, code: e.code }, failed: null };
      }
    }
  }
  return { hit: null, failed };
}

const WHY = { "??": "it was never saved in git", "!!": "it is git-ignored" };

export function bulkDenyText(hit, lockctlCmd, coverage) {
  return [
    `policy-guard BLOCKED Bash: \`${hit.label}\` would remove or overwrite a locked file (the command names no path, so git was asked). Nothing was run.`,
    `file: ${hit.rel}`,
    `why: ${WHY[hit.code] || "it has changes that are not saved in git"}`,
    `reason: ${hit.entry.reason || "(none recorded)"}`,
    `request: run the command on named paths that are not locked, or ask the user to authorize an unlock of ${hit.rel} for a stated purpose, then run: ${lockctlCmd} unlock ${hit.rel} --authorized-by "<the user's words>" --purpose "<purpose>"`,
    coverage,
  ].join("\n");
}
