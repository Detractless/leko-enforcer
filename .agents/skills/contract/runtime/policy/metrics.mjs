// Function-level measurements from the token stream (JavaScript only).
//
//   lines     closing-brace line minus header line plus 1 (header = the
//             `function`/`async` token, the method name, or the arrow's
//             parameter list).
//   nesting   deepest chain of BRACED control blocks (if/else/for/while/do/
//             switch/try/catch/finally/with) inside the function. An
//             `else if` chain is one level. Braceless bodies add no level.
//   branches  decision points in the function's own body, excluding nested
//             functions: if, for, while, catch, case, &&, ||, ??, ?: and
//             &&= ||= ??=. (`default` and `else` do not count.)
//
// Functions measured: `function` declarations/expressions, methods (class and
// object shorthand), and arrows with a BLOCK body. Arrows with an expression
// body are not separate functions; their tokens count toward the enclosing
// function. Code outside any function is not measured.

const CONTROL_PAREN = new Set(["if", "for", "while", "switch", "catch", "with"]);
const CONTROL_BARE = new Set(["try", "finally", "else", "do", "catch"]);
const BRANCH_WORDS = new Set(["if", "for", "while", "catch", "case"]);
const BRANCH_OPS = new Set(["&&", "||", "??", "?", "&&=", "||=", "??="]);

function pairBrackets(tokens) {
  const pairs = new Array(tokens.length).fill(-1);
  const stack = [];
  tokens.forEach((t, i) => {
    if (t.t !== "p") return;
    if (t.v === "(" || t.v === "[" || t.v === "{") stack.push(i);
    else if (t.v === ")" || t.v === "]" || t.v === "}") {
      const o = stack.pop();
      pairs[o] = i;
      pairs[i] = o;
    }
  });
  return pairs;
}

const isP = (t, v) => t && t.t === "p" && t.v === v;
const isId = (t, v) => t && t.t === "id" && (v === undefined || t.v === v);

/** What does the "{" at index i open? -> {kind:"fn", start, nameAt} | {kind:"ctl"} | null */
function classifyBrace(tokens, pairs, i) {
  const prev = tokens[i - 1];
  if (!prev) return null;
  if (isP(prev, "=>")) {
    let s = i - 2;
    if (isP(tokens[s], ")")) s = pairs[s];
    if (isId(tokens[s - 1], "async")) s -= 1;
    return { kind: "fn", start: s, nameAt: s - 2 };
  }
  if (isId(prev) && CONTROL_BARE.has(prev.v)) return { kind: "ctl" };
  if (!isP(prev, ")")) return null;
  const open = pairs[i - 1];
  const before = tokens[open - 1];
  if (isId(before) && CONTROL_PAREN.has(before.v)) return { kind: "ctl" };
  if (isId(before, "await") && isId(tokens[open - 2], "for")) return { kind: "ctl" };
  if (isId(before, "function")) return fnAt(tokens, open - 1, -1);
  if (isId(before)) {
    const at = open - 1;
    const kw = isP(tokens[at - 1], "*") ? at - 2 : at - 1;
    if (isId(tokens[kw], "function")) return fnAt(tokens, kw, at);
    return isMethodLead(tokens[at - 1]) ? { kind: "fn", start: at, nameAt: at } : null;
  }
  if (isP(before, "]")) return { kind: "fn", start: pairs[open - 1], nameAt: -1 };
  if (before && before.t === "str") return { kind: "fn", start: open - 1, nameAt: open - 1 };
  return null;
}

// A method definition's name follows { } ; , a modifier, or "*". Anything else
// (for example `class A extends mixin(B) {`) is a call expression, not a method.
function isMethodLead(lead) {
  if (!lead) return true;
  if (lead.t === "p") return ["{", "}", ";", ",", "*"].includes(lead.v);
  return lead.t === "id" && ["static", "async", "get", "set"].includes(lead.v);
}

function fnAt(tokens, kwIndex, nameAt) {
  const start = isId(tokens[kwIndex - 1], "async") ? kwIndex - 1 : kwIndex;
  return { kind: "fn", start, nameAt };
}

function nameOf(tokens, found) {
  const at = found.nameAt;
  const t = tokens[at];
  if (found.start === found.nameAt && t) return t.v.replace(/^['"]|['"]$/g, "");
  if (t && isId(t) && at > found.start) return t.v;
  const before = tokens[found.start - 1];
  const target = tokens[found.start - 2];
  if ((isP(before, "=") || isP(before, ":")) && isId(target)) return target.v;
  return "<anonymous>";
}

/** @returns {{name:string,startLine:number,endLine:number,lines:number,nesting:number,branches:number}[]} */
export function measureFunctions(tokens) {
  const pairs = pairBrackets(tokens);
  const fns = [];
  const stack = [];
  let fn = null;
  tokens.forEach((tok, i) => {
    while (stack.length && stack[stack.length - 1].end === i) {
      const f = stack.pop();
      if (f.kind === "fn") fn = f.parent;
      else if (fn) fn.depth--;
    }
    const prev = tokens[i - 1];
    const member = isP(prev, ".") || isP(prev, "?.");
    if (tok.t === "id" && !member && BRANCH_WORDS.has(tok.v) && fn && !isP(tokens[i + 1], ":")) fn.branches++;
    else if (tok.t === "p" && BRANCH_OPS.has(tok.v) && fn) fn.branches++;
    else if (isP(tok, "{")) {
      const found = classifyBrace(tokens, pairs, i);
      if (found && found.kind === "fn") {
        const endTok = tokens[pairs[i]];
        const rec = {
          name: nameOf(tokens, found),
          startLine: tokens[found.start].line,
          endLine: endTok.line,
          lines: endTok.line - tokens[found.start].line + 1,
          nesting: 0,
          branches: 0,
          depth: 0,
        };
        fns.push(rec);
        stack.push({ kind: "fn", end: pairs[i], parent: fn });
        fn = rec;
      } else if (found && fn) {
        fn.depth++;
        fn.nesting = Math.max(fn.nesting, fn.depth);
        stack.push({ kind: "ctl", end: pairs[i] });
      }
    }
  });
  return fns.map(({ depth, ...rest }) => rest);
}
