# Agent handoff

Updated September 23, 2026. Start with [V1_TASKS.md](V1_TASKS.md); it is the
only release ledger. [Documentation index](README.md) explains the rest.

## Current work and checkout

Phase 0 (R01–R03) is complete. Main contains the integrated code and the
consolidated V1 documentation plus the frozen 100-app planning roster.

- PRs #3–#6 are merged. Main integration commit `7d9e0d3` has exactly the
  tested tree from PR #6 head `720ce23d46a8bcfd7b401b5ef94f840296f74749`.
  Quality, minimum-Rust, Windows, macOS and Linux passed
  [run 34695407242](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/34695407242).
- Merged feature/design branches were removed after ancestry checks. The
  temporary documentation integration branch and worktree were also removed.
- Keep the stash `pre-consolidation-local-documents-2026-09-12`: its 335 V2
  files match integrated content; its three extra drafts are preserved in
  research/screening documents. Keep unrelated detached worktrees intact.
- The unmerged Claude cleanup commit remains preserved by the pushed tag
  `archive/claude-cleanup-2026-09-12`. Do not apply it merely because archived.
- [Frozen roster](v1-app-roster.md): 52 existing offerings and 48 expansion
  candidates, each with a useful task and explicit acceptance gaps. Live public
  repository checks excluded archived File Browser and Pingvin Share. Selection
  does not promote candidates, refresh old image pins or establish agent access.
- Regenerate the matrix with `python scripts/build-v1-roster.py`; use `--check`
  to detect input drift. Membership changes require an explained selection diff.

## Owner decisions to carry forward

V1 requires 50 managed-engine-verified apps selected from the 52 current
offerings, a bundled engine, owner-led V3 frontend, signed Windows delivery, an
automatic updater, and agents managing the store and accessing every offered
app. The frozen 100-app roster is a future sourcing and replacement pool, not a
V1 release gate. The new signing requirement supersedes its earlier deferral.
Windows x64 is now the only active build and certification target. The existing
macOS/Linux implementation and release-staging code are retained for later;
do not spend current V1 work or CI capacity extending or certifying them.
The first Windows-only quality run exposed Python's inherited cp1252 decoding
of Cargo's captured output. The MSRV, licence and advisory gates now parse JSON
as UTF-8 bytes and use replacement decoding only for failure diagnostics.
V2 is reference material; do not start its production integration while V3 is
being designed.

Source-available apps remain allowed with accurate notices. The eight Umbrel
icons remain under the owner's existing retain-with-NOASSERTION decision;
that does not settle broad distribution rights. Shrimply remains excluded.

## Phase 1 implementation checkpoint

- E01: [development engine payload](../engine/README.md), exact Docker component
  pins, isolated builder/exporter and offline refusal tests. Export evidence and
  128-package inventory are in `docs/evidence/engine-*-development-2026-09-13.*`.
  Local artifacts: `.cache/engine/build-4c6cb9c7590146799bcec6e000d231bf/`.
  The rootfs is unsigned and has never booted in WSL. No app uses this engine yet.
- E02: `src/runtime/engine.rs` persists local Docker endpoint bindings beside
  Compose files. New installs, lifecycle, rollback, qualification and recovery
  use the saved endpoint and clear child context overrides. Missing marked or
  corrupt bindings refuse fallback. Legacy unmarked apps are not automatically
  migrated. Explicit `bind-engine`
  adoption now verifies container ownership under the app lock before binding.
  Real Memos restart/reinstall/cleanup passed after its test process context was
  made invalid; evidence is `docs/evidence/engine-binding-memos-2026-09-13.json`.
  Default Rust suite and strict Clippy passed; final runtime tests (47), recovery
  tests and the real Memos regression also passed.
- September 14 adoption batch: full `cargo test --locked -q` and strict
  all-target/all-feature Clippy passed. New fixtures cover multi-service
  ownership, repeat adoption and refusals without changing legacy files.
  The subsequent real legacy-shaped Memos regression passed all 14 lifecycle
  steps; see `docs/evidence/engine-adoption-memos-2026-09-14.json`.
  The new test also passed strict Clippy. No existing user app was adopted.
