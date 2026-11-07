# Contract: clean-source-summaries-and-phonetics

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Format tool summaries directly at the source into natural fluent English without artificial prefixes, and keep the verbalizer strictly focused on acoustic phonetics.

## Domains
Domains: cli

## Acceptance criteria
- AC-001: Tool summaries format CLI status and block actions directly into natural spoken sentences without artificial prefixes like 'Status checked:'.
  - evidence: tests/clean_summaries.test.mjs "tool_summaries_are_fluent_sentences_without_artificial_prefixes"
  - breaker: Tool execution outputs telegraphic strings like 'Status checked: Site reddit.com is closed'.
- AC-002: Model replies flow naturally from LLM directly to UI and speech without grammar rewriting or prompt micro-management.
  - evidence: tests/clean_summaries.test.mjs "model_replies_preserve_natural_intelligence_without_prompt_handcuffs"
  - breaker: Conversational replies are filtered or rewritten by artificial grammar rules.
- AC-003: Verbalizer acts strictly as an acoustic phonetics normalizer (units, abbreviations, markdown symbols) without mutating semantic sentence structure.
  - evidence: tests/clean_summaries.test.mjs "verbalizer_strictly_normalizes_phonetics_without_structural_rewriting"
  - breaker: Verbalizer attempts to guess and rephrase sentence predicates or insert artificial verbs.
- AC-004: End-to-end status query and block tool workflows produce fluent summaries for both UI card and Maeve speech.
  - evidence: tests/clean_summaries.test.mjs "e2e_status_and_block_workflows_deliver_clean_spoken_takes"
  - breaker: Discrepancy between UI card text and clean spoken take during status check or block drafting.

## Checklist coverage
- CLI-1 -> out-of-scope: Streaming audio playback
- CLI-2 -> criterion:AC-001
- CLI-3 -> criterion:AC-003

## Edge cases
- Status check for app vs website -> AC-001
- Model replying to general knowledge query -> AC-002
- Plain text sentence without abbreviations -> AC-003

## Out of scope
- Retraining the underlying LFM3 model.
- Altering the Rust core decision engine.

## Decisions
- Clean at the source | Formulate natural spoken English directly in tool handlers rather than patching text downstream with regex | REVERSIBLE
- Acoustic-only verbalizer | Limits regex normalizer strictly to what TTS cannot pronounce (symbols, abbreviations, units) | REVERSIBLE
