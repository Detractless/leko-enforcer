// The complete content a Write / Edit / MultiEdit WOULD leave on disk.
//
// Edits are applied in order to the current file, the way the tools do:
// old_string must exist; without replace_all it must be unique; each edit sees
// the result of the previous one. When the content cannot be computed (old
// string missing or not unique, unreadable file, malformed input) the result is
// {status:"skip"} and the caller ALLOWS the call so the tool reports its own
// error. The result is never filtered by size: an edit that adds lines while
// removing characters is still returned and therefore still checked.

import fs from "node:fs";

function countOccurrences(text, needle) {
  let n = 0;
  for (let at = text.indexOf(needle); at !== -1; at = text.indexOf(needle, at + needle.length)) n++;
  return n;
}

function applyEdit(text, edit) {
  if (!edit || typeof edit.old_string !== "string" || typeof edit.new_string !== "string") return null;
  const { old_string: oldStr, new_string: newStr } = edit;
  if (oldStr === "") return text === "" ? newStr : null;
  const count = countOccurrences(text, oldStr);
  if (count === 0) return null;
  if (edit.replace_all) return text.split(oldStr).join(newStr);
  if (count !== 1) return null;
  const at = text.indexOf(oldStr);
  return text.slice(0, at) + newStr + text.slice(at + oldStr.length);
}

function readCurrent(abs) {
  try {
    return { text: fs.readFileSync(abs, "utf8") };
  } catch (e) {
    return e.code === "ENOENT" ? { text: null } : { error: e.code || e.message };
  }
}

/** @returns {{status:"ok",content:string,prev:string|null}|{status:"skip",why:string}} */
export function proposedContent(toolName, input, abs) {
  const cur = readCurrent(abs);
  if (cur.error) return { status: "skip", why: `cannot read current file (${cur.error})` };
  if (toolName === "Write") {
    return typeof input.content === "string"
      ? { status: "ok", content: input.content, prev: cur.text }
      : { status: "skip", why: "Write without string content" };
  }
  const edits = toolName === "MultiEdit" ? input.edits : [input];
  if (!Array.isArray(edits) || edits.length === 0) return { status: "skip", why: "no edits" };
  let text = cur.text ?? "";
  for (const edit of edits) {
    text = applyEdit(text, edit);
    if (text === null) return { status: "skip", why: "an old_string was not found or not unique (the tool will report it)" };
  }
  return { status: "ok", content: text, prev: cur.text };
}