- Q01: evidence schema 1 distinguishes the stronger harness. Historical missing
  versions deserialize as 0; batch resume reruns old/unknown versions. Full
  source/plan/engine/probe identity is still required; schema alone is not proof.
- Q02: `src/qualification/service_health.rs` checks every expected service at
  three lifecycle points. Missing/duplicate/foreign services, unhealthy or dead
  workers, missing declared health, and failed jobs refuse qualification.
  Failed/truncated cleanup inventory no longer reads as successful cleanup.
- Validation: full `cargo test --locked`, strict all-target/all-feature Clippy,
  offline payload refusal tests and an opt-in real Docker stopped-worker fixture
  passed. The full suite required normal filesystem access for its home-folder
  test; the restricted sandbox cannot read that directory. The first real run
  exposed Docker's absent `State.Health` field; the format now uses optional
  lookup. Keep that real regression when changing the inspection format.
- CI for checkpoint `c05206c` passed all jobs (run 34754367417).
- Main quality CI failure in run 34872950783 was traced to Linux fixtures using
  `/tmp` install paths while the production journal correctly requires a Windows
  path. The test helper now uses a Windows-shaped path on non-Windows CI and a
  real temporary path on Windows. Focused bootstrap tests and format check pass;
  push and confirm the full workflow before closing the incident.
- A separate local `CHANGELOG.md` edit was present and excluded from this batch.
- No historical app evidence or promotion was rewritten. Schema 1 still does
  not mean V1 qualified, resource-measured, or agent-accessible.

## Next bounded implementation batch

September 14 WSL integration checkpoint: `engine/wsl/projection.rs` shares the
plan renderer, translating binds into long syntax with missing-path creation
disabled. Seed paths have Windows/Linux identities; contents stay unchanged.
`EngineBinding::managed_wsl()` reserves schema 2 for the fixed experimental distro.
Install writes `compose.wsl.yaml`, lifecycle selects it, keep-data reinstall
retains it, failed-install rollback restores it, and recovery compares Linux
Compose labels. No launcher selection or ambient default chooses WSL yet.

Validation: 273 library tests passed (two opt-in tests skipped), plus CLI and
integration checks. The full run caught a missing mocked ownership-count reply
in the new WSL recovery fixture; after correction, all recovery tests and the
remaining registry/shared-folder/projection tests passed. Strict all-target,
all-feature Clippy passed. Real Compose parsing starts no containers. Keep the
Windows-only fake install/rollback/recovery tests in the Windows CI matrix.

Next implement a bootstrap ownership journal before enabling selection: reserve
the distro and data directory, refuse collisions, verify the rootfs digest,
record registration identity and recover interrupted imports without deleting
foreign state. Then expose explicit managed-engine qualification and run real
WSL lifecycle/conformance. The host has WSL 2.6.3 and only `docker-desktop`
registered; no Local Store distro was imported or started in this batch.
Its management CLI returned UTF-16LE output. Bootstrap inventory parsing needs
explicit decoding rather than assuming the existing UTF-8 command capture.

E03 journal checkpoint: `runtime/engine/wsl/bootstrap.rs` now records the fixed
distro, absolute data directory, exact rootfs SHA-256, caller-generated ownership
token and state. Reservation uses exclusive creation; corrupt/oversized/existing
state is retained and refused. Identity fields cannot change, and state can only
advance reserved → imported → verified. It makes no WSL calls. Next add a bounded
UTF-16LE inventory reader and prove that the fixed distro name and data directory
are both unused before reservation/import. A journal alone does not prove that a
registered distro belongs to Local Store.

