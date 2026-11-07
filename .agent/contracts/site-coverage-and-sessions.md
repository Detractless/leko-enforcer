# Contract: site-coverage-and-sessions

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
The Rust core can read, judge and match two additions to format 2: websites in a block's coverage, and timed sessions (a rule that ends at an exact moment, held by a seal that ends at the same moment). Every format-2 file the phone already reads gives the same answer as before.

## Domains
Domains: data

## Acceptance criteria
- AC-001: Every format-2 file the phone already reads gets the same verdict and the same sentence as before, with the new fields switched on and with them switched off (the 172 reader cases, 27 canonical cases, 183 change cases, 28 due-time cases and 2 key vectors).
  - evidence: core/tests/extension_compat.rs "format_2_results_are_the_same_in_both_modes"
  - breaker: A new field, rule kind or lock level changes the verdict or the wording for a file that uses none of them.
- AC-002: A block can cover sites: `covers.sites` holds 1 to 1000 lowercase domain names (at least two labels, last label not all digits, each name at most 253 characters, no scheme, path, port, wildcard or space), alone or beside `apps` or `allAppsExcept`; a Mac bundle id in `apps` is read; any other site text refuses the whole zip and names the file and field.
  - evidence: core/tests/site_coverage.rs "sites_are_read_and_bad_sites_refuse_the_whole_zip"
  - breaker: A URL, an uppercase name, a port, a wildcard, an IP number, an empty list or the 1001st site is accepted, or only the bad block is skipped while the rest of the zip is read.
- AC-003: A host is covered when it equals a listed domain or ends with a dot and a listed domain, ignoring upper or lower case and one trailing dot; lookalike hosts are not covered.
  - evidence: core/tests/site_matching.rs "hosts_match_listed_domains_and_their_subdomains_only"
  - breaker: `notinstagram.com` or `instagram.com.example.org` counts as covered by `instagram.com`, or `www.instagram.com` does not.
- AC-004: Sites in canonical form and in the loosening judge: a repeated site is kept once; removing a site, or dropping the whole list, loosens; adding a site tightens; reordering is no change.
  - evidence: core/tests/extension_loosening.rs "site_cases_match_the_listed_answers"
  - breaker: Removing one site is judged no change or adding one is judged a loosening, or a reordered list counts as a change.
- AC-005: A session rule is read: `kind` is `session`, `endsAt` is a UTC moment written YYYY-MM-DDTHH:MM:SSZ that is a real date, there is no `days`, the block holds at most one session rule, it counts toward the 16 rules, and a block whose only rule is a session takes no pass.
  - evidence: core/tests/session_rule.rs "session_rules_are_read_and_bad_ones_refuse"
  - breaker: A moment with an offset, no Z, February 30, a :60 second, a `days` list, a missing `endsAt` or a second session rule is accepted.
- AC-006: A session is judged by when it ends: a later end tightens, an earlier end loosens, removing it loosens, adding it tightens, and a session that has already ended at the moment of the change counts as no rule at all (ending at that exact instant counts as ended).
  - evidence: core/tests/session_rule.rs "session_changes_are_judged_by_when_they_end"
  - breaker: Shortening a running session is allowed at once, or removing a session that already ended has to wait.
- AC-007: A lock level `sealedAt` (not before an exact moment, written `at` in the same form as `endsAt`) is read and judged: the same moment is no change, a later moment tightens, an earlier moment or a drop to open loosens, a move to or from a wait, change-hours or date seal follows the rule the date seal already follows, and a seal whose moment has passed acts as open.
  - evidence: core/tests/instant_seal.rs "moment_seal_changes_are_judged_like_date_seals"
  - breaker: Replacing a seal that ends at 11:00 with one that ends at 10:00 is accepted at once.
- AC-008: A loosening requested under a moment seal lands at that exact moment or later, not at the next midnight; with a wait or change window beside it, it lands at the latest of all of them.
  - evidence: core/tests/instant_seal.rs "due_time_under_a_moment_seal_is_the_exact_moment"
  - breaker: A seal ending 10:00 UTC is treated like a date seal, so the change lands at midnight, or the answer depends on the pinned time zone.
- AC-009: The session maker, given the current moment and a number of minutes from 1 to 10080 (7 days), returns a session rule and a moment seal that end at the same instant, exactly that many minutes later (also across a clock change), and both read back through the readers; 0, a negative number, more than 10080, or a sum that overflows is refused with a sentence.
  - evidence: core/tests/timed_session.rs "rule_and_seal_end_at_the_same_moment"
  - breaker: The rule and the seal end a minute or an hour apart, a zone or rounding moves the end, or 0 minutes makes a session that has already ended.
- AC-010: With the new fields switched off, the reader refuses each of them with the sentence the format-2 reader already gives for an unknown field, rule kind or lock level, naming the file and field; a function lists every place a settings file uses a new field, so an export can say which blocks the phone 0.3 would refuse.
  - evidence: core/tests/extension_compat.rs "phone_mode_refuses_each_new_field_by_name"
  - breaker: The switched-off reader accepts a new field, or the list misses one.
