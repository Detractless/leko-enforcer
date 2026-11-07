# Contract: speech-cadence-and-emdash-pauses

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Implement natural acoustic breathing pauses for em-dashes and inter-sentence cadence in the speech verbalizer without changing raw UI text.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: Em-dashes and double hyphens are converted to natural acoustic breathing pauses (ellipsis) in the speech verbalizer.
  - evidence: tests/speech_cadence.test.mjs "em_dashes_convert_to_acoustic_breathing_pauses"
  - breaker: Em-dashes are spoken without pause or rushed into adjacent clauses.
- AC-002: Inter-sentence boundaries (periods, exclamation marks, question marks) maintain consistent cadence and separation.
  - evidence: tests/speech_cadence.test.mjs "sentence_boundaries_maintain_cadence_spacing"
  - breaker: Sentences run together without punctuation spacing, causing hurried TTS delivery.
- AC-003: Synthesis duration with em-dash breath pauses produces measurable timing expansion matching human speech cadence.
  - evidence: tests/speech_cadence.test.mjs "verbalized_speech_produces_natural_timing_expansion"
  - breaker: Audio duration with em-dashes remains compressed under 3.0 seconds for multi-clause utterances.
- AC-004: Raw text in the UI card and terminal remains pristine without inserted ellipses or punctuation tampering.
  - evidence: tests/speech_cadence.test.mjs "raw_ui_text_preserves_original_punctuation_and_em_dashes"
  - breaker: UI draft cards or query logs display acoustic pause formatting.

## Checklist coverage
- CLI-1 -> out-of-scope: Audio device streaming
- CLI-2 -> criterion:AC-001
- CLI-3 -> criterion:AC-003

## Edge cases
- Consecutive em-dashes or multiple hyphens -> AC-001
- Trailing exclamation mark followed by greeting emoji -> AC-002
- Short one-word responses -> AC-003

## Out of scope
- Retraining the acoustic TTS model.
- Adding third-party audio effects filters.

## Decisions
- Em-dash to ellipsis mapping | Ellipsis produces 300ms natural acoustic breath pause in Phonon vocoder without artificial silence splicing | REVERSIBLE
- Pre-TTS verbalizer isolation | Ensures UI typography preserves standard em-dash punctuation | REVERSIBLE
