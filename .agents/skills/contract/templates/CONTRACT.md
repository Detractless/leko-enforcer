# Contract: <feature>

Written before any code. After the user opens it, the contract is locked: criteria cannot be edited to match the code.
Every "<...>" part must be replaced with real text. `/contract open <feature>` refuses a contract that is not complete.

## Goal
<one or two sentences: what the user gets when this feature works>

## Domains
Domains: <ui, web, cli, data - or none>

## Acceptance criteria
- AC-001: <something a user can check, in one sentence>
  - evidence: <tests/name.test.mjs "exact test name">
  - breaker: <the bug this test must catch, in one sentence>

## Checklist coverage
- <ID from the checklists of your Domains> -> criterion:AC-001
- <ID> -> out-of-scope: <why it does not apply>

## Derived checklist
<only for "Domains: none": 5-10 lines such as "D-1: <a check you derive for this feature> -> criterion:AC-001">

## Edge cases
- <input or situation that could break it> -> AC-001
- <another> -> out-of-scope: <why it is not handled now>
- <a third, at least three in all> -> AC-001

## Out of scope
- <what this feature will not do>

## Decisions
- <choice> | <why> | REVERSIBLE
