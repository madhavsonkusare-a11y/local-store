# Linkding candidate review

Linkding is an existing member of the frozen 100-app planning pool. Its
withheld proposal is retained unchanged for provenance; a separate exact-input
review approved and explicitly registered `src/templates/linkding.json`.
The selected ten launch apps are unchanged.

The checksum-pinned CapRover definition already has one service, the supported
9090 loopback port, one named data volume mounted at `/etc/linkding/data`, and
initial administrator username/password settings. Its old 1.23.0 image was
screened as stale. The reviewed replacement keeps the same official image
repository and changes only that concrete version to 1.47.0 with immutable
index digest `sha256:e35cb50e0581178f245125ffaa909c565416c94c9f22ace305a7d234a4345522`.

Primary sources inspected October 2, 2026:

- [Official v1.47.0 release](https://github.com/sissbruecker/linkding/releases/tag/v1.47.0), published September 13, 2026.
- [Pinned standard image source](https://github.com/sissbruecker/linkding/blob/v1.47.0/docker/default.Dockerfile), using Python 3.13 and Debian Trixie in the final runtime. This is source maintenance review, not a vulnerability scan or proof that every package is patched.
- [Pinned MIT license](https://github.com/sissbruecker/linkding/blob/v1.47.0/LICENSE.txt).
- [Official installation documentation](https://github.com/sissbruecker/linkding/blob/v1.47.0/docs/src/content/docs/installation.md), describing default SQLite storage and the standard image separately from its browser-snapshot variant.
- [Login form](https://github.com/sissbruecker/linkding/blob/v1.47.0/bookmarks/templates/registration/login.html) and [bookmark creation tests](https://github.com/sissbruecker/linkding/blob/v1.47.0/bookmarks/tests/test_bookmark_new_view.py), defining the exact session, URL, title, description, notes, tags and private-sharing fields used in the synthetic task probe.

The source's optional administrator settings are tightened to required answers
to avoid relying on unconfigured proxy authentication. The password is sensitive;
the username is a plain setup field. The qualification fixture uses fresh
random synthetic credentials and removes its private scratch state afterward.
It saves a private bookmark to `example.com` with unique exact content and checks
the same fields after restart and keep-data reinstall. The browser visits only
the owned loopback app. Linkding may fetch a bookmarked website for metadata,
which is documented in its install review.

The reviewed ceiling is 512 MiB, two CPUs and 512 PIDs. Qualification records
actual resource samples and owned volume size; no approximate usage result is
invented from the image's base or app category.

Promotion requires the compiled Rust plan facts, live canonical repository
review, passing managed Windows task receipt and a separate decision naming
all exact proposal/evidence/plan/probe/archive/GitHub hashes. Until that concrete
decision, no offering registration or install capability is granted.

The first real task attempt failed at the anonymous login assertion, before
bookmark creation. The preserved receipt is
`docs/evidence/linkding-managed-candidate-failed-2026-10-02.json`.
The pinned upstream [login setting](https://github.com/sissbruecker/linkding/blob/v1.47.0/bookmarks/settings/base.py)
uses `/login`, while the probe had incorrectly required `/login/`.
The corrected check requires the same loopback origin, either official login
path and the exact protected destination `/bookmarks`; it continues to reject
anonymous access and wrong credentials. All containers, networks and volumes
labelled with the failed proof's exact Compose project were independently
confirmed absent before returning the serial proof slot. A second failed
attempt is preserved as `linkding-managed-candidate-failed-2-2026-10-02.json`;
its exact owned project also left no containers, networks or volumes. It
reached a browser visibility assertion but did not record enough context to
identify which form assertion failed. The login probe now uses the official
CSRF-bound session form and the pinned [login contract tests](https://github.com/sissbruecker/linkding/blob/v1.47.0/bookmarks/tests/test_login_view.py):
wrong credentials must return 401 with the official error; correct credentials
must return 302 to the same application's protected bookmarks page. The
authenticated browser then checks the real bookmark form. This avoids relying
on asynchronous error-page rendering and does not relax either denial check.
Later failed attempts are preserved as `linkding-managed-candidate-failed-3`,
`-failed-4` and `-failed-5` receipts for the same day. They identified a stale
form-contained token selector, Linkding's app-specific `ld_csrftoken` cookie
name and a sharing checkbox deliberately hidden when the account disables
sharing. Every exact failed project's containers, networks and volumes were
independently confirmed absent before retrying. The final probe uses the
current browser session's CSRF cookie, as accepted by [Django's official CSRF
contract](https://docs.djangoproject.com/en/5.2/howto/csrf/), and checks actual
private access boundaries instead of assuming an optional control exists.

The corrected real candidate task passed on October 2 in 122.55 seconds.
`docs/evidence/linkding-managed-candidate-2026-10-02.json` records all 18 passing
steps, including exact task readback after restart and keep-data reinstall,
owned-resource cleanup and unrelated-container preservation. The private
bookmark is absent from both the authenticated shared list and a fresh
anonymous shared list; its exact edit URL denies anonymous access. If the
sharing control is present, the probe also requires it to remain unchecked.
Three resource samples recorded 233,098,444 bytes peak total memory (222.3 MiB)
within the 512 MiB ceiling, 389,370 bytes peak owned volume storage and
585,985,353 bytes of resolved image virtual size. First start was 15.346 seconds
with the image already available locally; this is not a cold download estimate.

Fresh compiled facts matched the normalized plan, immutable proposal and live
canonical nonarchived GitHub repository. The original pending request is
`catalog/promotion-proposals/linkding-review-request.json`; it binds the exact
proposal, evidence, plan, probe, archived definition and live GitHub inspection.
The passing task did not itself approve or register the app. Codex `/root`
independently inspected the exact request, proposal, runtime/source/setup/risk
fields and passing receipt, then explicitly approved the six bound hashes.
`catalog/promotion-proposals/linkding-review-decision.json` names this delegated
implementation review; it does not claim direct owner review. The approved
request and artifact are retained beside it before explicit registration.

The original 100 member IDs are preserved. Explicit reviewed cohort counts
are now 53 offered definitions and 47 expansion candidates; Linkding's
original upstream repository snapshot is retained under `promoted`. The ten
launch-task ledger remains unchanged. Linkding's exact approved manifest and
real task receipt are bound separately in `catalog/qualified-candidate-apps.json`.
Readiness can show its setup-assisted, current task and lifecycle status while
agent content access remains unverified. No vulnerability scan is claimed.

`docs/evidence/linkding-github-approved-2026-10-02.json` records the final real
lookup through the same compiled source resolver, plan and proof projection
used by the launcher. Both local and live canonical GitHub resolution return
the unique approved `linkding` offering, with the same normalized plan hash
and current task evidence. A repository URL selects its reviewed installation
definition; it never runs arbitrary code or selects an unreviewed Git commit.

Focused verification passed: seven candidate-promotion refusal checks, three
draft-generation checks, three selected-launch-ledger checks, three runtime
readiness checks, 18 reviewed-template checks, 17 GitHub source-boundary checks
and the exact 53-offering agent directory check. The separate candidate ledger
also validates through the existing exact-input evidence gate. Six GitHub
review UI checks passed in 12.7 seconds, including accessibility, supported
install-review handoff, unsupported refusal, unsafe input, unknown status and
stale-response invalidation. Those UI checks use the native-command adapter;
the live compiled lookup and real managed app task are separate actual proofs.
The source suite's opt-in live unit test remains ignored; the real final
GitHub inspection above supplies the live observation. No clean-machine
Windows WebView journey or universal arbitrary-repository installer is claimed.

The following workflow was executed before registration. Its ignored
`managed_linkding_candidate` fixture deliberately refuses an already offered
Linkding; it must not be described as a post-registration qualification run.
Future candidates reuse this separation of proposal, actual proof and exact
named decision, reserving the same serial owned-engine proof slot:

```powershell
cargo build --locked --release --features tauri/custom-protocol,mcp-sidecar --test managed_linkding_candidate --example candidate_review_facts
$env:LOCAL_STORE_RUN_MANAGED_QUALIFICATION = '1'
cargo test --locked --release --features tauri/custom-protocol,mcp-sidecar --test managed_linkding_candidate -- --ignored --nocapture
```

After a candidate's real fixture passes, write fresh `candidate_review_facts`
output to an ignored cache file. The example performs
public metadata reads only. Then prepare a new review request:

```powershell
python scripts/candidate-promotion.py --proposal catalog/promotion-proposals/linkding-withheld.json --evidence docs/evidence/linkding-managed-candidate-2026-10-02.json --probe scripts/linkding-bookmark-probe.mjs --facts .cache/linkding-candidate-facts.json --output catalog/promotion-proposals/linkding-review-request.json
```

Those historical paths now exist and must not be overwritten. Do not
manufacture a passed receipt if a task fails. A separate reviewed
decision file must bind the exact request inputs before using `--decision` and
`--approved-output`. Registration then needs an explicit template include,
accurate unverified agent-access row, preserved 100 roster identities with
reviewed cohort reconciliation, catalog regeneration and focused boundary
checks. It must not add Linkding to the ten-app launch selection automatically.
