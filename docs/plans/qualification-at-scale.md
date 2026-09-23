# Qualify apps by reusable proof paths

Decision checkpoint, September 23, 2026. V1 still requires 50 selected apps
proven on the owned Windows/WSL engine with a meaningful task; this document
changes the work method, not the release bar. Today one app (Memos) meets the
task bar. n8n has a managed-engine lifecycle pass without a task proof.

## Why the one-script-per-app loop is too slow

The current harness already installs, checks all services, measures resources,
restarts, reinstalls over kept data, and cleans up safely. Rewriting its browser
setup and persistence check for each app wastes effort and creates fragile UI
selectors. An Uptime Kuma pilot passed initial setup and then failed on a login
button selector after restart. That failure is useful feedback on the method;
it is **not** a Uptime Kuma qualification or release evidence.

## Shared qualification pipeline

1. Run `python scripts/qualification-preflight.py` over the 52 approved manifests: image/platform and
   source freshness, unsupported capabilities, required credentials, fixture
   dependencies and estimated pull size. Reject invalid candidates before
   starting WSL. Keep this deterministic. The September 23 snapshot found 49
   clean metadata preflights and three stale image reviews (Adminer, Grafana,
   Vaultwarden); this is **not** install or task proof.
2. Schedule isolated managed-engine runs through the existing Rust lifecycle
   harness. Cache pulled images, but use a fresh app root and ownership label
   per run. Bound concurrency by actual memory/disk measurements; begin with
   sequential runs. Persist resumable, content-addressed evidence so an
   unchanged pass is not repeated.
3. Replace bespoke probe programs with a small set of versioned fixture
   providers and task drivers: content roundtrip, file roundtrip, workflow
   execution, monitoring, database client, external-service fixture, and
   setup/admin. Each app supplies reviewed configuration such as its API path
   or browser selectors; a driver creates a unique synthetic item and verifies
   its exact value after restart and keep-data reinstall. Prefer maintained
   app APIs and official clients over browser clicks. Browser remains a
   fallback and a separate human UX smoke check.
4. Reuse fixture services across apps: a controlled HTTP target/feed/webhook,
   a disposable SQL database, a tiny mock model endpoint, and sample file/media
   bytes. Fixture service ownership and cleanup must be proven separately from
   the app under test. Never treat a mock provider as real-provider coverage.
5. Run a release gate on immutable manifest, engine, image and probe fingerprints.
   Failed or missing tasks cannot be promoted. Review evidence and a small
   sample of the actual UI for each app family; automate repeat runs and stale
   input detection. Keep agent access, V3 UX and signed delivery as separate
   V1 gates.

## Where Jev helps

The user-requested TypeSafe skill is installed for Codex. TypeSafe's Jev model
returns typed choices/probabilities, not browser actions or code. The
`scripts/triage-v1-tasks.py` pilot sends only public roster name, category and
acceptance task, then suggests one of eight fixture paths. Its output is
`catalog/v1-task-triage.json`; all 52 rows remain **unreviewed**, and none count
as proof. The September 23 run used seven requests and 23,563 API tokens:
17 content, 7 workflow, 5 file, 4 monitoring, 3 database, 11 external-fixture,
and 5 other suggestions. Low-confidence or evidently mismatched cases need
manual correction before building a driver. The key stays outside the repo.

Jev could later rank candidate UI controls from a sanitized accessibility tree
or flag whether a failure looks like app behavior versus a broken test. It
must not decide pass/fail, choose arbitrary actions, see credentials, or grant
an app agent access. A deterministic assertion must verify the resulting app
state. Measure its accuracy and cost on labeled pilot cases before using it in
the harness.

## Next bounded batch

1. Review the 52 triage rows, correcting the five `other` cases and obvious
   mismatches (for example, a repository push is not merely a file upload).
2. Implement one shared content roundtrip driver and one shared controlled
   HTTP fixture. Migrate Memos and add a second, different app to each where
   applicable. The migration must retain or strengthen exact post-restart and
   post-reinstall assertions.
3. Generate a matrix of preflight, engine lifecycle, task, agent access, and UX
   state per app. Batch by fixture family and image overlap, then promote only
   passing apps into `catalog/v1-qualified-apps.json`.

The TypeSafe suggestion is a planning accelerator. The likely large gains are
reusing the lifecycle harness, fixture services and task drivers, and avoiding
repeated image pulls. Installation and meaningful use remain real integration
tests that cannot be inferred from source metadata or a model score.
