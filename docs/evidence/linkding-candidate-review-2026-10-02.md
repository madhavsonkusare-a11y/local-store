# Linkding candidate review

Linkding is an existing member of the frozen 100-app planning pool. It remains
withheld in `catalog/promotion-proposals/linkding-withheld.json`; the selected
ten launch apps are unchanged.

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
No passing task or promotion is claimed from the failed attempts. The corrected task still must produce
`docs/evidence/linkding-managed-candidate-2026-10-02.json` with a passing result.

To continue this lane, reserve the same serial managed-engine qualification
slot used by the launch-app and agent fixtures. Build with the shared canonical
profile, then explicitly opt in to the isolated ignored fixture:

```powershell
cargo build --locked --release --features tauri/custom-protocol,mcp-sidecar --test managed_linkding_candidate --example candidate_review_facts
$env:LOCAL_STORE_RUN_MANAGED_QUALIFICATION = '1'
cargo test --locked --release --features tauri/custom-protocol,mcp-sidecar --test managed_linkding_candidate -- --ignored --nocapture
```

After the real fixture passes, write fresh `candidate_review_facts linkding`
output to an ignored `.cache/linkding-candidate-facts.json`. The example performs
public metadata reads only. Then prepare a new review request:

```powershell
python scripts/candidate-promotion.py --proposal catalog/promotion-proposals/linkding-withheld.json --evidence docs/evidence/linkding-managed-candidate-2026-10-02.json --probe scripts/linkding-bookmark-probe.mjs --facts .cache/linkding-candidate-facts.json --output catalog/promotion-proposals/linkding-review-request.json
```

Do not manufacture a passed receipt if a task fails. A separate reviewed
decision file must bind the exact request inputs before using `--decision` and
`--approved-output`. Registration then needs an explicit template include,
accurate unverified agent-access row, preserved 100 roster identities with
reviewed cohort reconciliation, catalog regeneration and focused boundary
checks. It must not add Linkding to the ten-app launch selection automatically.