- AC-011: The extended schema file differs from the pinned format-2 schema only in the listed places (`covers.sites` and covers allowed with sites alone; rule kind `session` with `endsAt`; lock level `sealedAt` with `at`), and the pinned file is unchanged (sha256 9ce69e119fd82454dc3d3dd53890fb74485588fcb38e6c83ad5e5c3757be9af2).
  - evidence: core/tests/extension_schema.rs "extended_schema_changes_only_the_listed_places"
  - breaker: A change to an existing field's default or loosening direction slips in beside the additions.

## Checklist coverage
- DATA-1 -> out-of-scope: this feature writes no files; it reads, judges and builds values. Saving belongs to the later plan-commit-queue contract.
- DATA-2 -> criterion:AC-002
- DATA-3 -> criterion:AC-001

## Derived checklist
Not used: Domains is data, which has its own checklist above.

## Edge cases
- `covers` with `"sites": []` and no apps -> AC-002
- Site text that is really a URL, a name with a port, a wildcard, an IP number, `localhost` or uppercase -> AC-002
- A zip whose first block is good and whose second block has a bad site: the whole zip is refused and nothing is read -> AC-002
- A huge site list (100000 names), a 10 MB string or nested lists inside `sites`: refused, never a crash -> AC-002
- A host with a trailing dot, in capitals, or a lookalike such as `notinstagram.com` -> AC-003
- `endsAt` with an offset such as +02:00, February 30, a :60 second or no value -> AC-005
- A change requested at the exact instant a session ends -> AC-006
- A file whose seal moment has already passed when it is read: it reads, and acts as open -> AC-007
- Minutes of 0, -5 or 10081 (one over 7 days), and a current moment near the largest number: refused -> AC-009
- A 2-hour session across the spring clock change ends exactly 7200000 ms later -> AC-009
- A zip that uses a session, a site or a moment seal, read by the switched-off reader: the whole zip is refused -> AC-010
- A hand-written block with a session rule and no seal, or a seal that ends at another moment, reads fine and is judged by the lock it has -> out-of-scope: only the maker promises equal ends; forcing a seal would refuse files a person can reasonably write

## Out of scope
- The browser extension, Native Messaging and what happens when a site matches (Phase 5).
- Paths, keywords, "every site except", wildcards, Safari and private-mode handling.
- The shared catalog (a name like "instagram" that each device resolves) and verified lists (Phase 6).
- The Mac list of apps never to block.
- Sessions that start later or repeat; routines and outsideHours already do that.
- Enforcing a session (closing apps, refusing passes) and everything else in the decision loop (Phase 2).
- Writing the export zip and the Kotlin change on the phone; only the Rust side and the test files for the phone to copy are here.

## Decisions
- Sites are plain domain names in `covers.sites`, not catalog names | a catalog name can later resolve to domains and apps without changing this field | REVERSIBLE
- A listed domain covers itself and every subdomain | this is what Cold Turkey and CTBL++ users expect from "reddit.com" | REVERSIBLE
- Files hold lowercase ASCII names only (punycode for non-English names); the reader refuses anything else and the CLI lowercases before it writes | no IDN library, and the judge compares one spelling per site | REVERSIBLE
- The format stamp stays 2; the old reader refuses the new fields by name | the phone's own sentence already tells the person which field it cannot read, with no second version number to keep in step | IRREVERSIBLE
- Session ends are UTC moments, not local times | a 2-hour session is elapsed time, so zones and clock changes never move it | IRREVERSIBLE
- The new names are `covers.sites`, rule kind `session` with `endsAt`, lock level `sealedAt` with `at` | short, and they do not clash with `sealedUntil`'s date | IRREVERSIBLE
- Phone 0.3 refusing any zip that uses the new fields is accepted until a phone release reads them; the Mac export will name the blocks that cause it | keeps one block for both devices, and the phone's refusal is whole and plain, never a half-read | IRREVERSIBLE
- A session is at most 7 days (10080 minutes); longer lock-ins use the dated seal | the cap only guards against a mistyped number | REVERSIBLE
- While a session runs, no door and no pass opens the block; only its end does (the decision loop enforces this later; here a session rule takes no pass) | a locked session has no way in, as in Cold Turkey | REVERSIBLE
- Mac bundle ids sit in the same `apps` list as Android package names, and each device ignores names it does not have | both are package-shaped, so the phone reads the file; the phone's handling of unknown apps is a guess, not tested | REVERSIBLE
- At most one session rule per block | extending a session is then moving one date later, which the judge can read | REVERSIBLE
- A site list holds at most 1000 names, each at most 253 characters | keeps a zip bounded; long lists come from the shared catalog later | REVERSIBLE
- New logic goes in new files; existing files near the 350-line limit (strict_fields.rs at 321) are not grown past it | the standing size rule | REVERSIBLE
