// Re-runs the search command from an "Existing code checked" field.
//
// The command is parsed into argv and executed WITHOUT a shell, from the project root, with a
// 10 s timeout and a 100 KB output cap. What this proves: a read-only search command ran and
// exited 0 or 1. It does not prove the search was meaningful.

import { spawnSync } from "node:child_process";
import path from "node:path";
import { CODE_EXT } from "../core.mjs";

const TIMEOUT_MS = Number(process.env.PLAN_GATE_SEARCH_TIMEOUT_MS) || 10_000; // env override exists for tests only
const MAX_OUT = 100 * 1024;
const OPERATORS = /[;|&<>$`\r\n]/;
const COMMAND_START = /^(?:grep|rg|find|git\s+grep|git\s+ls-files)(?:\s|$)/;
const FIND_WRITES = /^-(?:delete|exec|execdir|ok|okdir|fprint\w*|fls)$/;
const IGNORED_DIRS = new Set([".agent", ".claude", ".git", "node_modules"]);
const NOTHING = new RegExp(
  "\\b(?:(?:found|returned|yielded|got|gave|shows?|showed|produced|revealed|turned up)\\s+(?:nothing|none|zero|0|no\\s+\\w+)" +
  "|nothing\\s+(?:found|relevant|similar|existing|related|reusable|useful|matching|came|to reuse|turned up|exists)" +
  "|none\\s+(?:found|reusable|exist|exists|relevant|match(?:es|ed)?)" +
  "|no\\s+(?:existing|prior|related|similar|relevant|matching|matches|results?|hits?|implementations?|usages?|occurrences?)" +
  "|not found|came back empty|empty (?:result|output)|(?:zero|0) (?:matches|results|hits)|(?:does not|doesn't) exist)\\b", "i");

/** Shell-like split: quotes group, a backslash escapes the next character; no expansion of any kind. */
export function tokenize(cmd) {
  const out = [];
  let cur = null;
  let quote = null;
  for (let i = 0; i < cmd.length; i++) {
    const ch = cmd[i];
    if (quote) {
      if (ch === quote) quote = null;
      else if (ch === "\\" && quote === '"' && (cmd[i + 1] === '"' || cmd[i + 1] === "\\")) cur += cmd[++i];
      else cur += ch;
    } else if (ch === '"' || ch === "'") { quote = ch; cur ??= ""; }
    else if (ch === "\\" && i + 1 < cmd.length) cur = (cur ?? "") + cmd[++i];
    else if (/\s/.test(ch)) { if (cur !== null) { out.push(cur); cur = null; } }
    else cur = (cur ?? "") + ch;
  }
  if (quote) return { error: "unbalanced quote" };
  if (cur !== null) out.push(cur);
  return { tokens: out };
}

function badFlag(tool, args) {
  return args.find((a) => {
    if (tool === "find") return FIND_WRITES.test(a);
    if (tool === "rg") return /^--(?:pre|pre-glob|hostname-bin)(?:=|$)/.test(a) || a === "--search-zip" || /^-[A-Za-z]*z[A-Za-z]*$/.test(a);
    if (tool === "git") return /^--open-files-in-pager(?:=|$)/.test(a) || /^-[A-Za-z]*O[A-Za-z]*$/.test(a);
    return false;
  });
}

const hitPath = (line) => line.replace(/^\.\//, "").split(":", 1)[0];
const codeFile = (p) => CODE_EXT.has(path.extname(p).toLowerCase());

/**
 * Single-file grep prints no file name ("12:text"), so the hit cannot be classified from its own text. Judge it
 * by the command's last operand: an operand that is clearly not code (".gitignore", "README.md") makes the
 * hits non-code; a code file, a directory or anything unclear counts, as before.
 */
function unlabelledIsCode(args) {
  const last = args.filter((a) => !a.startsWith("-")).pop();
  if (!last || codeFile(last)) return true;
  return !(path.extname(last) || last.startsWith("."));
}

/**
 * Does one output line of the re-run search point into a code file? Only files with a code extension (the
 * gate's CODE_EXT) are code: dotfiles such as .gitignore, Makefile, LICENSE, *.md and *.json never count.
 */
function isCodeHit(line, args) {
  const cand = hitPath(line);
  if (!/^\S+$/.test(cand) || /^\d+$/.test(cand)) return unlabelledIsCode(args);
  if (cand.split("/").some((s) => IGNORED_DIRS.has(s))) return false;
  return codeFile(cand);
}

/**
 * `ignore` (optional): hits in these project-relative paths are not counted by the contradiction check. A Set,
 * or a function returning a Set or null; the function is called only when code files matched an entry that claims nothing.
 * @returns {string[]} problems (empty when the field is acceptable)
 */
export function verifySearch(value, root, ignore = null) {
  const spans = [...value.matchAll(/`([^`]+)`/g)].map((m) => m[1].trim());
  if (!spans.length) return ["Existing code checked: must contain the actual search command in backticks, for example `grep -rn -e export src`. Prose alone is not evidence; the hook re-runs the command."];
  const cmd = spans.find((s) => COMMAND_START.test(s)) ?? spans[0];
  const shown = `\`${cmd}\``;
  if (OPERATORS.test(cmd)) return [`Existing code checked: ${shown} contains a shell operator (one of ; | & > < $ backtick). Use ONE plain command; for alternatives repeat -e PATTERN instead of using |.`];
  if (!COMMAND_START.test(cmd)) return [`Existing code checked: ${shown} is not an allowed search. Use one plain grep, rg, find, git grep or git ls-files command; listings such as ls are not evidence.`];
  const parsed = tokenize(cmd);
  if (parsed.error) return [`Existing code checked: ${shown} has an ${parsed.error}.`];
  const [tool, ...args] = parsed.tokens;
  const flag = badFlag(tool, args);
  if (flag) return [`Existing code checked: ${shown} uses ${flag}, which can write, delete or execute. The hook re-runs the command, so it must be purely read-only.`];

  const res = spawnSync(tool, args, {
    cwd: root, shell: false, encoding: "utf8", timeout: TIMEOUT_MS, maxBuffer: MAX_OUT,
    stdio: ["ignore", "pipe", "pipe"], env: { ...process.env, GIT_PAGER: "cat", GIT_OPTIONAL_LOCKS: "0", RIPGREP_CONFIG_PATH: "" },
  });
  const code = res.error?.code;
  if (code === "ENOENT") return [`Existing code checked: ${shown} cannot run here (${tool} is not installed or ${root} is not usable).`];
  if (code === "ETIMEDOUT") return [`Existing code checked: ${shown} did not finish within ${TIMEOUT_MS / 1000} s. Narrow it (a subdirectory, --include, --exclude-dir=node_modules).`];
  if (res.error && code !== "ENOBUFS") return [`Existing code checked: ${shown} failed to run: ${res.error.message}`];
  // grep, rg and git grep exit 1 for "no matches"; find (and git ls-files) exit non-zero only on errors.
  if (!res.error && res.status !== 0 && !(res.status === 1 && tool !== "find")) {
    const why = String(res.stderr || "").trim().split("\n")[0].slice(0, 200);
    return [`Existing code checked: ${shown} errored (exit ${res.status ?? res.signal}): ${why}. The command runs from ${root} without a shell, so globs are not expanded and every path must exist. Fix it so it actually runs.`];
  }

  const claimsNothing = NOTHING.test(value.replace(/`[^`]*`/g, " "));
  if (!claimsNothing) return [];
  const codeHits = String(res.stdout || "").slice(0, MAX_OUT).split("\n").map((l) => l.replace(/\r$/, "")).filter((l) => l.trim() && isCodeHit(l, args));
  if (!codeHits.length) return [];
  const skip = typeof ignore === "function" ? ignore() : ignore;
  const hits = codeHits.filter((l) => !skip?.has(hitPath(l)));
  if (!hits.length) return [];
  return [
    `Existing code checked: the entry says the search found nothing, but re-running ${shown} returned ${hits.length} hit line(s) in code files:\n${hits.slice(0, 5).map((l) => `    ${l.slice(0, 200)}`).join("\n")}\n  Reuse or extend that code, or rewrite the entry to say why each hit does not fit.`,
  ];
}
