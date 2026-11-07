# Contract: rust-config-core

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
Leko Enforcer gets a Rust library that reads Enforcer settings files (format 2) as strictly as the phone does, and decides which changes land now and which must wait. Running `cargo test` proves it matches the phone on the phone's own test files: 172 zip cases, 27 canonical cases, 183 loosening cases, 28 landing-time cases and 2 partner-key vectors.

## Domains
Domains: data

## Acceptance criteria
- AC-001: Every one of the 172 corpus zips is accepted or refused exactly as the corpus says, including zips with an unknown file, an unknown field, a duplicate key, an out-of-range number or a never-blocked package.
  - evidence: core/tests/corpus.rs "corpus_matches_all_172_cases"
  - breaker: A reader that skips one strict rule (for example ignores an unknown field) accepts a file the phone refuses.
- AC-002: The corpus test fails if the corpus file is changed, truncated or replaced, because it checks the case count and the SHA-256 of the copied file.
  - evidence: core/tests/corpus.rs "corpus_file_is_the_pinned_copy"
  - breaker: A truncated corpus makes the test pass while it runs zero or few cases.
- AC-003: Reading each of the 27 canonical documents and writing it back gives exactly the canonical form, with every default filled in.
  - evidence: core/tests/canonical.rs "canonical_matches_all_27_cases"
  - breaker: A missed default (such as `dayStart` "04:00") or a dropped or reordered field.
- AC-004: For all 183 loosening cases, the judge says "lands now" or "waits" exactly as the case expects.
  - evidence: core/tests/loosening.rs "diff_matches_all_183_cases"
  - breaker: A field direction reversed, such as treating a raised minutes limit as stricter.
- AC-005: For all 28 landing-time cases, the moment a waiting change lands is exactly the expected time under the pinned zone, the 04:00 day start, waits, change windows and seals.
  - evidence: core/tests/loosening.rs "due_matches_all_28_cases"
  - breaker: A change window that crosses midnight, or a seal that ends a day early or late.
- AC-006: Both partner-key vectors verify, a wrong secret fails, and a secret typed in two Unicode forms (composed and decomposed accents) gives the same result.
  - evidence: core/tests/keys.rs "key_vectors_verify_and_normalise"
  - breaker: Hashing the secret without NFC normalisation, so the same word fails on another keyboard.
- AC-007: Only the time module reads the clock or the time zone; the judge takes the time and zone as input.
  - evidence: core/tests/no_clock_reads.rs "only_the_time_module_reads_the_clock"
  - breaker: A `SystemTime::now()` call added inside the judge, which makes tests and two devices disagree.
- AC-008: A refused zip is refused whole, and the message names the file and the field in plain words; a file stamped with a schema number other than 2 is refused with a message that says only format 2 is read.
  - evidence: core/tests/errors.rs "refusal_is_whole_and_names_file_and_field"
  - breaker: A zip with one good and one bad file partly accepted, or a generic "invalid" message.
- AC-009: Empty, truncated, non-UTF-8 and very deeply nested input never crashes the library; each gives a refusal, never a panic.
  - evidence: core/tests/hostile_input.rs "hostile_input_is_refused_not_a_panic"
  - breaker: An `unwrap` on malformed bytes aborts the process, so a damaged settings file stops enforcement.

## Checklist coverage
- DATA-1 -> out-of-scope: this feature only reads and judges; it writes no file. Saving is the store in Phase 2.
- DATA-2 -> criterion:AC-009
- DATA-3 -> criterion:AC-008

## Derived checklist
(not used: Domains is not "none")

## Edge cases
- A zip with only GUIDE.md and no settings file -> AC-001
- A zip with a file the format does not list, such as `notes.txt` -> AC-001
- The first file in a zip is good and the second is bad (it fails halfway) -> AC-008
- A number just past its limit, such as minutes 1000001 -> AC-001
- A change window that crosses midnight, with the clock at the 04:00 day start -> AC-005
- A secret with an accented letter typed as one character or as two -> AC-006
- A JSON file nested thousands of levels deep -> AC-009
- Two devices judging the same change at slightly different times -> out-of-scope: syncing devices is Phase 6.

## Out of scope
- Writing or saving any file, and the wait queue, signed change log, plan and commit with a fingerprint (separate contract: plan-commit-queue).
- Reading the clock or touching the operating system (ports are Phase 2).
- Any CLI, voice, model or UI.
- New fields: site coverage, timed sessions, bundle ids (separate contract for format 2.1; see feature-map.md, section 6).
- Windows and Android builds.

## Decisions
- Port format 2 exactly as the phone reads it today and add no field in this feature | any change breaks equality with the phone's own test files; extending comes in its own contract | REVERSIBLE
- One library crate named `core`, strict JSON through `serde_json` with unknown fields denied, plus the `zip`, `pbkdf2`, `sha2` and `unicode-normalization` crates | small, well-known crates that match how the phone does each step | REVERSIBLE
- The five shared JSON files are copied unchanged into `vectors/`, with a pinned hash | the tests cannot drift from the phone's | REVERSIBLE
- Time and zone are plain arguments, never read inside the library | plan rule 4 (only the time module reads the clock) | REVERSIBLE
- Tests run with `~/.cargo/bin/cargo test` | cargo 1.98.1 is installed but its folder is not on the shell PATH | REVERSIBLE
