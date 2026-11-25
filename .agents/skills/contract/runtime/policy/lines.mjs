// Counting rules for the file-size limits. One definition, used by the
// preflight guard and the Stop audit.
//
// Lines: the number of "\n"-separated lines. A trailing "\n" does not add a
// line. "" is 0 lines, "\n" is 1 (one empty line), "a\nb" and "a\nb\n" are 2.
// "\r" is not a separator: a CRLF file counts the same as its LF twin.
//
// Characters: text.length, i.e. UTF-16 code units of the UTF-8 decoded text
// (an emoji counts 2). Bytes are not counted.

export function countLines(text) {
  if (text === "") return 0;
  const parts = text.split("\n").length;
  return text.endsWith("\n") ? parts - 1 : parts;
}

export function countChars(text) {
  return text.length;
}
