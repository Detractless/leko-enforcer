// The fix block shown in a denial about a missing or invalid plan entry: the real path, a suggested Feature
// slug derived from the file's name and folder, and short <placeholders> for the parts only the model knows.
// Copied verbatim it is rejected (placeholders), unlike the template's example values.

import { ROLE_FOLDERS, SOURCE_ROOTS, related, wordsOf } from "../naming/feature-folder.mjs";

const kebab = (s) => String(s).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");

/**
 * Feature slug for `rel` (project-relative, forward slashes): the file stem. In a feature folder (not a source root or
 * role folder) the folder name is put in front unless the stem already says it, so rule R5 accepts the suggestion.
 */
export function suggestSlug(rel) {
  const segs = rel.split("/");
  const dir = segs.length > 1 ? segs[segs.length - 2] : "";
  let stem = kebab(segs[segs.length - 1].split(".")[0]);
  const folder = kebab(dir);
  if (!stem || stem === "index" || stem === "main" || stem === "init" || stem === "conftest") stem = folder || "feature";
  const plain = !folder || SOURCE_ROOTS.has(dir.toLowerCase()) || ROLE_FOLDERS.has(dir.toLowerCase());
  const stemWords = wordsOf(stem);
  const says = wordsOf(folder).some((fw) => stemWords.some((sw) => related(sw, fw))); // R5's own test: a 4-letter prefix or equal
  const slug = (plain || says ? stem : `${folder}-${stem}`).slice(0, 60).replace(/-+$/, "");
  return slug || "feature";
}

export function fixEntry(rel, contract = false) {
  const criterion = contract ? ["**Criterion:** <AC-001: the acceptance criterion of the feature's open contract that this file serves>"] : [];
  return [
    `### ${rel}`,
    "**Purpose:** <what this file does, one sentence>",
    `**Feature:** ${suggestSlug(rel)}`,
    "**Existing code checked:** `<grep -rn -e WORD src>` <what it showed>",
    "**Failure paths:** <what happens on empty or bad input>",
    ...criterion,
    "**Split justification:** <only if the Feature already exists: why this file is separate>",
  ].join("\n");
}
