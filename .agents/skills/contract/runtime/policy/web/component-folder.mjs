// C1: the component folder contract, for files under a folder called components/.
//   - no code file directly in components/ (an index.* barrel is fine): each component gets its own folder
//   - the component folder is kebab-case: components/recipe-card/, not RecipeCard/ or recipe_card/
//   - every file directly in components/<name>/ is called <name>, <name>-something, or index:
//     recipe-card.js, recipe-card.css, recipe-card-header.js, recipe-card.test.js, index.js
// It looks at the path only, so web-check.mjs refuses it for NEW files; a file that already sits in the wrong place
// can still be edited. Whether a folder has its anchor file (<name>.js or index.js) is left to the /refactor scan,
// because a folder is built file by file and no order is wrong.

import path from "node:path";
import { CODE_EXT } from "../../plan/core.mjs";

const KEBAB = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;

function problem(measure, reason) {
  return { webComponentFolder: { count: 1, measure, limit: "components/<kebab-name>/<kebab-name>[-part].<ext>", reason } };
}

export function componentProblems(rel) {
  const segs = rel.split("/");
  const at = segs.indexOf("components");
  const file = segs[segs.length - 1];
  if (at === -1 || at === segs.length - 1 || !CODE_EXT.has(path.extname(file).toLowerCase())) return {};
  const stem = file.split(".")[0];
  const rest = segs.slice(at + 1);
  if (rest.length === 1) {
    return stem === "index" ? {} : problem(`${file} sits directly in components/`,
      `Give the component its own folder: components/${stem.toLowerCase().replace(/[^a-z0-9]+/g, "-")}/${file}. A component folder keeps its markup, style and test together.`);
  }
  const name = rest[0];
  if (!KEBAB.test(name)) return problem(`the component folder "${name}" is not lowercase-with-dashes`, `Name the folder in lowercase with dashes, for example components/${name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/[^A-Za-z0-9]+/g, "-").toLowerCase()}/.`);
  if (rest.length === 2 && stem !== "index" && stem !== name && !stem.startsWith(`${name}-`)) {
    return problem(`${file} does not carry the component name "${name}"`, `Every file in components/${name}/ is called ${name}, ${name}-something, or index (for example ${name}.js, ${name}.css, ${name}-header.js). Rename it, or move it to the component it belongs to.`);
  }
  return {};
}
