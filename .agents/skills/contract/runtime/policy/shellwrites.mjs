// BEST-EFFORT detection of shell commands that write to a file. This is text
// matching on the command line, not a sandbox. It splits the line into
// commands (quotes, escapes, heredoc bodies respected), then recognises:
//
//   redirection   >  >>  >|  &>  N>  to a file (not /dev/null, not >&N)
//   tee           tee [-a] FILE...
//   in-place      sed -i / --in-place, perl -i (any flag cluster with i)
//   copy/move     cp mv install ln rsync: the destination (a directory
//                 destination expands to dir/basename(source)); for mv the
//                 sources too
//   delete        rm unlink rmdir shred truncate: every argument
//   dd            of=FILE
//   git           checkout, restore, rm, mv, clean with path arguments (a
//                 directory argument covers files below it). Index-only forms
//                 (`restore --staged` without --worktree, `rm --cached`) leave the
//                 working file alone and are NOT writes.
//   nested        bash/sh/zsh -c "<script>" and eval "<script>" are scanned
//
// bulk git      clean, reset --hard, stash, checkout/switch --force name no path:
//               they are returned in `bulk` (see git-bulk.mjs) for a git-backed check
//
// NOT detected (reported to the caller as nothing): python/node/ruby/perl -e
// scripts that open files, awk -i inplace, patch, git apply/am/merge/pull/
// rebase/checkout <branch>, curl -o/wget -O, xargs, find -exec,
// editors, heredoc bodies fed to an interpreter, aliases, functions, and any
// target given by variable, command substitution, or glob (those are returned
// in `unresolved` so the caller can say it could not check them). Only a word in
// a TARGET position of a write operator counts: assigning a variable, a sed/perl
// script, a truncate size or a git branch/ref given by variable is not reported.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { bulkGitKind } from "./git-bulk.mjs";

const NON_FILES = new Set(["/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty", "/dev/zero"]);
const LEADERS = new Set(["sudo", "command", "builtin", "exec", "nohup", "time", "env", "nice", "!", "{", "}", "then", "do", "else", "elif", "if", "while", "until"]);
const REDIRECT = /^(>>|>\||>&|>|&>>|&>)/;

function lex(command) {
  const toks = [];
  const heredocs = [];
  let cur = null;
  let i = 0;
  const n = command.length;
  const end = () => { if (cur) { toks.push(cur); cur = null; } };
  const add = (ch, dynamic = false) => { cur ||= { k: "word", text: "", dynamic: false }; cur.text += ch; if (dynamic) cur.dynamic = true; };
  while (i < n) {
    const c = command[i];
    if (c === " " || c === "\t") { end(); i++; continue; }
    if (c === "\n") {
      end(); toks.push({ k: "sep" }); i++;
      for (const d of heredocs.splice(0)) {
        while (i < n) {
          let e = command.indexOf("\n", i);
          if (e < 0) e = n;
          const lineText = command.slice(i, e).trim();
          i = Math.min(e + 1, n);
          if (lineText === d) break;
        }
      }
      continue;
    }
    if (c === "#" && !cur) { while (i < n && command[i] !== "\n") i++; continue; }
    if (c === "\\") { if (command[i + 1] === "\n") { i += 2; continue; } add(command[i + 1] ?? ""); i += 2; continue; }
    if (c === "'") { const e = command.indexOf("'", i + 1); const stop = e < 0 ? n : e; cur ||= { k: "word", text: "", dynamic: false }; cur.text += command.slice(i + 1, stop); i = stop + 1; continue; }
    if (c === '"') {
      cur ||= { k: "word", text: "", dynamic: false };
      i++;
      while (i < n && command[i] !== '"') {
        if (command[i] === "\\" && i + 1 < n) { cur.text += command[i + 1]; i += 2; continue; }
        if (command[i] === "$" || command[i] === "`") cur.dynamic = true;
        cur.text += command[i++];
      }
      i++;
      continue;
    }
    if (c === "$" || c === "`") {
      add(c, true); i++;
      if (c === "`") { while (i < n && command[i] !== "`") add(command[i++]); add("`"); i++; }
      else if (command[i] === "(") { let depth = 0; while (i < n) { if (command[i] === "(") depth++; if (command[i] === ")" && --depth === 0) { add(")"); i++; break; } add(command[i++]); } }
      continue;
    }
    if (c === ">" || c === "<" || (c === "&" && command[i + 1] === ">")) {
      if (cur && /^\d+$/.test(cur.text)) cur = null; else end();
      if (command.startsWith("<<<", i)) { toks.push({ k: "redir", op: "<" }); i += 3; continue; }
      if (command.startsWith("<<", i)) {
        i += command[i + 2] === "-" ? 3 : 2;
        while (command[i] === " " || command[i] === "\t") i++;
        let delim = "";
        while (i < n && !/[\s;|&<>()]/.test(command[i])) { if (!"'\"\\".includes(command[i])) delim += command[i]; i++; }
        heredocs.push(delim);
        toks.push({ k: "redir", op: "<" }, { k: "word", text: "(heredoc)", dynamic: false });
        continue;
      }
      const m = REDIRECT.exec(command.slice(i, i + 3));
      const op = m ? m[1] : "<";
      toks.push({ k: "redir", op });
      i += op.length;
      continue;
    }
    if (c === ";" || c === "|" || c === "&" || c === "(" || c === ")") {
      end(); toks.push({ k: "sep" });
      i += (c === "&" || c === "|") && command[i + 1] === c ? 2 : 1;
      continue;
    }
    add(c, c === "*" || c === "?" || c === "[");
    i++;
  }
  end();
  return toks;
}

