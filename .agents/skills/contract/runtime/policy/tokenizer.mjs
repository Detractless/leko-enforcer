// A small JavaScript tokenizer for the structure checks. It is not a parser.
//
// Handled: line and block comments, single/double-quoted strings (with line
// continuations), template literals (including nested `${ ... }` and nested
// templates; the embedded expressions are skipped, not tokenized), regex
// literals (decided from the previous token: a "/" after an identifier,
// number, string, ")" "]" or "}" is division; after a keyword such as
// return/typeof/case or after any other punctuation it starts a regex), and
// bracket balance.
//
// It FAILS (returns `error`, no tokens) on: unterminated string, template,
// comment, or regex, and unbalanced/mismatched ( [ {. JSX, TypeScript, Flow
// and other non-JS syntax usually trip one of these; callers report the
// failure as "parser failure" and skip the structure checks.
//
// Known blind spots (documented, not detected): a regex literal that follows
// ")" or "}" is read as division; regex literals inside `${}` are not
// recognised; "/" after "++"/"--" is division.

const OPS = [
  ">>>=", "===", "!==", "**=", "...", "<<=", ">>=", ">>>", "&&=", "||=", "??=",
  "=>", "==", "!=", "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=",
  "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>", "**",
];
const REGEX_AFTER_WORD = new Set([
  "return", "typeof", "case", "in", "of", "instanceof", "new", "delete", "void",
  "throw", "else", "do", "yield", "await", "extends",
]);
const ID_START = /[A-Za-z_$#\u0080-\uffff]/;
const ID_PART = /[\w$\u0080-\uffff]/;
const NUMBER = /^(?:0[xX][0-9a-fA-F_]+n?|0[bB][01_]+n?|0[oO][0-7_]+n?|(?:\d[\d_]*\.?[\d_]*|\.\d[\d_]*)(?:[eE][+-]?\d+)?n?)/;
const CLOSER = { ")": "(", "]": "[", "}": "{" };

class ScanError extends Error {
  constructor(message, line) {
    super(message);
    this.line = line;
  }
}

const newlines = (s) => s.split("\n").length - 1;

function regexAllowed(prev) {
  if (!prev) return true;
  if (prev.t === "id") return REGEX_AFTER_WORD.has(prev.v);
  if (prev.t === "p") return ![")", "]", "}", "++", "--"].includes(prev.v);
  return false;
}

/** @returns {{tokens: {t:string,v:string,line:number}[], error: null|{message:string,line:number}}} */
export function tokenize(src) {
  try {
    return { tokens: scan(src), error: null };
  } catch (e) {
    if (e instanceof ScanError) return { tokens: [], error: { message: e.message, line: e.line } };
    throw e;
  }
}

function scan(src) {
  const n = src.length;
  const toks = [];
  const open = [];
  let i = 0;
  let line = 1;
  const lineAt = (idx) => newlines(src.slice(0, idx)) + 1;
  const fail = (msg, at = line) => { throw new ScanError(msg, at); };

  function skipQuoted(from) {
    const q = src[from];
    let j = from + 1;
    for (;;) {
      if (j >= n || src[j] === "\n") fail("unterminated string", lineAt(from));
      if (src[j] === "\\") { j += src[j + 1] === "\r" && src[j + 2] === "\n" ? 3 : 2; continue; }
      if (src[j] === q) return j + 1;
      j++;
    }
  }
  function skipComment(j) {
    if (src[j + 1] === "/") { while (j < n && src[j] !== "\n") j++; return j; }
    const end = src.indexOf("*/", j + 2);
    if (end < 0) fail("unterminated block comment", lineAt(j));
    return end + 2;
  }
  function skipTemplate(from) {
    let j = from + 1;
    for (;;) {
      if (j >= n) fail("unterminated template literal", lineAt(from));
      if (src[j] === "\\") { j += 2; continue; }
      if (src[j] === "`") return j + 1;
      if (src[j] === "$" && src[j + 1] === "{") { j = skipBraced(j + 2, from); continue; }
      j++;
    }
  }
  function skipBraced(from, tplStart) {
    let depth = 1;
    let j = from;
    while (j < n) {
      const ch = src[j];
      if (ch === "{") depth++;
      else if (ch === "}") { if (--depth === 0) return j + 1; }
      else if (ch === "`") { j = skipTemplate(j); continue; }
      else if (ch === '"' || ch === "'") { j = skipQuoted(j); continue; }
      else if (ch === "/" && (src[j + 1] === "/" || src[j + 1] === "*")) { j = skipComment(j); continue; }
      j++;
    }
    return fail("unterminated template expression", lineAt(tplStart));
  }
  function skipRegex(from) {
    let j = from + 1;
    let inClass = false;
    for (;;) {
      if (j >= n || src[j] === "\n") fail("unterminated regular expression literal", lineAt(from));
      const ch = src[j];
      if (ch === "\\") { j += 2; continue; }
      if (ch === "[") inClass = true;
      else if (ch === "]") inClass = false;
      else if (ch === "/" && !inClass) break;
      j++;
    }
    j++;
    while (j < n && ID_PART.test(src[j])) j++;
    return j;
  }

  if (src.startsWith("#!")) while (i < n && src[i] !== "\n") i++;
  while (i < n) {
    const c = src[i];
    if (c === "\n") { line++; i++; continue; }
    if (c <= " " || c === "\u00a0" || c === "\ufeff" || c === "\u2028" || c === "\u2029") { i++; continue; }
    if (c === "/" && (src[i + 1] === "/" || src[i + 1] === "*")) {
      const end = skipComment(i);
      line += newlines(src.slice(i, end));
      i = end;
      continue;
    }
    const start = line;
    if (c === '"' || c === "'") {
      const end = skipQuoted(i);
      toks.push({ t: "str", v: src.slice(i, end), line: start });
      line += newlines(src.slice(i, end));
      i = end;
      continue;
    }
    if (c === "`") {
      const end = skipTemplate(i);
      toks.push({ t: "tpl", v: "`", line: start });
      line += newlines(src.slice(i, end));
      i = end;
      continue;
    }
    if (c === "/" && regexAllowed(toks[toks.length - 1])) {
      const end = skipRegex(i);
      toks.push({ t: "re", v: src.slice(i, end), line: start });
      i = end;
      continue;
    }
    if (ID_START.test(c)) {
      let j = i + 1;
      while (j < n && ID_PART.test(src[j])) j++;
      toks.push({ t: "id", v: src.slice(i, j), line: start });
      i = j;
      continue;
    }
    const num = (/\d/.test(c) || (c === "." && /\d/.test(src[i + 1] || ""))) && NUMBER.exec(src.slice(i, i + 80));
    if (num) {
      toks.push({ t: "num", v: num[0], line: start });
      i += num[0].length;
      continue;
    }
    if (c === "(" || c === "[" || c === "{") open.push({ ch: c, line });
    else if (CLOSER[c]) {
      const top = open.pop();
      if (!top || top.ch !== CLOSER[c]) fail(`unbalanced '${c}'`);
    }
    const op = OPS.find((o) => src.startsWith(o, i)) || c;
    toks.push({ t: "p", v: op, line: start });
    i += op.length;
  }
  if (open.length) fail(`unclosed '${open[open.length - 1].ch}'`, open[open.length - 1].line);
  return toks;
}
