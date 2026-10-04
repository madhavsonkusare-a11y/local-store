# Bounded production capture investigation

October 4, 2026. F03 remains PARTIAL. All experimental production icon and
capture-harness changes were restored to the committed `66fa565` bytes. The
approved freeze, baseline images and exact-byte criterion remain unchanged.

| Experiment | Result | Retained change |
| --- | --- | --- |
| Reference-style bundled image warmup before target navigation, existing software-raster flags | Three selected cases passed; full 15-case matrix passed 11 and failed 4 | None |
| Same warmup with a fresh browser process for each independent context | Three selected cases passed; full matrix passed 12 and failed 3 | None |
| Identical seven Lucide rail paths inlined at the original 17px geometry, inversion filter retained | Three selected cases failed | None |
| Inline paths with direct white stroke rather than inversion filter | Two selected cases passed; full matrix passed 13 and failed 2 | None |
| Installed Chrome, matching the approved prototype's browser channel | All three selected launches exited before page creation | None; no host/browser reconfiguration |

The final inline-vector matrix's two failures were Settings at 1440×900
(two selected-marker pixels, maximum channel delta 6) and Activity at 1280×800
(626 pixels, maximum delta 1, primarily the existing empty-state image region).
Those PNGs belong to the experimental worktree, not a new production baseline.
All full-matrix failures occurred at the strict byte equality assertion; geometry,
accessibility, assets, overflow and keyboard checks preceding it passed.

The underlying cache/render explanation remains a hypothesis: selected-case
passes did not establish repeatable complete acceptance. No implementation fix
was retained solely to improve a pass count. Do not repeat these variants without
a specific new cause or change tolerance/mask/freeze to infer approval.

Raw JSON reports remain ignored under `.cache/`: `v2-capture-warm-report-2026-10-04.json`,
`v2-capture-isolated-report-2026-10-04.json` and `v2-capture-vectors-report-2026-10-04.json`.
The October 4 capture exception remains the current committed production evidence.
The 144 prior functional/state checks remain historical passing evidence; this
investigation did not rerun or replace their results.