function toSegments(toks) {
  const segs = [];
  let seg = { words: [], redirects: [] };
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i];
    if (t.k === "sep") { if (seg.words.length || seg.redirects.length) segs.push(seg); seg = { words: [], redirects: [] }; }
    else if (t.k === "redir") {
      const target = toks[i + 1];
      if (target && target.k === "word") {
        i++;
        const fd = t.op === ">&" && /^(\d+|-)$/.test(target.text);
        if (t.op !== "<" && !fd && !NON_FILES.has(target.text)) seg.redirects.push(target);
      }
    } else seg.words.push(t);
  }
  if (seg.words.length || seg.redirects.length) segs.push(seg);
  return segs;
}

/**
 * @returns {{targets: {abs:string, word:string, via:string, dirCovers:boolean}[], unresolved: {word:string, via:string}[]}}
 */
export function findShellWrites(command, cwd, depth = 0) {
  const out = { targets: [], unresolved: [], bulk: [] };
  // Every directory the shell could be in: the start directory plus each place a
  // `cd` went. Subshell scoping is not modelled, so a relative path is checked
  // against all of them (this can only add denials, never hide one).
  const state = { cwds: Array.isArray(cwd) ? [...cwd] : [cwd] };
  const aim = (word, via, dirCovers = false, only = state.cwds) => {
    if (word.dynamic) return out.unresolved.push({ word: word.text, via });
    const text = word.text.startsWith("~") ? path.join(os.homedir(), word.text.slice(1)) : word.text;
    for (const c of only) out.targets.push({ abs: path.resolve(c, text), word: word.text, via, dirCovers });
  };
  for (const seg of toSegments(lex(command))) {
    seg.redirects.forEach((w) => aim(w, "redirection (>)"));
    let words = seg.words;
    while (words.length && (LEADERS.has(words[0].text) || /^[A-Za-z_]\w*=/.test(words[0].text))) words = words.slice(1);
    if (!words.length) continue;
    inspectCommand(words, state, out, aim, depth);
  }
  return out;
}

// Words that are arguments of a write operator but not its targets (a dynamic one must not
// be reported as an unchecked write target): the sed/perl script, truncate's size.
function notTargets(cmd, args) {
  const skip = new Set();
  const after = (re) => args.forEach((a, i) => { if (re.test(a.text) && args[i + 1]) skip.add(args[i + 1]); });
  if (cmd === "sed" || cmd === "gsed") {
    after(/^(-[A-Za-z]*[ef]|--expression|--file)$/);
    if (!args.some((a) => /^(-[A-Za-z]*[ef]|--expression|--file)/.test(a.text))) {
      const script = args.find((a) => !a.text.startsWith("-") && a.text !== "");
      if (script) skip.add(script);
    }
  } else if (cmd === "perl") after(/^-[A-Za-z]*[eE]$/);
  else if (cmd === "truncate") after(/^(-s|--size|-r|--reference)$/);
  return skip;
}