E03 preflight follow-up: the same module now accepts bounded UTF-8 or the
UTF-16LE-as-captured shape observed from `wsl.exe`, and rejects replacement
characters, malformed NUL placement, controls, oversized inventories and long
names. `preflight` invokes only `wsl.exe --list --quiet`, clears `WSLENV`, refuses
the reserved name case-insensitively, and refuses any existing target directory
before executing WSL. Four focused tests and strict Clippy passed. Next stage the
payload into a fresh sibling temporary file, verify its locked length/SHA-256,
reserve the journal, then issue one bounded `--import ... --version 2` command.

E03 payload follow-up: `verify_rootfs` streams the exact locked length through
SHA-256 and refuses non-files, invalid expectations, size drift or digest
mismatch. `prepare_import` requires separate state/install paths, runs preflight,
verifies payload, reserves ownership, then returns (without executing) exactly
`wsl.exe --import <fixed-name> <data> <rootfs> --version 2` with the provision
timeout and `WSLENV` removed. Six focused tests and strict Clippy pass. `sha2` is
a direct pinned dependency; Cargo.lock reused its existing transitive version.
Next execute via a coordinator, record Imported only after success, then verify
WSL2, in-distro ownership, components and daemon before recording Verified.

E03 import follow-up: `bootstrap::import` now executes the prepared command and
advances the immutable journal to Imported only on a complete zero exit. Nonzero,
truncated and runner-error/timeout outcomes retain Reserved, report recovery is
needed, and never retry or unregister. Seven focused bootstrap tests and strict
Clippy pass. This path was tested with fake runners only; no distro was imported.
Next implement post-import identity creation/verification and the explicit
Reserved recovery decision before calling this from product UI or CLI.

E03 verification follow-up: reservation now writes a separate ownership-token
source file. `verify_imported` requires one exact distro inventory row ending in
WSL version 2, copies that token into `/usr/share/local-store` using direct
`install` arguments, verifies it with direct `cmp`, checks the exact versions of
the five locked dpkg packages, and requires Docker/Compose doctor readiness.
Only then does Imported advance to Verified. Failures retain Imported. Nine
focused tests and strict Clippy pass; all WSL execution is still fake-runner only.
Next add the explicit Reserved recovery classifier and deletion authorization;
never infer ownership from the distro name or install directory alone.

E03 recovery follow-up: `classify_recovery` reads only the journal, fixed-name
inventory and data-directory presence. A clean Reserved state permits retry;
Imported with both footprints resumes verification; Verified with both is ready.
Every partial, duplicated or uncertain combination requires manual review. It
never invokes unregister. Ten focused bootstrap tests and strict Clippy pass.
Next implement retry from the existing immutable reservation and define removal
authorization requiring matching external and in-distro tokens.

E03 retry/removal follow-up: `retry_import` accepts only the classifier's clean
Reserved case, streams the locked rootfs again, reuses the original identity and
retains Reserved on uncertain execution. `authorize_unregistration` returns the
fixed unregister command only for a complete Verified footprint after a direct
comparison of the external and in-distro ownership tokens; it never executes or
deletes app data. Twelve focused tests and strict Clippy pass. Next add Windows
prerequisite/elevation reporting and wire bootstrap recovery into product state.

E03 status follow-up: `managed_engine_status` is now a launcher-only, read-only
backend command. It reports either `not_configured` or a recovery phase and safe
disposition without exposing an ownership token, payload digest or data path;
missing state makes no WSL call. The state is kept under machine-local app data,
not roaming profile data. Thirteen focused bootstrap tests pass. Explicit setup
consent, prerequisite/elevation guidance and every destructive recovery action
remain unimplemented.

Windows prerequisite follow-up: the same launcher command now wraps bootstrap
state with a bounded `wsl.exe --status` result. It uses only availability and
exit status, never localized output, and reports whether future remediation
requires elevation and may require restart. It performs no setup mutation.
Consent, elevated execution, disk/virtualization checks and recovery actions
remain.

