---
name: research-first
description: >
  Parallel, verification-driven research protocol: hypothesize what should exist, fan out
  overlapping searchers with different community vocabularies, adversarially verify, synthesize
  with a dead-end registry and build-vs-borrow verdict. Use this skill whenever the user asks to
  research, investigate, survey, "look into", "find options for", or compare tools/models/libraries/
  approaches — AND proactively, without being asked, BEFORE designing or coding anything complex
  (a new feature, pipeline, engine, integration, or any component that would take more than ~an hour
  to build): first check what already exists to borrow. The goal of building is to make what's
  possible possible — most complex things are assemblies of existing parts, and skipping the search
  step risks weeks of reinventing something that exists as a 7 MB Apache-licensed file. Also
  trigger when the user is about to choose between technical options (models, libraries, formats,
  licenses) with real consequences. Do NOT trigger for single-fact lookups, questions answerable
  from the current codebase, or trivial glue code.
---

# Research-first: parallel overlapping search

A protocol for finding what already exists — thoroughly enough to trust a "nothing exists"
conclusion, cheaply enough to run before any significant build. Distilled from a live session
where the decisive find (a 44-download model that was exactly the needed architecture) was
invisible to one search angle and obvious to another; the method's value is proven, not
theoretical.

**The governing principle: search-before-build.** Before writing anything complex, assume someone
has built part of it. The deliverable of research is a *build-vs-borrow map*: what to adopt, what
to adapt, and the (hopefully small) genuinely novel gap that justifies writing new code. If
research concludes "build it all," that conclusion must survive the same verification as any
other claim.

## Phase 0 — Scale the effort (do not skip, do not over-trigger)

Match the protocol to the stakes. Fanning out three agents to answer a one-fact question wastes
tokens and time; searching solo before a multi-week build risks far worse.

| Scale | When | Shape |
|-------|------|-------|
| **Quick** | single fact, known-name lookup | search directly, no protocol |
| **Solo** | small build decision, one library choice | Phases 1 + 4 + 5 yourself, ~15–30 min |
| **Standard** | user asked for research; before a complex component | you + 1–2 parallel agents, full protocol |
| **Full** | before a major architecture; "wide search, miss nothing" | 2–3 angled agents + a dedicated verifier + snowball round |

## Phase 1 — Hypothesize before searching

Search engines reward knowing the name of the thing. So first, *predict the thing*:

1. **Enumerate what could exist.** Write the taxonomy of plausible solutions — categories, not
   products. For each: what would it be called? What would its README say? If you can imagine it,
   someone may have built it — search for the imagined name ("segment any text" was findable
   because the hypothesized capability matched a real project's actual name).
2. **Map the communities.** The same artifact carries different names in different fields
   (speech researchers say "disfluency removal"; ML people say "token-classification deletion
   tagger"). List every community that might own the problem — academic fields, product
   ecosystems, adjacent industries, non-English ecosystems. Each community's *vocabulary* becomes
   a search angle. This step is what makes overlap productive rather than redundant.
3. **State what "good" means up front** so every searcher reports comparable fields: typically
   license, runtime/language, size/speed evidence, maintenance status, and task fit — adjust per
   domain.

## Phase 2 — Fan out: angled, overlapping, independent

Spawn parallel searchers (subagents when available; sequential self-passes otherwise), each with
a **different community's vocabulary** as its mandate. Deliberate overlap is a feature: for
obscure artifacts each searcher has low find-probability *p*, and independent sweeps compound
(1−(1−p)ᵏ). The decisive find is usually cheap for one angle and invisible to another.

Rules that make the fan-out work:

- **Independence.** Searchers must not see each other's results mid-flight, or they inherit each
  other's blind spots — including premature "doesn't exist" conclusions.
- **Structured deliverable spec.** Every mandate names the exact fields to report, and makes
  **dead-end reporting mandatory** ("things that sound right but are unusable, and why") — the
  dead-end registry prevents the whole team from re-researching traps forever.
- **One registry-enumeration angle.** Web search *samples*; databases *enumerate*. Assign one
  searcher (or one pass) to systematically walk the structured registries for the domain —
  HuggingFace by task-tag, crates.io/PyPI/npm by keyword, awesome-lists, paperswithcode — where
  an obscure artifact is found deterministically rather than luckily.
- Search names you hypothesized in Phase 1, synonyms, and adjacent phrasings; include the
  current year in recency-sensitive queries.

## Phase 3 — Search yourself, in parallel

Do not only orchestrate. Run your own sweep on the highest-value unknowns while agents work —
the lead's context (knowing *why* each question matters) catches fit-relevance that mandate-bound
agents miss, and your interim results sharpen how you read their reports.

## Phase 4 — Verify adversarially

Findings are claims, not facts. Before any finding influences a decision:

- **Three-field license check** on anything with a model or dataset: **code / weights / training
  data** are licensed independently and diverge constantly. Registry license badges are often the
  uploader's wrong guess — read the actual LICENSE file and the model/data cards. Classic traps:
  Apache code over non-commercial weights; permissive weights trained on research-only corpora;
  an LGPL data file inside an MIT repo that infects the shipped binary.
- **Verify load-bearing claims at the primary source** — the repo's LICENSE file, the benchmark
  table in the paper, the last-commit date — not the searcher's summary of them.
- **Tag every claim** verified / reported / inferred, and keep unverified numbers out of
  decisions. At Full scale, make this a dedicated verifier agent whose only job is to try to
  *disprove* the top findings.
- Treat "nothing exists" as a claim too — it earns belief only after multiple angles, the
  registry sweep, and the snowball round all come back empty.

## Phase 5 — Snowball from the best finds

One search round is a sample; citation-chasing completes it. From each top find: who cites it,
what does it cite, what does its README link to, what else did its author build, what sits beside
it in its awesome-list? One seeded second round is cheap and reliably surfaces the long tail the
first round missed.

## Phase 6 — Synthesize with adjudication

- **Do not average disagreements — resolve them.** When searchers conflict ("no shippable X
  exists" vs. "here is X"), the resolution is usually a sharper truth than either report
  ("X exists as a recipe; one ingredient blocks shipping; the fix is a small retrain").
- **Produce the consolidated report** (structure below), never just concatenated agent output.
- **End with a build-vs-borrow verdict**: adopt / adapt / build columns, with the novel gap
  explicitly named — that gap is the actual engineering project.

## Report structure

ALWAYS include these sections (rename freely, keep the content):

```
# <topic> — research report (<date>)
## Headline verdict            — 3–5 sentences: what exists, what doesn't, what it means
## Chosen candidates           — table incl. per-item licenses (code/weights/data), size/speed
                                 evidence, maintenance, fit
## Standout finds              — anything that changes the plan, with the catch stated honestly
## Dead-end registry           — every trap found, WITH the reason (so it's never re-researched)
## Build-vs-borrow verdict     — adopt / adapt / build-new, the novel gap named
## Recommended next actions    — ordered, smallest-commitment first (gate-checks and spikes
                                 before adoption)
## Sources                     — primary links for every load-bearing claim
```

## Why each part matters (the compressed rationale)

Hypothesis-first turns recall into recognition. Community-vocabulary angles are the real
coverage mechanism — parallelism without vocabulary diversity is just the same search three
times. Independence keeps errors uncorrelated. Mandatory dead-ends turn negative results into
permanent savings. Adversarial verification exists because the most expensive research failure
is a confident wrong claim (usually a license) discovered after integration. And the
search-before-build trigger exists because the cheapest code is the code you didn't write:
finding a maintained, permissively-licensed component is almost always faster than building one
— the craft is in the finding and the verifying.