function inspectCommand(words, state, out, aim0, depth) {
  const cmd = path.basename(words[0].text);
  const args = words.slice(1);
  const skip = notTargets(cmd, args);
  const aim = (w, ...rest) => (skip.has(w) && w.dynamic ? undefined : aim0(w, ...rest));
  const flags = args.filter((a) => a.text.startsWith("-") && a.text !== "-");
  const files = args.filter((a) => !a.text.startsWith("-") || a.text === "-");
  const hasFlag = (re) => flags.some((f) => re.test(f.text));
  if (cmd === "cd") { if (args[0] && !args[0].dynamic) state.cwds.push(...state.cwds.map((c) => path.resolve(c, args[0].text))); return; }
  if (/^(ba|z|da|k)?sh$/.test(cmd) && hasFlag(/^-[a-z]*c[a-z]*$/) && depth < 3) {
    const script = files[0];
    if (script) { const inner = findShellWrites(script.text, state.cwds, depth + 1); out.targets.push(...inner.targets); out.unresolved.push(...inner.unresolved); out.bulk.push(...inner.bulk); }
  } else if (cmd === "eval" && depth < 3) {
    const inner = findShellWrites(args.map((a) => a.text).join(" "), state.cwds, depth + 1);
    out.targets.push(...inner.targets);
    out.unresolved.push(...inner.unresolved);
    out.bulk.push(...inner.bulk);
  } else if (cmd === "tee") files.forEach((w) => aim(w, "tee"));
  else if ((cmd === "sed" || cmd === "gsed") && hasFlag(/^(-[A-Za-z]*i|--in-place)/)) files.forEach((w) => aim(w, "sed -i"));
  else if (cmd === "perl" && hasFlag(/^-[A-Za-z]*i/)) files.forEach((w) => aim(w, "perl -i"));
  else if (["cp", "install", "mv", "ln", "rsync"].includes(cmd)) copyTargets(cmd, files, state, aim);
  else if (["rm", "unlink", "rmdir", "shred", "truncate"].includes(cmd)) files.forEach((w) => aim(w, cmd, hasFlag(/^-[a-zA-Z]*[rR]/)));
  else if (cmd === "dd") args.filter((a) => a.text.startsWith("of=")).forEach((a) => aim({ ...a, text: a.text.slice(3) }, "dd of="));
  else if (cmd === "git") gitTargets(args, state, aim, out);
}

function copyTargets(cmd, files, state, aim) {
  if (files.length < 2) return;
  const dest = files[files.length - 1];
  const sources = files.slice(0, -1);
  for (const c of state.cwds) {
    let destIsDir = false;
    try { destIsDir = !dest.dynamic && fs.statSync(path.resolve(c, dest.text)).isDirectory(); } catch { /* not a directory */ }
    if (destIsDir) sources.forEach((s) => aim({ ...dest, text: path.join(dest.text, path.basename(s.text)) }, cmd, false, [c]));
    else aim(dest, cmd, false, [c]);
  }
  if (cmd === "mv") sources.forEach((s) => aim(s, "mv (source is moved away)", true));
}

function gitTargets(args, state, aim, out) {
  let i = 0;
  while (i < args.length && args[i].text.startsWith("-")) i += args[i].text === "-C" || args[i].text === "-c" ? 2 : 1;
  if (args[i - 2] && args[i - 2].text === "-C" && !args[i - 1].dynamic) state.cwds.push(...state.cwds.map((c) => path.resolve(c, args[i - 1].text)));
  const sub = args[i] && args[i].text;
  const rest = args.slice(i + 1);
  const scoped = rest.findIndex((a) => a.text === "--");
  if (sub === "stash" && scoped >= 0 && rest.length > scoped + 1) { rest.slice(scoped + 1).forEach((w) => aim(w, "git stash", true)); return; }
  const bulk = bulkGitKind(sub, rest.map((a) => a.text));
  if (bulk) out.bulk.push({ ...bulk, cwds: [...state.cwds] });
  if (!["checkout", "restore", "rm", "mv", "clean"].includes(sub)) return;
  const flagTexts = rest.map((a) => a.text).filter((t) => t.startsWith("-") && t !== "-" && t !== "--");
  const has = (re) => flagTexts.some((f) => re.test(f));
  // Index-only forms never touch the working file.
  if (sub === "rm" && has(/^--cached$/)) return;
  if (sub === "restore" && has(/^(--staged|-[A-Za-z]*S[A-Za-z]*)$/) && !has(/^(--worktree|-[A-Za-z]*W[A-Za-z]*)$/)) return;
  const dashdash = rest.findIndex((a) => a.text === "--");
  rest.forEach((w, at) => {
    if (w.text === "--" || (w.text.startsWith("-") && w.text !== "-")) return;
    // `git checkout $REF` / `-b "$BRANCH"`: before `--` a dynamic word is a ref, not a path.
    if (sub === "checkout" && w.dynamic && !(dashdash >= 0 && at > dashdash)) return;
    aim(w, `git ${sub}`, true);
  });
}
