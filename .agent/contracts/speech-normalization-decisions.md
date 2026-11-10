# Decision page: speech-normalization

The following baseline was reviewed and accepted in chat. These are the recommended contract choices; no new choice is blocking the draft.

- Locale and dates: use `en-US`; speak valid dates with ordinal days; preserve malformed or unsupported date strings. Recommended because the app is English-only and locale makes numeric dates deterministic.
- Numbers and fractions: normalize only clearly typed spans; retain IDs, versions, ratios, scores, and meaningful decimal precision. Recommended to improve ordinary speech without corrupting technical values.
- Slash wording: ordinary multi-letter `A/B` means “A or B”; `A/B/C` is an “A, B, or C” list; explicit `and/or` means “A, B, or both”; slash means “per” only in a recognized unit. Protect fractions, dates, URLs, paths, code, and identifiers first.
- Ambiguous units: use `m` only with duration or length context; expand `in` and `oz` only with clear unit context; otherwise preserve the written form.
- URLs and pacing: preserve complete URLs and technical literals; add speech pauses only at clear list or clause boundaries, without paraphrasing.
- Scope: deterministic local rules only; no added model, dependency, or unit conversion. Include the explicit duration, length, mass, volume, speed, temperature, and area inventory listed in AC-008.

To change one of these decisions, revise this draft before opening the contract. `/contract open speech-normalization` will lock it and authorize work under the contract.
