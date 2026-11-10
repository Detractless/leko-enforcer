# Contract: speech-normalization

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.

## Goal
When Leko speaks, a deterministic local normalizer turns clearly recognized English numbers, dates, times, units, abbreviations, and slash lists into easy-to-understand speech text. It changes only the TTS-bound string, preserves ambiguous or technical text, and never converts a value to a different unit system.

## Domains
Domains: none

## Acceptance criteria
- AC-001: Speech normalization changes only the TTS-bound text; the model reply, UI text, transcript, memory, and model context remain unchanged.
  - evidence: `tests/speech_verbalizer.test.mjs "keeps_non_speech_text_unchanged"`
  - breaker: A spoken rewrite leaking into the displayed reply or stored conversation would change the user's record of what the model said.
- AC-002: URLs, email addresses, file paths, code spans, versions, identifiers, hashes, and their punctuation survive normalization unchanged.
  - evidence: `tests/speech_verbalizer.test.mjs "preserves_urls_paths_code_and_identifiers"`
  - breaker: Generic punctuation or slash cleanup could corrupt a link, path, or identifier.
- AC-003: With the accepted `en-US` locale, valid month-name and supported numeric dates are spoken with an ordinal day and readable year.
  - evidence: `tests/speech_verbalizer.test.mjs "speaks_en_us_dates_with_ordinal_days"`
  - breaker: Zero-padded dates or a wrong month/day order could be spoken literally or misread.
- AC-004: Clock times and weekday abbreviations are normalized only in their appropriate calendar context; no AM/PM or zone is invented, and ordinary words remain unchanged.
  - evidence: `tests/speech_verbalizer.test.mjs "speaks_clock_times_and_expands_weekdays_by_context"`
  - breaker: Ratios, citations, `Monument`, or underspecified times could be changed into incorrect calendar speech.
- AC-005: Clearly typed cardinals, ordinals, decimals, signs, ranges, and fractions are spoken in `en-US` wording without losing digits or meaningful precision, while dates, ratios, IDs, and code are preserved.
  - evidence: `tests/speech_verbalizer.test.mjs "speaks_typed_numbers_and_preserves_protected_numeric_forms"`
  - breaker: A blanket number-to-words or slash pass could alter versions, dates, IDs, scores, ratios, or decimal precision.
- AC-006: The explicit supported inventory expands `µs/ms/s/sec/min/h/hr`; `mm/cm/m/km/in/inch/ft/foot/yd/yard/mi/mile`; `mg/g/kg/lb/lbs`; `mL/L/cup/pt/qt/gal/fl oz`; `km/h/km/hr/m/s/mph`; `°C/°F`; and `m²/ft²/sq ft/sq mi` with correct singular/plural and compound forms, without changing the numeric value or unit system.
  - evidence: `tests/speech_verbalizer.test.mjs "speaks_supported_measurement_families_without_converting_values"`
  - breaker: A missing or malformed unit mapping could produce a wrong quantity or silently convert the measurement.
- AC-007: Ambiguous `m`, `in`, and `oz` forms are expanded only with strong context; otherwise their written form is preserved.
  - evidence: `tests/speech_verbalizer.test.mjs "preserves_ambiguous_unit_abbreviations"`
  - breaker: `m` could become minutes instead of meters, `in` could lose its preposition meaning, or `oz` could confuse mass and fluid volume.
- AC-008: Exact `w/`, `w/o`, and contextual `I/O` mappings run before ordinary slash-list handling; `A/B`, `A/B/C`, and explicit `and/or` use the approved distinct readings.
  - evidence: `tests/speech_verbalizer.test.mjs "applies_abbreviations_and_slash_semantics_by_context"`
  - breaker: Generic slash replacement could say “and or,” break units, or change an acronym.
- AC-009: Speech preparation adds pauses only at clear list or clause boundaries and does not paraphrase the model's answer or insert a pause after every comma.
  - evidence: `tests/speech_cadence.test.mjs "pauses_at_clear_boundaries_without_rephrasing"`
  - breaker: A long list could remain hard to follow or become choppy and change the answer's wording.
- AC-010: Normalization is deterministic and idempotent, and a mid-pass failure returns the original string instead of partial output or an exception.
  - evidence: `tests/speech_verbalizer.test.mjs "normalization_is_idempotent_and_fails_open"`
  - breaker: The existing double call path could expand a unit twice, or a later rule failure could leave a sentence half-normalized or stop speech entirely.

## Checklist coverage
- No named project checklist applies because this is a voice-text backend change, not UI, web, CLI, or data-domain work.

## Derived checklist
- D-001: Keep the normalized speech string separate from every non-audio reply surface -> criterion:AC-001
- D-002: Protect URL, email, path, code, and identifier spans before punctuation cleanup -> criterion:AC-002
- D-003: Use the accepted `en-US` locale and validate dates before changing them -> criterion:AC-003
- D-004: Recognize clock times and weekdays only in calendar-like spans -> criterion:AC-004
- D-005: Normalize typed numbers while protecting date, ratio, identifier, and code forms -> criterion:AC-005
- D-006: Use an explicit unit inventory and preserve each value and unit system -> criterion:AC-006
- D-007: Preserve uncertain abbreviations instead of guessing -> criterion:AC-007
- D-008: Handle exact abbreviations and recognized compound units before generic slash lists -> criterion:AC-008
- D-009: Add only clear speech pauses without paraphrase -> criterion:AC-009
- D-010: Make transformations idempotent and fail safely -> criterion:AC-010

## Edge cases
- Empty input -> return an empty speech string without error or side effects -> criterion:AC-001
- Invalid date `13/13/2026`, malformed ordinal `1th`, and unknown unit `5 xyz` -> preserve unsupported text rather than guessing -> criterion:AC-003
- Mixed valid and invalid spans, such as `11:32 PM; 13/13/2026; v1.2.3` -> normalize the valid clock while preserving the invalid date and version -> criterion:AC-004
- Context collisions `5 m`, `2 in the box`, and `8 oz` without context -> preserve the ambiguous abbreviation -> criterion:AC-007
- A later normalizer rule raises after an earlier span matched -> return the original complete string, not a partially changed prefix -> criterion:AC-010

## Out of scope
- Changing speech recognition, the voice model, model prompts, model reasoning, tool behavior, or the spoken answer's meaning.
- Unit conversion, automatic time-zone conversion, external services, additional AI models, or adding a third-party normalization dependency.
- Proving acoustic pronunciation or microphone behavior; this contract checks text sent to TTS, not generated sound quality.
- Speaking URLs, paths, or source code character by character; these are preserved as text.

## Decisions
- Locale `en-US` | Matches the accepted English baseline and resolves valid numeric dates deterministically | REVERSIBLE
- Plain alphabetic slash pair/list reads as alternatives; explicit `and/or` reads “A, B, or both”; recognized unit slash reads “per” | Separates meanings without a model and matches the accepted baseline | REVERSIBLE
- Generic slash lists require at least two letters per item; protect known `I/O`; preserve unknown one-letter forms | Avoids rewriting math, paths, and short identifiers | REVERSIBLE
- `m` requires duration/length context; ambiguous `in` and `oz` remain written | Prevents changing the intended unit or ordinary word | REVERSIBLE
- Preserve complete URLs, paths, code, and identifiers; do not strip useful path/query content | Prevents data loss in technical references | REVERSIBLE
- Keep only clear list/clause pauses and preserve the model's wording | Improves listening without adding meaning | REVERSIBLE
- No added dependency or model call | Keeps normalization local and deterministic | REVERSIBLE
