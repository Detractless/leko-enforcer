# Page region extractor (Revision 8)

`regions.mjs` is vendored verbatim from the Revision 8 source. `SHA256SUMS` pins its digest:

```
a72de98ddf1b1c6acb47e5c065bbab31f28ed9876d2a806c0203eac41ae0ecfa  regions.mjs
```

Do not edit `regions.mjs`. `extract-single-page.mjs` is the CLI wrapper used by the daemon; the daemon's default path is repo-relative and can be overridden with `WAE_EXTRACT_MJS`. The wrapper still imports the WAE collector helpers; vendoring those helpers is a follow-up step in `UNIFICATION_PLAN.md`.
