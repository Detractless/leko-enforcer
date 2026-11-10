# Voice Speech Conversion Rules

**Status:** Research snapshot and rule reference. The baseline table below describes behavior before the speech-normalization contract; see the implementation update before treating proposed rules as pending.

## Contract implementation update — 2026-11-10

The accepted deterministic baseline is implemented under [the speech-normalization contract](../.agent/contracts/speech-normalization.md). The implementation lives in `voice/normalization/speech_text.py`, called through `voice/verbalizer.py`; it changes only TTS text and preserves protected technical spans. Contract coverage is in `tests/speech_verbalizer.test.mjs` and `tests/speech_cadence.test.mjs`.

It currently handles en-US dates, calendar-context clock times and weekday abbreviations, typed numbers and common fractions, the contracted duration and measurement inventory, contextual `m`/`in`/`oz`, `w/` and `w/o`, computing-context `I/O`, alternative slash lists, and explicit `and/or`. Ambiguous or malformed forms are preserved. This is text-level evidence; it does not certify the TTS engine's acoustic pronunciation.

Follow-up speech rules are also implemented: prose colons before whitespace use the existing ellipsis pause, recognized clock ranges say “to,” and bare `m` uses local duration-versus-measurement cues with an ambiguity fallback. Focused tests cover these rules and protected clock/technical colons. The schedule-colon example was listened to by the owner; other colon contexts still need acoustic review.

## Goal

Make spoken replies easy to understand while preserving the model's wording in the UI, transcript, memory, and model context. The verbalizer must not call another model or change the meaning of an answer.

## Current pipeline

1. The model's reply is recorded and sent to the UI as written.
2. `voice/verbalizer.py` prepares a separate text string for speech synthesis.
3. `voice/assistant.py` calls the local TTS engine with voice `Zoey`; the app does not pass a speech-rate option.
4. Session logs save the exact text sent to TTS and playback timing, but not the generated audio. A log can show what was requested to be spoken, not prove how a sound was pronounced.

**Implementation note:** The normalizer is called in `voice/session/query_flow.py` and again in `voice/assistant.py`. Existing conversions appear mostly idempotent; check this wiring before adding transformations that can apply twice.

## Pre-contract implementation baseline (research snapshot)

These rules are in `voice/verbalizer.py` and covered in part by `tests/speech_verbalizer.test.mjs` and `tests/speech_cadence.test.mjs`.

| Area | Current behavior |
|---|---|
| Markdown | Removes code fences, inline-code marks, emphasis, heading and list markers; flattens Markdown tables into spoken rows. |
| Symbols | Turns arrows into “meaning”; strips several brackets, backslashes, pipes, and emoji; normalizes spacing around punctuation. |
| URLs | Removes a URL scheme and path, then speaks supported domain endings such as `.com` as “dot com.” |
| Abbreviations | `e.g.` → “for example”; `i.e.` → “that is”; `etc.` → “and more”; `vs.` → “versus”; `w/o` → “without”; `w/` → “with” when followed by whitespace or end of text. |
| Durations | `h/hr/hrs`, `m/min/mins`, and `s/sec/secs` after a number become hour, minute, and second words with basic singular/plural handling. |
| Slash | `and/or` becomes “and or.” A single alphabetic `word/word` becomes “word and or word.” That generic rule also alters `I/O`, `km/h`, and `m/s`; the tests do not cover those collisions. `w/coffee` misses the explicit `w/` rule and falls into the generic slash rule. Numeric fractions/dates are not matched by that alphabetic rule; `/Users/me/file` is covered as preserved. A chain such as `lock/logoff/shutdown` is not handled as a list. |
| Dashes | Em dashes, en dashes, and double hyphens become an ellipsis pause (`...`). |

Current tests explicitly expect `block/unblock` → “block and or unblock,” and preserve `1/2`, `10/20/2026`, and `/Users/me/file`. The only current unit-like conversions are time-duration suffixes (`h`, `m`, `s` and variants); `m` is treated as minutes, not meters. There are no current speech mappings for milliseconds, kilometers per hour, inches, feet, or miles. The rules also do **not** convert clock times, written dates, weekday abbreviations, or `I/O` to expanded speech.

## Recent observed issues

Evidence: local session `~/Library/Application Support/Leko/session-logs/2026/11/09/session-e475afdfe46f45a382b4fd0f238c8014.jsonl`.

1. **Clock and date pronunciation:** The model returned `It's 11:32 PM on Monday, November 09, 2026.` The TTS input kept `11:32 PM` and the zero-padded `09`. The user heard the time as an “O” or zero. The log cannot confirm the exact sound because it stores text and timing, not audio. “Monday” was already fully spelled in the model text; if it sounded like “mun,” that is a TTS pronunciation issue rather than an `MO` abbreviation in this response.
2. **Rushed list:** A capability reply contained about 61 words in one long sentence with several lists, a dash, a semicolon, and `(lock/logoff/shutdown)`. TTS completed it in about 22.3 seconds (roughly 164 words per minute). The dash became `...`, but the semicolon and slash chain remained. The dense structure likely made the list harder to follow even if the overall rate was not extreme.
3. **Recognition is a separate issue:** In the same session, STT produced `That I look walk.` This was the recognized input, not a text-conversion error. Its reply was interrupted after about 2.8 seconds when another capture started.

## Research proposals and external comparisons

Apply rules in this order so a general slash rule cannot damage a date, unit, URL, or acronym.

### 1. Protect structured text, then use explicit conversions

- Recognize URLs, domains, file paths, numeric dates, fractions, clock strings, and known technical abbreviations before generic slash handling.
- Keep the original model/UI text unchanged. Only the TTS-bound string gets normalized.
- Use a small explicit vocabulary for abbreviations and units rather than a general “slash means a word” rule.

