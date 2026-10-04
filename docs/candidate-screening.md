> Historical screening snapshot from September 9. These are not current offering statuses.

> Historical record, indexed October 4, 2026. This preserves its original test/selection scope. Current ten-app launch acceptance belongs to [V1_TASKS.md](V1_TASKS.md); local runtime artifacts were removed during shelving.
> See [V1_TASKS.md](V1_TASKS.md) and the current manifests for release work.

# First candidate screening

September 9, 2026. This completes a bounded B1 shortlist, not app qualification.
Evidence is in `catalog/candidate-screening-evidence.json`; explicit decisions
and identity matches are in `catalog/candidate-reviews.json`. Regenerate metadata
with `python scripts/screen-candidate-images.py --refresh`; offline execution
prints the saved evidence. No layers are downloaded or containers started.

| Priority | Definition | Observed image metadata | Decision |
| --- | --- | --- | --- |
| 1 | Runtipi PrivateBin 2.0.6 | Image updated August 8, 2026; latest project release also 2.0.6; linux/amd64 and arm64 published | First B2 qualification candidate |
| 2 | Runtipi Node-RED 5.0.6 | Image updated September 1, 2026; project 5.0.7 released September 8; linux/amd64 and arm64 published | Review patch difference, then qualify |
| 3 | CapRover linkding 1.23.0 | Image updated November 24, 2023; project latest 1.46.2 | Withhold this pin; prepare maintained version and account setup |
| 4 | Runtipi Joplin server 3.7.1 + Postgres 14.2 | Server image May 18, 2026; database image May 12, 2022 | Withhold database pin; review server-specific compatibility |

These are manual screening judgments based on metadata and documented setup,
not vulnerability scans or successful installations. GitHub's latest Joplin
project release is not evidence of its latest server release. Exact registry
URLs, timestamps and platform digests are retained in the JSON evidence.

## Why this order

PrivateBin has no initial account requirement. Its documented image uses port
8080 and persists pastes in `/srv/data`. The image documentation requires
particular volume ownership and describes optional read-only/tmpfs hardening;
those details must be reviewed against our Windows-managed bind mount. The real
test must create an encrypted paste in the browser, reopen it, restart the app
and reopen it again. Loopback browser crypto must be exercised, not inferred
from an HTTP response. [Official image documentation](https://github.com/PrivateBin/docker-nginx-fpm-alpine).

Node-RED can open directly into its editor, but first-use evidence must deploy
and execute a flow, check editor websocket behavior, and preserve `/data`,
including credential encryption state. The documented container UID is 1000;
Windows storage access needs proof. This is a loopback desktop qualification,
not approval for an unauthenticated editor on a network.
[Official Docker documentation](https://nodered.org/docs/getting-started/docker).

Linkding does not ship an initial user. Its source definition's optional empty
username/password fields therefore cannot establish zero-input usability.
Review startup user creation or a typed setup requirement before qualification.
[Official installation documentation](https://linkding.link/installation/).

Joplin is a sync server, so an admin page alone does not demonstrate the user's
note-taking goal. Require account setup and a real client sync round trip;
inspect the imported base URL and review a supported database update first.
[Server source and documentation](https://github.com/laurent22/joplin/tree/dev/packages/server).

## Identity reconciliation and next handoff

Reviewed the pinned CapRover definitions against official project/image docs.
Node-RED, PrivateBin and Joplin now share their repository identity with the
corresponding Runtipi definitions; linkding gains an explicit repository match.
Alternate definitions remain separate records. Reviews bind to source revision
and normalized image list, and regeneration refuses stale matches. Image
similarity alone still never merges projects. Other unknown identities remain
unresolved; no verified unique-app total is claimed.

Next: qualify PrivateBin through the existing importer and install transaction.
The reviewed-template module currently maps only CapRover definitions; add
Runtipi support with config provenance and refusal tests before offering a
Runtipi template. Do not bypass this by adding an arbitrary-template command.
Prepare complete first-use/lifecycle evidence and then request owner promotion.
If Windows ownership or browser crypto fails, record the failure and consider
Node-RED; do not silently widen privileges or change storage semantics.
