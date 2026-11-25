// H3-H6: an HTML file holds markup only. Style lives in a stylesheet, behaviour in a script file.
//   H3  no <style> block with rules or @import, no <link href="data:...">
//   H4  no <script> with a body, no <script src="data:..."> or "javascript:..."
//   H5  no on...= handler attributes, no javascript: links
//   H6  at most 3 style= attributes in one file
// Returns { checkName: { count, measure, limit, reason } } for what the text contains. web-check.mjs compares
// the proposed text with the text on disk, so a file that already breaks a rule is only refused when the change
// adds more. A <script> whose type is not JavaScript (ld+json, importmap, ...) is data, not code, and is allowed.

const JS_TYPE = /^(|module|text\/javascript|application\/javascript|text\/babel|text\/jsx)$/i;
const MAX_STYLE_ATTRS = 3;
const TAG = /<[a-zA-Z][a-zA-Z0-9-]*(?:"[^"]*"|'[^']*'|[^<>"'])*>/g;

const scriptType = (attrs) => (/\btype\s*=\s*["']?([^"'\s>]*)/i.exec(attrs) || [, ""])[1];
const scriptSrc = (attrs) => (/\bsrc\s*=\s*["']?([^"'\s>]*)/i.exec(attrs) || [, null])[1];

function add(out, check, count, measure, reason) {
  if (count > 0) out[check] = { count, measure: `${count} ${measure}`, limit: "none allowed in an HTML file", reason };
}

function styleBlocks(html) {
  let rules = 0;
  for (const m of html.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi)) if (/\{|@import/i.test(m[1])) rules++;
  return rules;
}

function scriptBlocks(html) {
  let bodies = 0, smuggled = 0;
  for (const m of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    const src = scriptSrc(m[1]);
    if (src === null && m[2].trim() && JS_TYPE.test(scriptType(m[1]))) bodies++;
    if (src !== null && /^(data|javascript):/i.test(src)) smuggled++;
  }
  return { bodies, smuggled };
}

/** Counts per tag: handlers, javascript: links, style attributes. Attribute values are cut out first. */
function tagCounts(markup) {
  let handlers = 0, jsUrls = 0, styleAttrs = 0;
  for (const [tag] of markup.matchAll(TAG)) {
    jsUrls += (tag.match(/\b(?:href|src|action|formaction)\s*=\s*["']?\s*javascript:/gi) || []).length;
    const names = tag.replace(/=\s*("[^"]*"|'[^']*')/g, "=");
    handlers += (names.match(/\son[a-z]+\s*=/gi) || []).length;
    styleAttrs += (names.match(/\sstyle\s*=/gi) || []).length;
  }
  return { handlers, jsUrls, styleAttrs };
}

export function htmlProblems(source) {
  const html = source.replace(/<!--[\s\S]*?-->/g, "");
  const markup = html.replace(/(<(script|style)\b[^>]*>)[\s\S]*?(<\/\2>)/gi, "$1$3");
  const { bodies, smuggled } = scriptBlocks(html);
  const { handlers, jsUrls, styleAttrs } = tagCounts(markup);
  const out = {};
  add(out, "webInlineStyle", styleBlocks(html), "inline <style> block(s) with CSS rules or @import",
    "CSS in an HTML file splits the styling across two places. Move the rules to a stylesheet and load it with <link rel=\"stylesheet\" href=\"...\">.");
  add(out, "webDataLink", (markup.match(/<link\b[^>]*href\s*=\s*["']?data:/gi) || []).length, "<link href=\"data:...\"> (inline CSS in disguise)",
    "A data: link hides CSS inside the HTML. Use a real stylesheet file.");
  add(out, "webInlineScript", bodies, "inline <script> block(s) with a body",
    "JS in an HTML file cannot be checked or reused. Move it to a module file and load it with <script type=\"module\" src=\"...\"></script>.");
  add(out, "webScriptSrcData", smuggled, "<script src> value(s) starting with data: or javascript:",
    "A data: or javascript: source hides JS inside the HTML. Use a real module file.");
  add(out, "webInlineHandler", handlers, "on...= handler attribute(s) (onclick=, onload=, ...)",
    "Handler attributes put behaviour inside the markup. Attach the listener with addEventListener in the JS file.");
  add(out, "webJavascriptUrl", jsUrls, "javascript: link(s)",
    "A javascript: link is an inline handler in disguise. Use a real listener in the JS file.");
  if (styleAttrs > MAX_STYLE_ATTRS) {
    out.webStyleAttrs = {
      count: styleAttrs, measure: `${styleAttrs} style= attributes in one file`, limit: `at most ${MAX_STYLE_ATTRS} per HTML file`,
      reason: "Style belongs in the stylesheet as classes. Replace the style= attributes with class names.",
    };
  }
  return out;
}