| Input | Proposed spoken form | Notes |
|---|---|---|
| `80 km/h` or `80 km/hr` | “80 kilometers per hour” | Treat as a unit, not an option list. Use “1 kilometer per hour” for one. |
| `w/ coffee` or `w/coffee` | “with coffee” | Expand the recognized abbreviation even when there is no space after `/`. |
| `w/o coffee` | “without coffee” | Current rule already covers the spaced example. |
| `I/O` | “input and output” | In a computing context. Keep an exception for unrelated identifiers or literal code. |
| `11:32 PM` | “eleven thirty-two p.m.” | Do not let TTS guess how to read the digits and colon. |
| `November 09, 2026` | “November ninth, twenty twenty-six” | Remove the leading zero and make the day ordinal. |
| `Mon, Tue, Wed` | “Monday, Tuesday, Wednesday” | Expand only in a recognized weekday/date/schedule context. The latest sample already said “Monday.” |

Numeric dates such as `11/09/2026` are locale-ambiguous. Preserve them unless the user's locale/date format is known; then speak the full month and ordinal day. Do not replace a weekday-looking substring inside an ordinary word.

### Measurement coverage: speak the unit, do not convert the measurement

The speech layer should expand a written measurement into natural words while keeping its value and unit system unchanged. For example, `5 mi` becomes “five miles,” not a converted kilometer value. Unit conversion between imperial and metric systems is a separate user-requested operation.

| Priority | Family | Candidate forms and examples |
|---|---|---|
| First | Time | `ms` → milliseconds; `µs` → microseconds; `s/sec` → seconds; `min` → minutes; `h/hr` → hours. Keep `m` context-sensitive because it can mean meter or minute. |
| First | Length | `mm`, `cm`, `m`, `km`; `in`/`inch`, `ft`/`foot`, `yd`/`yard`, `mi`/`mile`. Say “one foot” but “two feet”; “one inch” but “two inches.” |
| First | Speed | `km/h`, `km/hr` → kilometers per hour; `mph` → miles per hour; `m/s` → meters per second. A slash means “per” only inside a recognized compound unit. |
| Next | Mass | `mg`, `g`, `kg`, `oz`, `lb/lbs`. `oz` may mean mass or fluid ounces; use `fl oz` for fluid volume. |
| Next | Volume | `mL`, `L`, `cup`, `pt`, `qt`, `gal`, `fl oz`; expand to milliliters, liters, cups, pints, quarts, gallons, and fluid ounces. |
| Next | Temperature and area | `°C`, `°F` → degrees Celsius/Fahrenheit; `ft²`, `m²`, `sq ft`, and `sq mi` → square units. Add cubic units only with explicit rules. |

Treat the displayed number as data, not a cue to convert units. Normalize decimals, signs, fractions, ranges, and compound measurements only when the syntax is clear: `2.5 km`, `-5 °C`, `5–10 ft`, and `5 ft 6 in` are different cases. Avoid generic substitutions for `in`, `m`, `oz`, `C`, or `s`: each can also be an ordinary word, letter, or different unit.

### 2. Use one generic rule for ordinary word lists

- First match exact known abbreviations such as `km/h`, `w/`, and `I/O`.
- For a plain alphabetic slash sequence, require each item to have at least two letters. Exclude digits, URL/path context, and protected abbreviations.
- One pair `A/B` becomes “A or B.” Three or more items `A/B/C` become “A, B, or C.” Do not infer “and” from older conversation context; that would be brittle and can change meaning.
- Treat explicit `A and/or B` as inclusive and say “A, B, or both.” Do not read the literal phrase “and or.”
- Plain “or” is the fallback because slash alone does not say that both items are required. If both must be explicit, the source text should say `and/or` or “or both.”

Examples: `lock/logoff/shutdown` → “lock, log off, or shut down”; `cookies/milk` → “cookies or milk”; `cookies and/or milk` → “cookies, milk, or both.” The slash alone cannot disambiguate whether a pair is exclusive or inclusive.

### 3. Make long spoken lists easier to follow

- Keep model text and on-screen text unchanged; build a separate spoken presentation.
- Convert clear slash lists into comma-separated alternatives with “or” before the final item.
- Use a full pause at a semicolon or when a dash separates two complete clauses. Do not add pauses after every comma; that would make speech choppy.
- Do not paraphrase arbitrary answers with regexes. If a reply is still too long, test a deterministic sentence-splitting rule or a supported TTS pause/rate control separately. The current TTS call supplies no rate argument, and support for explicit pause markup has not been verified.

Example from the observed reply, as a clearer spoken rendering while leaving the model/UI text intact:

> I can block apps, websites, or your whole computer. For a computer block, choose to lock it, log out, or shut it down. I can also set schedules, check what's blocked, and start focus sessions. What would you like to do?

This example is a target for a future voice presentation step; it is not an implemented rewrite rule yet.

## Existing projects and what to borrow

Research found several related projects. This is a fit review, not an adoption decision.

