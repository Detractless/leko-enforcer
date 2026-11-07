# Contract: speech-verbalizer-and-pronunciation

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Implement a text verbalization and pronunciation normalizer for TTS that transforms markdown tables, symbols, and abbreviations into natural conversational speech without altering raw model text in the UI, and provide an interactive pronunciation review tool.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: Verbalizer translates abbreviations (e.g., etc., i.e., vs.), symbols, and unit durations into natural spoken English.
  - evidence: tests/speech_verbalizer.test.mjs "verbalizes_abbreviations_and_durations_for_speech"
  - breaker: Abbreviations are spoken verbatim letter-by-letter (such as "e dot g" or "E T C").
- AC-002: Markdown formatting (tables, pipes, dividers, backslashes, asterisks, bullet dashes) is stripped and restructured into conversational sentences.
  - evidence: tests/speech_verbalizer.test.mjs "strips_markdown_formatting_and_flattens_tables"
  - breaker: TTS pronounces pipe characters, markdown dividers, or backslashes out loud.
- AC-003: Model output remains untouched in UI bridge and CLI display, applying verbalization only to TTS synthesis.
  - evidence: tests/speech_verbalizer.test.mjs "preserves_raw_ui_text_while_verbalizing_audio"
  - breaker: Original raw model text in the UI card is mutated by the speech verbalizer.
- AC-004: Pronunciation review tool provides side-by-side text comparisons and audio preview takes.
  - evidence: tests/speech_verbalizer.test.mjs "review_runner_formats_comparison_and_synthesizes_audio"
  - breaker: Pronunciation review script fails to format side-by-side comparison or synthesize preview takes.

## Checklist coverage
- CLI-1 -> out-of-scope: Streaming audio playback
- CLI-2 -> criterion:AC-001
- CLI-3 -> criterion:AC-004

## Edge cases
- Markdown tables with headers and divider rows -> AC-002
- Numbers with units like '25m', '2h', '10s' -> AC-001
- URLs with https:// and trailing slashes -> AC-001

## Out of scope
- Changing LFM3 model prompts or generation outputs.
- Modifying UI font rendering.

## Decisions
- Pre-TTS verbalizer pipeline | Keeps UI text visually clean and formatted while ensuring audio sounds natural to human ears | REVERSIBLE
- Standalone review script | Lets user inspect transformations and listen to takes from the terminal | REVERSIBLE