September 23 Windows CI follow-up: run 35458423558 passed MSRV, metadata,
advisories, format, Clippy and 296 library tests, then failed in the debug CLI
doctor integration test. The runner has Docker despite the test clearing PATH;
the test now checks that the exit status matches actual readiness, as the
packaged CLI test already does. The fake-runner unit test retains deterministic
coverage for absent Docker. Commit `a090278` passed the full Windows-only
[workflow 35818879007](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/35818879007):
minimum Rust, quality, browser UI tests, packaged CLI, release build and
artifact staging. The release publication job was correctly skipped on main.

CI fixture follow-up: Linux jobs compile the WSL module even though the product
path is Windows-only. Test journals now use Windows-shaped install paths, and
the test-only token-source adapter keeps Linux temporary paths out of the
production Windows path translator. The runtime path still translates only a
canonical local Windows path. The prior assertion also now compares the command
program as a string on every host. The minimum-Rust job passed; Linux strict
Clippy then found one needless return in its test-only branch, which is removed
in the Q02 measurement batch. Re-run the full workflow after that commit.

Q02 measurement checkpoint: qualification evidence now records measured
time-to-first-answer and project-scoped memory after install, restart and
reinstall. The first sample is the idle baseline; per-container and total peaks
are retained across phases. Missing or incomplete measurements cannot suppress
a later run. WSL routing admits the same shell-free `docker stats` command, so
the bundled engine uses one evidence path. A follow-up records bounded managed
bind-storage bytes at the same three points and virtual bytes for every unique
resolved image id in one inspection. It rejects symlinks, overflow, excessive
entry counts, malformed ids and partial image output. Named-volume measurement,
explicit limits and a real managed-engine measurement remain next.

CI follow-up: the read-only `managed_engine_status` command is now declared in
the Tauri build manifest and granted only to the launcher capability. The
capability parity test caught the missing generated permission after the
command was registered; keep that test in the full workflow.

Real managed-engine checkpoint: the cached rootfs matched its recorded
464,494,592-byte SHA-256 and was imported as the fixed WSL2 distro alongside
Docker Desktop. External/in-distro tokens matched, all five locked components
matched, Docker 29.8.0 and Compose 5.5.1 were ready. Pinned Memos then passed a
projected bind mount, health wait, Windows loopback HTTP 200, stop/start recovery
and exact-project cleanup. See `evidence/engine-wsl-coexistence-2026-09-14.json`.
The proof distro remains installed and its untracked journal/data live under
`.cache/engine/real-wsl-proof`. This is coexistence proof, not the required clean
machine run. Product bootstrap/selection and prerequisite UX remain open.

Finish E01's complete dependency lock/signed-index provenance and rootfs notice/
source review. E02's WSL code is experimental until owned bootstrap and real
proof pass. Preserve
`local-store-engine.json` and its Compose marker when keeping app data. Do not
rewrite bindings to adopt a new default; engine migration requires its own flow.
The saved endpoint is not yet Q01's full daemon/plan/probe identity.

Use the development payload for the E03 boot experiment only after defining the
owned distro/data layout and path translation. Systemd alone does not keep WSL
alive; E04 must prove background operation and coexistence. Never shut down all
WSL distros or remove a distro owned by another product.

Complete Q01 identity/freshness and Q02 RAM/disk/startup measurements before the
50-app release qualification. Do not call Docker Desktop fixtures managed-engine proof.
V3 and owner/provider signing credentials remain external inputs. No signing key,
account, spending or release publication was created by this batch.

Q01 schema follow-up: new qualification output is schema 2 and includes an
`EvidenceIdentity` covering source kind/revision, normalized Compose-plan hash,
requested and resolved images, host OS/architecture, selected engine, Compose
version, first-use probe hash and evidence level. Schema 0/1/future files remain
readable history but cannot suppress current runs. `recorded_current` requires
exact identity equality, with regression tests for changed plan, Compose,
architecture and probe. Nineteen focused tests and strict Clippy pass. Next wire
the current identity into batch scheduling and add explicit age limits for
time-sensitive upstream/image metadata.

