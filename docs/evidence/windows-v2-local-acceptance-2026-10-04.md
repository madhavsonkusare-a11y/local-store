# Local production V2 acceptance review

October 4, 2026. This review covers production controllers with explicit test IPC,
not actual Windows WebView flows or a clean-machine release candidate. F03 remains
PARTIAL: the approved handoff's exact-byte screenshot requirement is not met.

The approved inventory contains 88 states: 86 production states and two excluded
prototype states (`overview.concepts`, `panel.open`). The existing
[individual state map](windows-v2-production-state-map-2026-10-03.md) maps each
state to its real controller. The family review below explains tested behavior;
neither inventory coverage nor a shared renderer establishes an individual
pixel/accessibility acceptance result for every state at every viewport.

| Family | States | Production contract and concrete checks |
| --- | ---: | --- |
| Overview | 6 | Saved-app counts, engine status, pending refresh, true empty registry, read failure/retry, real session activity. `overview`, `v2-engine-loading`, `v2-empty-preflight-acceptance`, `v2-production-gaps` tests. |
| Discover | 10 | Current catalog, initial loading/failure, retained pagination/search, combined filters, selected identity, detail/connect drawers and empty results. `catalog`, `app`, `v2-recovery-discovery-acceptance`, `v2-production-gaps` tests. |
| My Apps | 17 | Compact selected rows, detail-header/Manage actions, actual status/readiness, operation focus and errors, linked-app limits, on-demand logs including loading/empty/failure, keep-data default and exact-name deletion. `my-apps`, `operations`, `readiness`, `v2-log-state-acceptance`, `v2-production-gaps` tests. |
| Activity | 8 | Session-local data, pending/finishing/success/failure, safe bounded metadata, filter/expansion/focus and clearing on reload. `activity` tests. The prototype loading state maps to immediate in-memory history; no fabricated asynchronous fetch ships. |
| Settings | 6 | Routed workspace, heading/back focus, real pending/status/failure, explicit setup consent/busy navigation, separate agent controls. `v2-settings-route`, `v2-engine-loading`, `catalog`, `launch-collection`, `agent*` tests. |
| Install | 21 | Current recipe/proof/typed answers, unavailable review refusal, busy/cancel/success, all seven actual backend stages and five error codes. `operations`, `setup`, `first-run-result`, `v2-install-state-acceptance`, `v2-empty-preflight-acceptance` tests. No fabricated percentages or automatic launch/grant. |
| Recovery | 11 | Verified/no-container candidates, loading/empty/mismatch/failure, explicit keep/delete review, pending focus/Escape guard and confirmed results. `recovery`, `v2-recovery-discovery-acceptance` tests. |
| First run | 7 | Actual checking/ready/missing preflight, starter choice/port, reviewed install and saved installed/connected results. `first-run-result`, `v2-first-connection`, `v2-empty-preflight-acceptance`, `v2-production-gaps` tests. |

The broad current-source run exercised 159 tests: 144 behavior/state/reading checks
and nine exact viewport captures passed; six strict capture comparisons failed.
The temporary diagnostic criterion then passed 15 viewport cases with exact
geometry/styling and bounded pixel variance. That diagnostic is not a replacement
for approved handoff gate F. The final test again requires literal PNG equality.

The final bounded investigation waited for fonts, visible image decoding, finite
animations (up to one second), and two frames, and compared inner status-dot
geometry/colors/shadows. Three serial route checks still failed only on sidebar
anti-alias pixels: 102–114 changed pixels, maximum channel difference 6/255, with
matching geometry, accessible controls, no external requests and no asset errors.
[Exact final result](windows-v2-capture-exception-2026-10-04.json) preserves this
limitation. No screenshot masking/cropping, frozen-design edit or acceptance
waiver was made; do not report all 159 tests as passing.

Reproduce the still-required strict gate with `npm run test:ui:captures`.
It has a separate Playwright configuration and retains literal equality; the
source milestone's ordinary functional suite does not execute this unreleased
Windows capture gate. This separation prevents a known rendering exception
from breaking unrelated functional CI; it neither passes nor waives gate F.

The tested sizes are 1440×900, 1280×800, and 1280×640 at device scale 1.5.
Frozen assets remain unchanged: 135 tokens, three contexts, five fonts, nine
screens and 88 states. Five changed shell baselines were visually reviewed.
Production has no lab panel, forced-state query parameter, fake history or
prototype telemetry. Native scaling, full installed-app WebView flows and a
supported clean Windows machine remain separate F04/R04 acceptance.

Next visual action: resolve the reproducible strict capture environment or obtain
an explicit revised owner-approved capture criterion. Do not repeat whole builds
or change product styling solely to disguise renderer variance.
