// The single validation engine. The PreToolUse guard (preflight) and the Stop
// audit both call these two functions, which run the same content checks and
// graph checks; they differ only in the `mode` passed down.
//
//   preflightFile(ctx, rel, content, prev)  -> outcome for one proposed file
//   auditFiles(ctx, changedRels)            -> outcome for files changed since the last audit
//
// outcome = { findings, notices, exempted }
//   findings  violations ({check,file,measure,limit,reason,key,scope?}); a
//             preflight blocks on these
//   notices   visible non-blocking results ({kind,file,text}) with kind
//             unsupported | parser-failure | over-limit
//   exempted  violations an exemption suppressed (listed, never silent)

import { checkContent } from "./contentcheck.mjs";
import { mergeOutcome, newOutcome } from "./common.mjs";
import { checkGraph, checkGraphPreflight, graphConfigured } from "./graphcheck.mjs";

export function preflightFile(ctx, rel, content, prev) {
  const out = checkContent(ctx.policy, "preflight", rel, content, prev);
  return mergeOutcome(out, checkGraphPreflight(ctx, rel, content, prev));
}

export function auditFiles(ctx, changedRels) {
  const out = newOutcome();
  const changed = new Set(changedRels);
  for (const rel of changedRels) {
    const text = ctx.read(rel);
    if (text !== null) mergeOutcome(out, checkContent(ctx.policy, "audit", rel, text, null));
  }
  if (graphConfigured(ctx.policy)) {
    const g = checkGraph(ctx);
    const touches = (f) => (f.involves || f.files || [f.file]).some((x) => changed.has(x));
    out.findings.push(...g.findings.filter(touches));
    out.exempted.push(...g.exempted.filter(touches));
    out.notices.push(...g.notices.filter((n) => changed.has(n.file)));
    out.graph = g.stats;
  }
  return out;
}
