// H1 + X1: a stylesheet gets its colors, font sizes and z-index values from tokens (var(--...)).
//   H1  no color literal outside :root: #hex, rgb()/hsl()/hwb()/lab()/lch()/oklab()/oklch()/color() with numbers,
//       and named colors (red, steelblue, ...) in color-bearing properties. `transparent`, `currentColor` and
//       `inherit` are always fine, and so is a function around tokens: rgb(var(--c) / 50%), color-mix(in srgb, var(--a), var(--b)).
//       A fallback counts: var(--bg, #fff) holds the literal #fff.
//   X1  no font-size with a literal length (except 100%, 1em, 1rem, 0) and no z-index above 1 or below -1, unless it uses var().
//       (The write guard refuses only the z-index part; web-check.mjs lists font-size as report-only, for the /refactor scan.)
// The file that defines the :root tokens (isTokenFile) may also hold color literals in custom properties of other
// selectors, so a [data-theme="dark"] block can sit beside :root. Everywhere else a --custom: #fff counts as a literal.
// Returns { checkName: { count, measure, limit, reason } }; web-check.mjs refuses only what a change adds.

const NAMED = new Set(("aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen").split(" "));

const COLOR_PROP = /^(?:(?:[\w-]*-)?color|background|border[\w-]*|outline[\w-]*|fill|stroke|box-shadow|text-shadow|text-decoration[\w-]*|column-rule[\w-]*)$/;
const COLOR_FN = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch)\(\s*([^)]*)/gi;
const COLOR_SPACE = /\bcolor\(\s*(?:srgb|srgb-linear|display-p3|a98-rgb|prophoto-rgb|rec2020|xyz|xyz-d50|xyz-d65)\b/gi;
const FONT_OK = /^(?:100%|1em|1rem|0)$/;
const FONT_LENGTH = /\d(?:px|rem|em|pt|ex|ch|vw|vh|vmin|vmax|%)/i;
const DECLARATION = /([\w-]+)\s*:\s*([^;{}]*)(?=;|\})/g;

const withoutVarCalls = (v) => v.replace(/var\(\s*--[\w-]+\s*(?:,([^)]*))?\)/gi, " $1 ");
const withoutUrls = (v) => v.replace(/url\(\s*(?:"[^"]*"|'[^']*'|[^)]*)\)/gi, " ");

function colorHits(value, named) {
  const v = withoutVarCalls(withoutUrls(value));
  let n = (v.match(/#[0-9a-fA-F]{3,8}\b/g) || []).length;
  for (const m of v.matchAll(COLOR_FN)) if (/^\s*(?:[-+.\d]|none\b)/i.test(m[1])) n++;
  n += (v.match(COLOR_SPACE) || []).length;
  if (named) for (const w of v.matchAll(/(?<![\w-])[a-z]+(?![\w-])/gi)) if (NAMED.has(w[0].toLowerCase())) n++;
  return n;
}

function entry(count, measure, limit, reason) {
  return count > 0 ? { count, measure, limit, reason } : null;
}

export function cssProblems(source, isTokenFile) {
  const css = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/:root\s*\{[\s\S]*?\}/gi, "");
  let colors = 0, fonts = 0, layers = 0;
  for (const [, name, raw] of css.matchAll(DECLARATION)) {
    const prop = name.toLowerCase();
    const value = raw.trim();
    if (prop.startsWith("--")) {
      if (!isTokenFile) colors += colorHits(value, true);
      continue;
    }
    colors += colorHits(value, COLOR_PROP.test(prop));
    const usesVar = /\bvar\(/i.test(value);
    if (prop === "font-size" && !usesVar && FONT_LENGTH.test(value) && !FONT_OK.test(value)) fonts++;
    if (prop === "z-index" && !usesVar && /^[-+]?\d+$/.test(value) && Math.abs(Number(value)) > 1) layers++;
  }
  const out = {};
  const put = (check, e) => { if (e) out[check] = e; };
  put("webColorLiteral", entry(colors, `${colors} color value(s) written out outside :root`, "none outside the :root tokens",
    "Define each color once as a --color-... variable in :root in the one tokens file (for example styles/tokens.css), and use var(--color-...) here. transparent, currentColor and inherit are fine."));
  put("webFontSizeLiteral", entry(fonts, `${fonts} font-size value(s) written out`, "font-size comes from a token",
    "Define the size once as a --font-size-... variable in the tokens file and use var(--font-size-...) here."));
  put("webZIndexLiteral", entry(layers, `${layers} z-index value(s) written out`, "z-index comes from a token (0, 1 and -1 are fine)",
    "Define the layer once as a --z-... variable in the tokens file and use var(--z-...) here, so the stacking order has one list."));
  return out;
}