Q01 scheduling follow-up: `Batch::remaining_current` now takes each app with the
identity the caller intends to prove. A current pass is retained, a current
failure follows the selected retry policy, and any identity mismatch reruns.
Twenty focused qualification tests and strict Clippy pass. The remaining Q01
gap is explicit expiry policy for time-sensitive upstream/image observations.

Q01 freshness follow-up: schema-2 evidence records its qualification timestamp.
Exact-identity evidence is reusable for at most 30 days; the boundary is
inclusive, while missing timestamps and timestamps more than one day in the
future refuse. Batch scheduling inherits this gate through `recorded_current`.
Twenty-one focused tests and strict Clippy pass. Next distinguish the age of
upstream/image observations from the age of the qualification execution itself.

Q01 completion: evidence identity now includes the exact source locator and
adapter as well as revision. Reviewed offering dates flow into distinct source
and image observations. Qualification execution expires after 30 days, source
review after 90 days and the oldest image audit after 30 days; invalid, absent
or future dates cannot be reused. Candidate-only runs deliberately carry no
review dates and therefore remain history rather than reusable certification.
All Q01 ledger identity and freshness fields are implemented; twenty-two
focused tests and strict Clippy pass.

September 23 E03 recovery safety follow-up: Verified bootstrap status now
compares the bounded external token with the journal and directly compares it
with the in-distro token before reporting Ready. Import verification and
unregistration authorization also reject a modified external token. Mismatch
or an unavailable comparison returns ManualReview without deleting anything.
Fifteen focused bootstrap tests pass. This read-only comparison may start the
owned WSL distro when status is requested; it does not import or unregister.
The next release-critical work is consented product setup with an authenticated
payload and clean Windows proof, followed by supervision. Do not count this
hardening as a completed engine install flow.

September 23 E01 payload-lock follow-up: `engine/packages.lock.tsv` freezes the
exact 128-package inventory from the recorded development export (SHA-256
`1f0011d3aadb90d950c3125fdaba92327f38195c19fab44acf57e5844fb430db`).
The Dockerfile compares the installed inventory against it before export, and
the Python exporter independently checks exact bytes and includes the lock in
build-input evidence. Offline pin validation and four refusal tests pass.
Docker was unavailable locally, so no post-lock build or new WSL proof was run.
E01 remains partial: transitive package bytes/index provenance, clean rebuild,
notices/source review and the Windows support matrix still block distribution.
Next obtain a repeatable clean payload build, then wire consented product setup
to an authenticated payload rather than a development tarball.

## Baseline and limitations

52 offerings (3 recipes, 49 approved templates), 1,678 discovery entries with
local icons. Generic proof establishes startup/actionable page/persistence;
it does not demonstrate a useful task in every app. No bundled engine, agent
gateway, V3 production UI or updater exists yet. See the master ledger for
remaining work rather than old app counts in historical evidence.

PR integration fixed OCI metadata CI, filled image caches, verified Tautulli's
original digest after its tag moved, filled five missing icon records, fetched
full history for catalog provenance, and corrected Compose digest validation.
The default Rust suite, strict Clippy and all remote checks passed at the head
above. Documentation changes after that head are not covered by that CI run.

## Working and verification

Reuse `src/importers`, `src/plan.rs`, `src/setup`, `src/runtime`,
`src/qualification.rs`, `src/offerings.rs` and existing frontend state modules.
Read [lessons.md](lessons.md) before Docker work. Run only one qualification
at a time, use private roots/project labels and never prune unrelated resources.
Choose the source explicitly where alternate definitions differ. Rebuild the
CLI before proving changed manifests. Pulling by digest does not restore a tag.

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
python scripts/test_catalog.py
python scripts/test_template_platforms.py
python scripts/test_validate_recipes.py
python scripts/check-template-platforms.py
python scripts/catalog_pipeline.py --check
python scripts/cache-catalog-icons.py --check
python scripts/validate-recipes.py
node scripts/generate-brand.mjs --check
npm test -- --ignore-snapshots
```

Select relevant checks for each bounded change; do not repeat Docker proofs for
text-only edits. Record task IDs, actual result and next action before stopping.