| Project | Relevant approach | Fit for Leko |
|---|---|---|
| [NVIDIA NeMo Text Processing](https://github.com/NVIDIA/NeMo-text-processing) | Rule-based text normalization with typed categories for numbers, dates, times, measures, money, and electronic strings. Its English measure grammar and unit table cover compound units and “per” forms. | Best reference for rule ordering, typed spans, and coverage tests. Its Pynini dependency makes it a poor direct install for this macOS app; borrow the taxonomy and cases instead. NeMo's TTS docs describe text normalization as preprocessing before synthesis. |
| [text2spoken](https://github.com/TigreGotico/text2spoken) | A small Python normalizer with separate date/time, measure, ordinal, and number passes; locale settings and a custom lexicon; unknown forms can remain unchanged with a warning. | Closest whole-text prototype candidate and reports no model/network requirement. It is very new, has a small maintainer base, and its README says its English test examples have not had native-speaker review. Compare it against Leko fixtures before considering it as a dependency. |
| [OpenVoiceOS ovos-date-parser](https://github.com/OpenVoiceOS/ovos-date-parser) and [ovos-number-parser](https://github.com/OpenVoiceOS/ovos-number-parser) | Plain-Python helpers for speaking structured dates, times, durations, numbers, ordinals, and fractions. | Useful smaller building blocks for structured values, but they do not normalize arbitrary assistant prose or provide the whole unit/slash policy. |
| [Pint](https://pint.readthedocs.io/en/latest/) | Parses and operates on physical quantities and unit systems. | Not a TTS verbalizer. Avoid adding it just to say `km/h` aloud, and do not convert values as part of speech formatting. |

**Research recommendation:** Keep the no-extra-model design. Prototype `text2spoken` on the same fixed phrase suite as a comparison, and borrow NeMo's typed-span organization and measure cases. Do not adopt either until local macOS installation, offline behavior, output quality, and edge cases pass that suite. Implementing a small explicit rule set remains the lightest path if the comparison library adds dependencies or rewrites text unexpectedly.

### Reuse and provenance

NeMo Text Processing is open source under Apache-2.0. Its license permits reuse subject to the license terms; redistribution of copied code or derived files carries notice and attribution conditions, including retaining the license and applicable NOTICE material. Open-source projects do not all use the same license. For this effort, prefer to study published behavior and independently write Leko-specific rules. If we copy grammar code, unit tables, examples, or tests, record the source and follow that source's license. We do not need to import the complete NeMo library just to borrow its rule organization.

## Meta-prompt for a future rule review

Use this prompt after reviewing the rule inventory, before any code change:

> You are reviewing Leko's local, English, speech-only text normalization layer. Your goal is to propose precise deterministic rules that make the assistant's spoken replies easier to understand while leaving the model's reply, UI text, transcript, memory, and model context unchanged.
>
> First inspect `docs/voice-speech-conversion-rules.md`, `voice/verbalizer.py`, its tests, and a recent local session log. Separate current behavior, observed failures, research findings, and new proposals. Never describe a proposal as implemented.
>
> Compare relevant open-source text-normalization projects, especially NVIDIA NeMo Text Processing, `text2spoken`, and the OpenVoiceOS date/number parsers. Use project documentation, source files, test fixtures, and license files as primary evidence. For each borrowed idea, identify the project, exact source, license, and whether the proposal reuses only a general approach or copies code/data/examples. Do not assume open source means license-free. Do not add a package or copy third-party files as part of this review.
>
> Cover, at minimum: cardinal and ordinal numbers; decimals, fractions, signs, ranges, dates, times, time zones, and weekdays; duration units; length, mass, volume, speed, temperature, and area; unit singular/plural and compound forms; abbreviations and acronyms (`w/`, `w/o`, `I/O`); slash semantics (`A/B`, `A/B/C`, `and/or`, and unit slash meaning “per”); URLs, domains, file paths, code, identifiers, Markdown, punctuation, and long spoken lists. Include collisions such as `m` for minute or meter, `in` as a preposition or inch, `oz` as mass or fluid volume, and numeric slash dates versus fractions.
>
> Keep speech normalization deterministic and local; do not call an additional model. Never convert a quantity into another unit system unless the user asked for conversion. Prefer typed/context-aware recognition, exact known mappings, and safe preservation when ambiguous. Do not use a blanket slash replacement or blanket number-to-words pass.
>
> Deliver: (1) a table of each rule with current behavior, external precedent, proposed output, ambiguity/fallback, source/license, and test cases; (2) a prioritized support inventory; (3) a phrase suite that can compare current output with candidate libraries; (4) unresolved decisions and risks; (5) a recommendation. Do not edit code. Stop after the research and proposed test plan so the owner can review it.

Sources: [NeMo English measure grammar](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/measure.py), [NeMo unit inventory](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/data/measure/unit.tsv), [NeMo TTS normalization setup](https://docs.nvidia.com/nemo-framework/user-guide/24.09/nemotoolkit/tts/configs.html), [NeMo Apache-2.0 license](https://github.com/NVIDIA/NeMo-text-processing/blob/main/LICENSE), [NeMo macOS Pynini packaging note](https://docs.nvidia.com/nemo/curator/v26.07/about/release-notes/26-07-migration), [text2spoken API, coverage, and release details](https://pypi.org/project/text2spoken/), [OVOS date parser](https://github.com/OpenVoiceOS/ovos-date-parser), [OVOS number parser](https://github.com/OpenVoiceOS/ovos-number-parser).

## Edge cases to preserve or resolve

| Case | Risk | Draft handling |
|---|---|---|
| `11/09/2026`, `1/2`, `3/4` | Dates and fractions could turn into “eleven or nine” or “one or two.” | Protect digit-containing slash expressions; date conversion needs locale. |
| `km/h`, `m/s`, `mph`, `°C` | Units are not alternatives. | Add explicit unit mappings and test singular/plural values. |
| `m`, `in`, `oz`, `s` | Short symbols collide with ordinary words or multiple units. | Require a numeric/unit context and use typed units; do not blindly expand a token. |
| `1 ft`, `2 ft`, `5 ft 6 in` | Irregular plurals and compound heights defeat simple suffix rules. | Use quantity-aware unit forms and explicit compound-measure patterns. |
| `2.5 km`, `-5 °C`, `5–10 mi`, `1/2 in` | Decimals, signs, ranges, and fractions need different number readings. | Normalize by measurement class; preserve unsupported syntax rather than guess. |
| `5 km` vs `3.1 mi` | A speech conversion might accidentally change the measurement system or value. | Only expand words; never convert quantities between units unless requested. |
| `I/O`, `R/W`, `TCP/IP` | Acronyms can mean a known phrase or a literal identifier. | Expand only known items in the right context; otherwise preserve. |
| `w/`, `w/o` | The current `w/` rule misses `w/coffee`. | Accept the no-space form only when it is a recognized abbreviation, not in URLs or paths. |
| `and/or` | “And or” is awkward and may be misunderstood. | Render an explicit pair as “A, B, or both.” |
| `on/off`, `read/write`, `input/output` | A pair can mean alternatives or a combined capability. | Default ordinary slash pairs to “or”; add a specific dictionary entry only where the combined meaning is established. |
| One-letter forms such as `I/O`, `x/y`, `a/b` | A general word-length rule could damage acronyms or math. | Handle recognized acronyms first; leave short ambiguous tokens unchanged. |
| Three or more words, e.g. `lock/logoff/shutdown` | A two-word regex leaves a chain untouched. | Parse the full chain and join it as a spoken list. |
| Hyphens, apostrophes, capitalization, punctuation around slash | Token boundaries may be misdetected. | Add tests before broadening the candidate grammar; preserve unknown forms. |
| Long comma-heavy prose | Global comma pauses can sound robotic; no rewrite may remain rushed. | Change only clear list/clause boundaries and compare actual spoken takes. |

## Draft test matrix for implementation

Add tests for exact spoken outputs and unchanged protected cases before changing code:

- Times, textual dates, ordinal days, years, full weekdays, and weekday abbreviations in schedule context.
- `km/h`, `km/hr`, one versus plural quantities, `w/` with and without a space, and contextual `I/O`.
- Unit families by priority: milliseconds/seconds/minutes/hours; metric and imperial length; speed; then mass, volume, temperature, and area.
- Singular/plural and irregular forms (`1 foot`/`2 feet`, `1 inch`/`2 inches`), decimals, negative values, ranges, fractions, and compound heights.
- Verify speech-only output never changes the numeric value or converts miles to kilometers, feet to meters, or vice versa.
- `A/B`, `A/B/C`, explicit `A and/or B`, and mixed-case/punctuation boundaries.
- Fractions, numeric dates, URLs, domains, paths, acronyms, and one-letter tokens remaining intact.
- Long-list speech text differing from the original while UI/model text stays identical.
- A real listening comparison for date pronunciation and list pacing; text-only tests cannot establish what Zoey actually sounds like.

## Current decisions and open questions

**Working direction:** no extra decision model; preserve model/UI wording; use explicit mappings before generic slash handling; default ordinary slash alternatives to “or”; render explicit `and/or` as “or both.”

**Still to settle before implementation:** date locale behavior; the exact unit vocabulary and ambiguity rules (`m`, `in`, `oz`); whether to adopt a dependency or borrow its rules; how to recognize weekday abbreviations without rewriting normal words; sentence-splitting threshold for long answers; whether Zoey supports a reliable rate or pause control; and whether to normalize once or keep the current double call.

## Source-backed rule-by-rule review

Reviewed 2026-11-10. This section separates the observed implementation from proposals. The proposals are not implemented. External project references are used for general design ideas and comparison cases only; no source code, data file, test, or example has been copied into Leko.

### Source and license key

| Key | Evidence inspected | License and reuse boundary |
|---|---|---|
| **N — NeMo Text Processing** | [English cardinal](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/cardinal.py), [ordinal](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/ordinal.py), [fraction](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/fraction.py), [date](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/date.py), [time](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/time.py), [measure grammar](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/measure.py), [unit table](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/data/measure/unit.tsv), [time-zone table](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/data/time/zone.tsv), [electronic grammar](https://github.com/NVIDIA/NeMo-text-processing/blob/main/nemo_text_processing/text_normalization/en/taggers/electronic.py), [English fixture directory](https://github.com/NVIDIA/NeMo-text-processing/tree/main/tests/nemo_text_processing/en/data_text_normalization) including [fraction](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_fraction.txt), [decimal](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_decimal.txt), [ordinal](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_ordinal.txt), [date](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_date.txt), [measure](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_measure.txt), and [electronic](https://raw.githubusercontent.com/NVIDIA/NeMo-text-processing/main/tests/nemo_text_processing/en/data_text_normalization/test_cases_electronic.txt) cases; and [README/install notes](https://github.com/NVIDIA/NeMo-text-processing/blob/main/README.md). | Apache-2.0. Proposed reuse is the typed-category and ordered-match approach only. No NeMo grammar, table, code, or tests are copied. Its README says pip install on macOS/Windows is unsupported without building Pynini/OpenFst or using conda-forge. |
| **T — text2spoken** | [README/API](https://github.com/TigreGotico/text2spoken/blob/dev/README.md), [English rules](https://github.com/TigreGotico/text2spoken/blob/dev/text2spoken/lang/en.py), [English fixture](https://github.com/TigreGotico/text2spoken/blob/dev/tests/data/en.tsv), [PyPI release metadata](https://pypi.org/project/text2spoken/), and [license](https://github.com/TigreGotico/text2spoken/blob/dev/LICENSE). | Apache-2.0. Proposed reuse is the span analysis, locale parameters, and conservative fallback ideas only. No package or fixture is added. The PyPI release inspected is a pre-release, Python 3.11+, one maintainer; the README says its English fixture rows have not been checked by native speakers. |
| **OD — OVOS date parser** | [README/API and runnable examples](https://github.com/OpenVoiceOS/ovos-date-parser), [English formatter source](https://github.com/OpenVoiceOS/ovos-date-parser/blob/dev/ovos_date_parser/dates_en.py), [English formatting tests](https://raw.githubusercontent.com/OpenVoiceOS/ovos-date-parser/dev/test/format_tests/test_format_en.py), [regional date test](https://github.com/OpenVoiceOS/ovos-date-parser/blob/dev/test/test_nice_date_region_tag.py), and [license](https://github.com/OpenVoiceOS/ovos-date-parser/blob/dev/LICENSE). | Apache-2.0. Proposed reuse is the separation of structured date/time/duration formatting from free text. No code or tests are copied. |
| **ON — OVOS number parser** | [README](https://github.com/OpenVoiceOS/ovos-number-parser/blob/dev/README.md), [API](https://github.com/OpenVoiceOS/ovos-number-parser/blob/dev/docs/api.md), [English number source](https://github.com/OpenVoiceOS/ovos-number-parser/blob/dev/ovos_number_parser/numbers_en.py), and [license](https://github.com/OpenVoiceOS/ovos-number-parser/blob/dev/LICENSE). | Apache-2.0. Proposed reuse is the number/ordinal API concept only. Its API docs say the dedicated `pronounce_fraction` implementations are currently Portuguese and Mirandese; do not assume complete English fraction support. No code or data is copied. |

NeMo’s measure grammar explicitly treats `/` as “per” inside a recognized unit expression and has typed handling for number, date, time, measure, fraction, and electronic spans. `text2spoken` likewise analyzes spans, has locale/date-order parameters, and leaves forms it cannot confidently read unchanged with a warning. Its English rule file has metric mass/length/speed/volume/temperature and data-unit entries such as `km/h`, `m/s`, and `mph`; the reviewed map does not include the requested US customary, broad liquid-volume, or area inventory. Its published English fixture covers dates, times, fractions, ranges, measurements, and ambiguity cases, but its own `checked_by` column is `unchecked`. OVOS provides focused Python formatters for structured values, not a general arbitrary-prose normalizer.

### Rule matrix

“Current” describes `voice/verbalizer.py` and its existing fixtures, not a live acoustic result. A target example is a proposal only. `N`, `T`, `OD`, and `ON` refer to the source/license key above.

| Rule | Current behavior | External precedent and license | Proposed spoken output | Ambiguity and fallback | Proposed test cases |
|---|---|---|---|---|---|
| Cardinal numbers | Digits stay digits except the integer suffixes handled as durations. | N cardinal grammar; T English speller/fixtures; ON `pronounce_number`; all Apache-2.0. General approaches only. | `1,234 apps` → “one thousand two hundred thirty-four apps” under explicit `en-US`. Convert only a confidently typed number span. | Do not turn versions, IDs, phone numbers, room labels, hashes, or code into cardinals. Preserve unsupported or untyped digits. | `1,234 apps`; `Room 12`; `v1.2.3`; `X5`; `0012`. |
| Ordinals | No numeric ordinal conversion. | N ordinal/date grammars; T ordinal suffix fixtures; ON ordinal API; Apache-2.0. General approaches only. | `21st place` → “twenty-first place”; `November 9` → “November ninth.” | Validate suffix (`1st`, `2nd`, `3rd`, `4th`; `11th`–`13th`). Preserve malformed/identifier forms. | `1st`, `2nd`, `3rd`, `11th`, `21st`, `1th`, `1st5`. |
| Decimals and signs | Decimal values and `+`/`-` are unchanged. An en dash in a range is changed to an ellipsis later. | N decimal/cardinal grammar; T `decimal_mark`/sign rules and fixture; ON number API; Apache-2.0. General approaches only. | `-5.2 °C` → “minus five point two degrees Celsius”; keep every fractional digit, including meaningful trailing zeroes. | Decimal comma depends on locale. Do not reinterpret a dotted version, IP, or identifier as a decimal. Preserve mixed/invalid forms. | `3.50`; `.5`; `-5.2`; `1.2.3`; `1,234.5` and locale variant `1.234,5`. |
| Fractions | Numeric slash forms are preserved; no speech conversion. | N fraction grammar; T fraction fixtures and slash guards; ON fraction helpers (English limitation noted above); Apache-2.0. General approach only. | `1/2 cup` → “one-half cup”; `3/4 inch` → “three-quarters of an inch.” | Convert only when the fraction grammar or a unit/count noun makes it clear. A ratio, score, division expression, or identifier remains literal. | `1/2 cup`; `3/4 inch`; `3/2 ratio`; `1/2 kg`; `10/20/2026`. |
| Numeric dates and date order | Dates are not normalized. `November 09, 2026` remains zero-padded. | N date grammar; T locale-controlled `MDY`/`DMY`/`YMD`; OD `nice_date`; Apache-2.0. General approach only. | With configured `en-US`, `November 09, 2026` → “November ninth, twenty twenty-six”; `2026-11-09` → “November ninth, twenty twenty-six.” | `03/04/2026` is ambiguous without a configured locale. Preserve it if locale is missing or format invalid; never guess from surrounding prose alone. | `03/04/2026` under US/GB settings; `2026-11-09`; invalid `13/13/2026`; `11/09` without year. |
| Clock times | Clock digits, colon, AM/PM, and zone stay as written. | N `time.py` plus `zone.tsv`; T locale clock rules and fixture; OD `nice_time`; Apache-2.0. General approaches only. | `11:32 PM` → “eleven thirty-two p.m.”; `6:02` → “six oh two” when a clock context is certain. | Protect ratios, scripture/citations, durations, and timestamps. The NeMo and T examples themselves show that context/evidence is needed for colon forms. | `11:32 PM`; `6:02 tomorrow`; `16:9 ratio`; `John 3:16`; `05:12 lap time`. |
| Time zones | Unchanged; no zone mapping. | N accepts a bounded time-zone table after a recognized time; T README/API does not document zone normalization; Apache-2.0 for N/T. General approach only. | `12:30 a.m. EST` → “twelve thirty a.m. E S T” only when the exact recognized zone is attached to a time. | Many abbreviations collide (`CST`, `IST`). Do not expand to a geographic region or seasonal rule. Preserve unknown zones. | `12:30 a.m. EST`; `12:30 CST`; `CST meeting`; `11:32 PM UTC`. |
| Weekdays and short forms | Full weekday names are untouched; abbreviations are not expanded. | OD date formatter emits full weekday names; T named-date parser handles named dates. No direct precedent found for general weekday-abbreviation expansion; Apache-2.0 for OD/T. | In a clear schedule/date list, `Mon, Tue, Wed` → “Monday, Tuesday, Wednesday.” | Expand only in a recognized weekday list/date/schedule span. Keep ordinary words and labels (`Monument`, `mon`, code) unchanged. | `Mon, Tue, Wed`; `Mon-Fri`; `Monument is open`; `mon` in a URL/path. |
| Duration units | Integer `h/hr/hrs`, `m/min/mins`, `s/sec/secs` expand with singular/plural. `m` always means minutes. No `ms`. | N measure grammar/unit table; T English unit map; OD `nice_duration`; Apache-2.0. General approach only. | `25m` in a focus-session context → “twenty-five minutes”; `1 ms` → “one millisecond.” | Treat bare `m` as a minute only with duration evidence; for dimension/length evidence use meter. If context is weak, preserve `m`. | `25m focus session`; `5 min`; `1 ms`; `5 m tall`; `5 m` alone. |
| Length | No length mappings. Generic slash handling can corrupt `m/s`. | N measure grammar/unit table; T metric map (`m`, `cm`, `mm`, `km`) and fixture; Apache-2.0. General approaches only. | `1 m` → “one meter”; `2 ft` → “two feet”; `5 ft 6 in` → “five feet six inches.” | Use a bounded unit dictionary and number/unit span. `in` is a preposition unless it is clearly a unit after a quantity; preserve uncertain cases. | `1 m`; `2 m tall`; `2 in the box`; `2 in wide`; `5 ft 6 in`; `3 miles`. |
| Mass | No mappings. | N measure grammar/unit table; T has kg/g/mg in its English map; Apache-2.0. General approaches only. | `1 kg` → “one kilogram”; `2 lb` → “two pounds”; preserve the unit system and value. | Bare `oz` can be mass or fluid volume. Use food/weight context for mass or preserve if unclear; never infer a conversion. | `1 kg`; `2 lb`; `8 oz` after “weighs”; `8 oz` without context. |
| Volume | No mappings. | N measure grammar and release examples include volume; T English map has `ml` but not broad household volume; Apache-2.0. General approach only. | `1 L` → “one liter”; `2 cups` → “two cups”; `8 fl oz` → “eight fluid ounces.” | Reserve `fl oz` for volume. Bare `oz` is ambiguous. Do not convert cups, liters, pints, or gallons. | `1 mL`; `2 L`; `1 cup`; `1 qt`; `8 fl oz`; ambiguous `8 oz`. |
| Speed and compound units | No unit-aware slash handling: `km/h` and `m/s` currently become `km and or h` and `m and or s`. | N `measure.py` converts slash to “per” inside a known compound unit; T maps `km/h`, `m/s`, `mph`; Apache-2.0. General idea only. | `80 km/h` → “eighty kilometers per hour”; `5 m/s` → “five meters per second.” | `/` means “per” only after exact recognition of a supported unit pattern. Unknown technical expressions are preserved. | `80 km/h`; `5 m/s`; `60 mph`; `input/output`; `a/b`. |
| Temperature | No mappings. | N measure grammar/table; T maps `°C`/`°F` in its English data and fixture; Apache-2.0. General approaches only. | `-5 °C` → “minus five degrees Celsius”; `72 °F` → “seventy-two degrees Fahrenheit.” | Do not expand bare `C`/`F` without the degree mark or a known unit context. Keep values and scale unchanged. | `0 °C`; `72°F`; `-5 °C`; `C code`; `room F`. |
| Area | No mappings. | N release notes show per-unit area cases such as square feet; N measure grammar/unit table; T reviewed English map has no area unit; Apache-2.0. General approach only. | `5 sq ft` / `5 ft²` → “five square feet”; `2 m²` → “two square meters.” | Recognize explicit square marker or `sq` prefix. Do not interpret `ft2` or superscript-like code as area unless the unit shape is exact. | `5 sq ft`; `5 ft²`; `2 m²`; `ft2 identifier`. |
| Plural and compound unit forms | Only basic singular/plural for existing durations. | N measure grammar has singular/plural unit paths; T maps one/other forms; Apache-2.0. General approaches only. | `1 foot`, `2 feet`; `1.0 meters` (precision-bearing decimal); `1 h 30 min` → “one hour thirty minutes.” | Preserve precision: `1.0` may need plural even though its numeric value is one. Define irregular unit forms explicitly; never do string `s` suffixing blindly. | `1 ft`, `2 ft`, `1.0 m`, `1 h 30 min`, `0 s`, `1.5 kg`. |
| `w/`, `w/o`, and acronyms | `w/o` → “without”; `w/` → “with” only before whitespace/end. `I/O` currently falls through to “I and or O.” | N explicit whitelist concept; T does not map these forms in reviewed English data; Apache-2.0 for N/T. Specific wording is Leko policy, no copied lexicon. | `w/coffee` → “with coffee”; `w/o sound` → “without sound”; `I/O device` → “input/output device” only in computing context. | Do not globally expand `I/O`; protect paths, code, and unknown initialisms. Keep single-letter slash forms unless exact context rule applies. | `w/ coffee`; `w/coffee`; `w/o sound`; `I/O device`; `I/O` in code; `a/b`. |
| Plain slash pair/list | `and/or` → “and or”; one alphabetic pair → “and or”; three-item chains are untouched. Tests encode the awkward pair output. | N/T support typed special cases (units, fractions, dates, explicit idioms) but do not prescribe general option-list semantics; Apache-2.0. Proposed list rule is Leko policy, not borrowed code/data. | Default `A/B` → “A or B”; `A/B/C` → “A, B, or C”; `A and/or B` → “A, B, or both.” | Slash does not tell us whether both are required. Ordinary slash defaults to alternatives; require explicit “and/or” or “or both” for inclusive meaning. Protect URLs, fractions, dates, units, identifiers first. | `cookies/milk`; `lock/logoff/shutdown`; `A and/or B`; `R/W`; `24/7`; `50/50`; `1/2`. |
| URLs and domains | Scheme and URL path are stripped; supported TLDs such as `.com` become “dot com.” | N has a dedicated electronic grammar; T says unsupported forms remain written, with no URL rules in the reviewed English map; Apache-2.0. General typed-span idea only. | Speak a recognized domain as “reddit dot com.” Do not drop URL path/query text when it carries the referenced resource; use a dedicated URL reader or preserve it. | TLD list is currently incomplete; domain/path boundaries and punctuation are easy to damage. Unknown URL shape stays unchanged. | `https://reddit.com/r/all`; `example.dev`; `https://host/a?x=1`; `localhost:3000`; `www.example.co.uk`. |
| File paths, code, identifiers | Backslash becomes a space; Unix path is preserved by fixture. Inline code markers/fences are removed but code content remains. | T2S explicitly leaves GPS/X5-like codes written; N has electronic handling but not a general source-code reader; Apache-2.0 for N/T. Approach only. | Preserve the exact token for paths, code, versions, IDs, hashes, and API names unless the user explicitly wants it spoken character-by-character. | Do not delete or reinterpret `/`, `\`, `_`, `-`, `:`, dots, or digits within protected spans. A future read-aloud-code mode is separate. | `/Users/me/file`; `C:\Users\me\file`; `v1.2.3`; `snake_case`; `user_id`; `X5`; `sha256:ab12`. |
| Markdown, punctuation, symbols | Removes fences/emphasis/headings/list markers; flattens Markdown tables. Arrows become “meaning”; em/en dashes become `...`; semicolon stays. | N has punctuation pre/post processing; T promises the rest stays character-for-character and documents punctuation preservation; Apache-2.0. Neither is a long-list policy. | Remove only presentational Markdown. Keep sentence meaning; use a pause at clear clause/list boundaries without changing the visible/model text. | Do not treat punctuation inside protected URL/path/code spans as prose punctuation. Avoid inserting pauses after every comma. | Markdown bullets/table; `A — B`; semicolon with two clauses; `https://a/b`; inline code; `x -> y`. |
| Long spoken lists and pace | No list segmentation or rate control is passed to TTS. Observed 61-word capability answer took about 22.3 seconds (~164 words/minute) and included a raw slash chain. | NeMo/T2S/OVOS normalize text/typed spans, not Leko's answer structure or the voice's prosody; no direct precedent for semantic list rewriting. | Keep wording and separate speech text; split only clear list/clause structure. Prefer “apps, websites, or the whole computer. You can lock it, log out, or shut it down.” | Avoid rewriting arbitrary model prose or adding meaning. TTS rate/pause support needs separate verification; no code or acoustic test in this review. | The logged capability answer; 3- and 5-item lists; nested list; comma-only prose; semicolon-separated clauses; compare playback duration and listener comprehension. |
| Speech-only boundary and repeated passes | Model/UI reply remains original; verbalizer is called from both `query_flow.py` and `assistant.py`. | T2S documents idempotent repeat normalization; NeMo uses a separate normalization stage; Apache-2.0 for both. General pipeline idea only. | Keep normalization strictly on the TTS string, and make every rule idempotent or invoke it exactly once. | An expansion like “m” to “minutes” must not expand a second time or mutate UI text, memory, transcript, or model input. | Assert original response unchanged; assert repeated normalization equals one pass; run date/unit/slash cases through both existing call paths. |

### Prioritized support inventory

| Priority | Scope | Why it comes first |
|---|---|---|
| **P0 — prevent meaning damage** | Protect recognized date/time/fraction/unit/URL/path/code spans before any generic punctuation or slash rule. Remove the generic `word/word → and or` policy; define `A/B`, `A/B/C`, and explicit `and/or` separately. | It currently damages common items such as `km/h`, `m/s`, and `I/O`, and produces the awkward wording the user noticed. |
| **P1 — address observed speech complaints** | Clock time, month-name date ordinal/year, full weekday preservation, exact weekday abbreviation in schedule context, human-readable list boundaries. | These match the recorded “O/zero,” zero-padded date, weekday, and rushed-list observations. |
| **P2 — safe common numeric forms** | Cardinal/ordinal spans, decimals/signs, unambiguous fractions, duration units (`ms`, `s`, `min`, `h`) and correct singular/plural. | Common in spoken status, time, and focus-session replies; backed by all three project families. |
| **P3 — bounded measures** | Metric length/mass/volume/speed/temperature and common US customary length/mass/volume/area; compound units and ranges; keep original units. | NeMo provides the broadest measure taxonomy; T2S has a narrower metric subset. Maintain explicit ambiguity tests for `m`, `in`, and `oz`. |
| **P4 — specialized technical speech** | Full URLs, domain variants, time-zone names/abbreviations, identifiers, uncommon prefixes, code read-out, long or nested lists. | These have higher ambiguity or need TTS/listening evaluation beyond static normalization. |

### Candidate comparison phrase suite

Run these exact inputs through current Leko and candidate libraries only after the owner approves a comparison environment. Record each tool's output verbatim, whether it changed unsupported content, and whether its result matches the target/fallback below. This review did not install packages or execute candidate libraries; the app's talk virtual environment currently has none of these candidate packages installed, so a source-only comparison was appropriate here.

| ID | Input | Leko target or fallback | Main comparison point |
|---|---|---|---|
| N01 | `There are 1,234 rooms.` | “There are one thousand two hundred thirty-four rooms.” | Cardinal grouping and locale (`en-US`). |
| N02 | `The 21st item arrived on November 09, 2026.` | “The twenty-first item arrived on November ninth, twenty twenty-six.” | Ordinal and date day/year style. |
| N03 | `The value is 3.50, not 3.5.` | Speak trailing zero if represented precision matters; do not collapse values. | Decimal digit-by-digit policy. |
| N04 | `Temperature: -5.2 °C.` | “Temperature: minus five point two degrees Celsius.” | Signed decimal plus typed unit. |
| N05 | `Add 1/2 cup and 3/4 inch.` | “Add one-half cup and three-quarters of an inch.” | Fraction requires a recognized measure context. |
| N06 | `The ratio is 3/2.` | Preserve `3/2`. | Fraction versus ratio fallback. |
| N07 | `Born 03/04/2026.` | Locale-configured reading only; otherwise preserve `03/04/2026`. | Ambiguous numeric date. |
| N08 | `Due 2026-11-09.` | “Due November ninth, twenty twenty-six.” | ISO-like date. |
| N09 | `It is 11:32 PM.` | “It is eleven thirty-two p.m.” | Clock phrasing and zero/minute handling. |
| N10 | `Meet at 6:02 tomorrow.` | “Meet at six oh two tomorrow.” | Minute with leading zero. |
| N11 | `It starts at 12:30 a.m. EST.` | “It starts at twelve thirty a.m. E S T.” | Zone only in clock context; no guessed geography. |
| N12 | `Mon, Tue, Wed, then go to Monument Avenue.` | Expand weekday list only; preserve “Monument.” | Contextual weekday abbreviations. |
| N13 | `Focus for 1h 30min.` | “Focus for one hour thirty minutes.” | Duration compound, singular/plural. |
| N14 | `It ran for 250ms.` | “It ran for two hundred fifty milliseconds.” | Duration versus measure. |
| N15 | `The room is 5 m long.` | “The room is five meters long.” | `m` as meter, not minute. |
| N16 | `Wait for 5m before the session.` | “Wait for five minutes before the session.” | `m` as minute with duration context. |
| N17 | `The cabinet is 2 in wide; the file is in the cabinet.` | Expand only the unit; keep the preposition. | `in` collision. |
| N18 | `It weighs 8 oz; pour 8 fl oz.` | Use weight for explicit “weighs”; use fluid ounces for `fl oz`. | `oz` collision. |
| N19 | `Drive at 80 km/h or 50 mph.` | “Drive at eighty kilometers per hour or fifty miles per hour.” | Slash in recognized compound units; no unit conversion. |
| N20 | `Wind is 5 m/s.` | “Wind is five meters per second.” | Recognized `/` means per. |
| N21 | `The box is 10 × 20 cm.` | “The box is ten by twenty centimeters.” | Dimensions and plural form. |
| N22 | `The area is 5 sq ft or 2 m².` | “The area is five square feet or two square meters.” | Area recognition; no conversion. |
| N23 | `A/B; A/B/C; A and/or B.` | “A or B; A, B, or C; A, B, or both.” | Distinct slash policies. |
| N24 | `24/7 service; 50/50 split.` | Spoken idioms only when exact mappings are registered. | Slash idioms versus fraction/list rule. |
| N25 | `w/coffee; w/o sound; I/O device.` | “with coffee; without sound; input/output device” only in computing context. | Exact abbreviations; avoid generic slash rewriting. |
| N26 | `https://reddit.com/r/all?sort=top` | Preserve URL meaning; do not silently drop the path/query. | URL/path protection. |
| N27 | `/Users/me/file_name.py and C:\\Users\\me\\file.py` | Preserve paths. | POSIX/Windows path integrity. |
| N28 | `` `v1.2.3` and `snake_case` `` | Preserve identifiers. | Inline code and punctuation. |
| N29 | `- [ ] Block apps, websites, or the whole computer.` | Speak the list naturally; keep its alternatives and order. | Markdown cleanup and list cadence. |
| N30 | `You can lock, log off, or shut down the computer; you can also schedule it.` | Pause at the clause boundary; do not add pauses after every comma. | Long-list pacing independent of lexical normalization. |

The phrase set deliberately includes protected controls that a whole-text normalizer may leave alone. It should compare both output and preservation. Candidate-library output is evidence for review, not automatically the correct Leko behavior.

### Contextual `m` implementation

Do not expand a bare `m` from the letter alone. Score evidence near the value, within the same phrase or coordinated option list; do not use distant conversation history. The implementation requires a winning score of at least three and a lead of two over the competing interpretation.

- **Minutes evidence:** A directly governing label such as “duration”; phrases such as “wait for,” “lasts,” or “block for”; or supporting schedule cues nearby, such as “open hours,” clock times with `a.m.`/`p.m.`, or a time range. In `a specific duration (like 30m), open hours (for example, 9am-5pm)`, “duration” is the strong cue and the clock schedule is supporting evidence.
- **Meters evidence:** Physical-size or distance wording such as “long,” “tall,” “wide,” “deep,” “high,” or “away”; a recognized form such as `m/s` or `m²`; or nearby length units (`mm`, `cm`, `km`, `ft`, `in`) and dimension patterns such as `30 m × 20 m`.
- Treat explicit local cues as stronger than broad nearby topic cues. Expand only when one interpretation has strong evidence and clearly outweighs the other. If evidence is weak or conflicting, leave `m` unchanged.

**Covered checks:** `Wait for 30m before the session.` → minutes; `The wall is 30m tall.` → meters; `The point is 30m away.` → meters; `30m` by itself → unchanged. Nearby dimension patterns and related units provide supporting meter evidence. Weak or conflicting evidence leaves `m` unchanged.

### Prose-colon and clock-range implementation

- **Prose colon pause:** In the speech-only string, a colon after a letter and before whitespace becomes the existing ellipsis pause (`...`): `Which schedule: always, ...` → `Which schedule... always, ...`. Preserve protected clock strings, URLs, paths, code, and identifiers.
- **Clock-range connector:** A dash or hyphen between two valid clock expressions becomes “to”: `9 a.m.-5 p.m.` → “nine a.m. to five p.m.”; `6 p.m.–8 p.m.` → “six p.m. to eight p.m.” The typed range is recognized before the general dash-to-ellipsis cadence rule. Other hyphens and unclear ranges are not changed by this rule.

**Covered checks:** `For which schedule: 9 a.m.-5 p.m.?` uses the ellipsis after “schedule” and “to” between the clock times. `The meeting is at 11:32 PM.` is spoken as words without changing the time meaning. Host-and-port and hash literals retain their colons. The schedule example was listened to; broader acoustic quality remains unverified.

### Unresolved decisions and risks

1. **Locale:** Set a speech locale explicitly (`en-US` is the likely default for this user); do not infer slash-date order from the text. If the app has no locale preference, ambiguous numeric dates must stay as written.
2. **Numbers:** Decide when conversational cardinal conversion improves speech without damaging versions, IDs, room labels, phone numbers, scores, and years. Preserve fractional trailing zeros when they express precision.
3. **Ambiguous units:** Bare `m`, `in`, `oz`, `C`, and `s` require a context rule or safe preservation. A wrong unit name changes meaning.
4. **Slash:** `A/B` is not inherently exclusive or inclusive. The proposed default is “or”; only explicit `and/or` becomes “A, B, or both.” Known units and exact idioms must run before the generic list rule.
5. **URLs/code:** Speaking full paths is verbose, but dropping path/query content can alter the reference. Keep the value intact unless there is an explicit, tested URL/code reader.
6. **Prosody:** Text normalization cannot prove Zoey’s pronunciation or pace. The log establishes TTS input and timing, not audio. Test candidate output acoustically after static phrase comparisons.
7. **Double application:** The current call path verbalizes in two places. Any new rule must be idempotent, or the application should later normalize once.
8. **Library maturity/platform:** `text2spoken` is a small pre-release and its English fixture is marked unchecked; NeMo's standard pip path is a poor macOS fit because of Pynini/OpenFst; OVOS is modular but not a full-prose solution. Do not adopt any dependency based on its license alone.

### Recommendation

Use a deterministic, span-first pipeline: identify and protect typed spans, apply exact local mappings, then apply narrow prose punctuation cleanup. Start with the P0/P1 inventory, use the phrase suite to compare T2S and OVOS components, and use NeMo as a broad grammar/unit reference. Keep unknown forms unchanged, preserve value/unit systems, make the transformation speech-only and idempotent, and do not install or adopt a library until Leko-specific tests and listening review pass. No code or third-party data has been added in this review.
