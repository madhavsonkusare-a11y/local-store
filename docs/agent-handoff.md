# Agent handoff

Updated October 2, 2026. Start with [V1_TASKS.md](V1_TASKS.md); it is the
only release ledger. [Documentation index](README.md) explains the rest.

## October 2 implementation checkpoint — 71.0% Windows progress

Windows roadmap: **22/31 DONE (71.0%)**, source milestone **4/4 (100%)**.
The requested 70% target is met without partial credit or changing the denominator.
The [checkpoint receipt](evidence/windows-development-checkpoint-2026-10-02.json)
records the exact completed/remaining IDs and verification counts.
A06 and A08 are the final two accepted rows. All ten launch apps have useful
actual content paths: three support exact approved writes and seven read-only
summaries/files. See [coverage](evidence/launch-agent-coverage-2026-10-02.md) and
[untrusted content/recovery boundaries](evidence/agent-content-boundaries-2026-10-02.md).

`python scripts/check-agent-provider-evidence.py` passes 10/10. Runtime directory
and capability displays check exact provider/test/fixture bytes and 30-day expiry;
failed, stale or changed proof withholds verification. Credentials/client grants
remain separate. Kanboard's pinned API now returns separate project/task lists;
the corrected adapter passed, and all five shared-source API proofs were refreshed.
The first failure is retained. Memos/n8n/PrivateBin/File proofs are current.

Focused verification: eleven Rust directory/provider/readiness/scope checks; targeted
library Clippy and formatting; 23 browser checks for first-run/result, My Apps,
provider labels and content controls; three source-proof refusal checks; 10/10
managed task/resource ledger; frozen 100-app roster and approved V2 token checks.
No optional whole-suite/platform build was used to reach this checkpoint.

UI refinements include explicit first-run heading focus, real selected-app Manage
controls using existing handlers/busy guards, requested-only Copy logs with honest
clipboard failure, and visible local-recovery/same-user limitations. Complete
native WebView acceptance remains F04; remaining full V2 component/state review is F03.

**Next bounded task:** C04. Linkding is still withheld after two retained failed
actual proofs. Its corrected CSRF login probe is prepared; run only
`managed_linkding_candidate` on the owned engine, inspect the concrete passed
receipt and hash-bound review request, then provide a separate exact reviewer
record before registration. Keep the ten launch apps unchanged and preserve the
100-ID roster. PicoShare remains withheld for its unsupported Alpine base.
Do not infer candidate approval from a parser test or image pull.

Nine active rows remain: E01–E04 (engine distribution/bootstrap/resilience),
C04 (candidate promotion), F03–F04 (complete V2/native flows), R04–R05
(clean-Windows release candidate and Windows artifact/notices review). Docker Desktop is not
required: the selected owned WSL engine serves new installs. Its 138-package
development rootfs includes the repaired systemd-session dependencies, but fresh
bootstrap, independent provenance/notices review and clean-host proof remain.
The rootfs/browser payloads stay ignored; the source tag stays immutable.

Run engine proofs serially. The prepared long-idle supervisor check can follow
candidate acceptance; it does not replace native close/sleep/wake/removal gates.
Stop only the exact owned worker through `engine stop-supervisor` before relinking
its running executable. Never stop global WSL or kill processes by broad name.

Implementation commit: `ca5746c` (owned runtime, ten-app access and V2 updates).
Post-commit catalog generation/check passes with no changed generated files.
Only the owner's two files remain outside the implementation commit.

Preserve owner's `CHANGELOG.md` and untracked `pnpm-lock.yaml`; they are excluded
from this implementation batch. Actual checkout: `D:\06 Projects\dockwrap`.
The sandbox names the old vault path; use an explicit workdir/escalated approval.
Canonical incremental Cargo flags: `--locked --release --features tauri/custom-protocol,mcp-sidecar`.

## October 1 source release complete — 100%

G01–G04 are DONE. The owner authorized publication, and
[source-v1.0.0](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0)
is public/latest at audited commit `a53e370`; the annotated tag remains immutable.
Both downloaded ZIP/tar.gz have 2,700 files matching committed Git blob hashes,
with identical SHA-256 content inventories. There are zero uploaded binary
assets, installer files or private-key files. The published ZIP passes secret
and icon validation; catalog generation passed in the full checkout and needs
Git history. Archives contain the committed generated data, so building does
not require regeneration. [Verification JSON](evidence/source-release-verification-2026-10-01.json)
records the actual release, tag, commit and observed archive hashes.
[Audit evidence](evidence/source-release-audit-2026-10-01.md) records the
corrected embedded-source build, narrow scanner exceptions, CapRover license
preservation and eight unlicensed artwork replacements using existing monograms.

Windows product progress remains **9/31 (29.0%)**. Next prioritize E03's owned
engine setup/consent and recovery in the approved V2 flow; E01's independent
engine provenance review and Q02's remaining nine resource ceilings are also
open. Agent content access and complete V2 workflows remain unfinished. Keep
Docker proof runs serial on this host. Preserve the owner's unstaged
`CHANGELOG.md`; do not recreate this source release or move its tag.

## September 30 release-scope correction

The owner chose a **source-only V1 GitHub Release**. The repository is already
public; no Windows installer, signing certificate, updater key or HTTPS update
channel is required for the next tag. The existing `v*` tag workflow publishes
an installer, so use the [source-only gate](V1_TASKS.md#source-only-v1-github-release-gate)
and [publishing procedure](../PUBLISH.md) before tagging; no release tag has
been created in this scope update. G02's source audit, G03 release notes/owner
publication authorization and G04 source-archive verification remain.

The Windows product roadmap continues separately: 9/31 active rows are DONE
(29.0% by strict task count), Q03/Q04 now close on the current 10/10 managed
task ledger, and S01–S05 are DEFERRED rather than blocked. E01–E04, Q02,
agent access and V2 implementation are still incomplete; none blocks source
publication. Keep the owner's uncommitted `CHANGELOG.md` change untouched.

## September 30 parallel implementation and launch scope

The owner reduced the V1 launch gate from 50 to **10 managed-engine-verified
apps**. PrivateBin passed an [encrypted paste task](evidence/privatebin-managed-paste-2026-09-30.json),
and Uptime Kuma passed an [HTTP monitor task](evidence/uptime-kuma-managed-monitor-2026-09-30.json)
through restart and keep-data reinstall. n8n then passed a [saved workflow and
exact execution-result task](evidence/n8n-managed-workflow-2026-09-30.json)
through both transitions. Gitea then passed a [private-repository, exact-file
and commit task](evidence/gitea-managed-repository-2026-09-30.json) with 19/19
steps through restart and keep-data reinstall. WordPress now also passed an
[exact published-post task](evidence/wordpress-managed-post-2026-09-30.json)
with 19/19 steps through both transitions. Jellyfin passed an
[exact sample-media import and playback task](evidence/jellyfin-managed-media-2026-09-30.json)
through both transitions. Immich passed an [exact photo upload, filename search
and download task](evidence/immich-managed-photo-2026-09-30.json) with 21/21
steps through both transitions. The curated validator is **10/10**. This clears
the selected-app proof count, not the full V1 release gate. Keep 52 current offerings and the 100-app planning
roster as discovery/replacement pools rather than treating them as verified.
Q02 now has its first enforced resource ceiling: Flatnotes' reviewed plan caps
one service at 512 MiB, 2 CPUs and 512 PIDs. Its [new managed task proof](evidence/flatnotes-managed-content-2026-09-30.json)
passed 18/18 steps; all-service checks read Docker's actual memory, CPU and PID
configuration after install, restart and keep-data reinstall. The previous
Flatnotes evidence is historical. The other nine release apps still need
app-specific limits and fresh proofs before Q02 can close.
Work is split into non-overlapping lanes: E01 reproducible bundled-engine
payload, A03 bounded agent management tools, F03 approved V2 screen integration,
and one managed-engine qualification worker. E01's dated Ubuntu snapshot produced
a [clean 128-package match](evidence/engine-snapshot-development-2026-09-30.json)
and [34 archive / 39 index hashes](evidence/engine-ubuntu-provenance-2026-09-30.json).
Independent signature and transitive-lock verification, plus release review,
remain. A03 added
owner-granted start/stop MCP tools with bounded scope and audit;
[Memos real-engine proof](evidence/memos-agent-lifecycle-2026-09-30.json)
passed stop/start, revocation denial and secret-free audit. Cancellation and
install/uninstall approval remain. The V2 lane added live-data Overview and My Apps
master/detail routes; Install/Recovery and first run still need integration.
Do not run simultaneous Docker qualifications on this shared host. The owner
excluded changedetection.io from the 10-app launch shortlist and asked for
more widely used offerings. The next managed-engine worker should prioritize
follow-up release checks. Immich uses its pinned Runtipi definition and two
reviewed named-volume pins for PostgreSQL and the model cache; its media remains
in the selected host folder. The first administrator uploaded a tiny owned PNG,
found it by filename and downloaded identical bytes after both lifecycle
transitions. Jellyfin uses the pinned Runtipi definition to expose
a selected host media folder; a reviewed named-volume pin keeps `/config` on the
engine Linux filesystem. Its proof completed the first-run wizard through the
API, imported an owned WAV and streamed identical bytes after both lifecycle
transitions. The UI opened to a server-selection screen on this host; check
launcher connection friction before release. WordPress required two reviewed named-volume
pins for its PHP files and MariaDB; generated credentials remain in the managed
app folder. Its first-use proof uses the installer, XML-RPC publication and the
query-form REST route, which works before permalink rewrite setup. Gitea required two narrow
reviewed storage pins: its `/data` Git/SSH files and PostgreSQL data need
Docker named volumes because Windows-backed bind mounts reject POSIX chmod.
The upstream Runtipi definition remains verbatim; a focused stale/duplicate-pin
test and the real managed proof passed. n8n's pinned browser
probe signs up an owner, saves a connected two-node workflow, runs it, and
checks the exact persisted output after restart and keep-data reinstall. The
recipe now sets `N8N_SECURE_COOKIE=false` because its editor is served through
loopback HTTP; a separate diagnostic exposed a transient healthy-before-editor
startup window, so the probe reloads until the editor is ready. Uptime Kuma's pinned browser
probe passed all 18 lifecycle/task steps and three resource samples. The focused
validator, Rust formatting and probe syntax checks pass; the V2 Overview/My
Apps browser checks passed 4/4. `CHANGELOG.md` is the owner's edit.

## September 29 unsigned personal build

The owner confirmed code-signing and updater private keys are needed before
shipping, not for a local personal build. The approved V2 Settings dialog now
shows typed managed-engine status and offers repair only after verified
ownership with an unresponsive daemon. Focused browser checks passed.
The first Tauri NSIS bundle incorrectly selected the MCP Cargo binary as the
main app (677 KB installer). The sidecar entry point is now an explicit
`mcp-sidecar` Cargo feature outside `src/bin`, the launcher is pinned by
`mainBinaryName`, and `scripts/check-local-windows-build.py` gates the
actual binary identity in CI. A corrected unsigned NSIS installer built at
`target/x86_64-pc-windows-msvc/release/bundle/nsis/Local Store_0.5.0-1_x64-setup.exe`
(16,017,533 bytes), copied with matching SHA-256 to `dist/local-preview/`; its launcher reports version 0.5.0-1 and differs from the
bundled MCP sidecar. This is a local preview dependent on Docker Desktop or
an already configured managed WSL engine, not a clean-machine bundled-engine
V1 release. S01–S04 remain release gates. The user's CHANGELOG edit remains
untouched.

## September 28 scope and implementation checkpoint

The owner selected the previously approved V2 design as the V1 interface; there
is no V3. F01 is DONE under the revised scope; **7/36 ledger rows are DONE
(19.4%)**. The V2 handoff and freeze are the active visual contract, with
engine/agent consent extensions still to implement. See [PUBLISH.md](../PUBLISH.md)
for the owner's plain-language signing and GitHub Releases updater choices.
Provider identity and updater private keys remain owner-controlled.

Two managed Node-RED flow qualifications failed during container startup.
Node.js `copyFileSync` returns EPERM copying `settings.js` into the
Windows-backed managed mount; an isolated UID1000 shell copy at the same path
depth succeeds. The timeout diagnostic now retains the actual error code. The failed evidence is
[evidence/nodered-managed-flow-2026-09-28.json](evidence/nodered-managed-flow-2026-09-28.json);
it is **not** a qualified app. The accepted task gate remains 3/50. Inspect
the exact mount and container error before retrying or changing permissions.

E01 clean rebuild correctly refused one upstream Ubuntu package drift
(`libapparmor1` .7 to .8). The bounded inventory diagnostic writes
[evidence](evidence/engine-inventory-drift-2026-09-28.json) without exporting
unlocked bytes. E01 remains PARTIAL. Agent MCP discovery now lists only
installed apps with live grants, and GitHub candidate previews check pinned
source archive provenance; A03/C04 remain PARTIAL. Focused tests passed.
The approved V2 fonts/brand/tokens and shell foundation are in production
`src/`; the remaining screens and engine/agent consent states keep F03 PARTIAL.
The V2 freeze, focused Discover/keyboard and dark-only tests pass; the focused
connect-dialog screenshot was refreshed and visually checked. Shipped font
licences now appear in `THIRD_PARTY_NOTICES.md`. The user's
`CHANGELOG.md` edit remains untouched.

## September 28 Kanboard follow-up

A pinned Kanboard JSON-RPC probe creates a unique project and exact task,
moves the task to a second column, and independently reads the state after
restart and keep-data reinstall. The isolated managed-engine run passed in
76.8 seconds and wrote [evidence](evidence/kanboard-managed-task-2026-09-28.json).
The curated gate now accepts **3/50** app tasks; n8n remains lifecycle-only.
The pilot used Kanboard's documented default admin credentials in its isolated
loopback install, so credential rotation and agent access remain unproven.
The overall V1 ledger stays **6/36 DONE (16.7%)**. Next build a controlled
HTTP fixture family and review a candidate that can reuse it.

## September 28 checkpoint

Docker Desktop is running. The Local Store WSL registration still pointed at
the old vault path after the repo move, so the first Flatnotes attempt stopped
at ownership preflight. The stopped 6,032,457,728-byte VHD was copied and
SHA-256 verified under `.cache/engine/repair-backup-2026-09-27/`; its bootstrap
journal and WSL registration were also backed up there. The stale distro
registration was repaired with WSL unregister/import-in-place using the moved
VHD, then the in-distro ownership token was compared directly with the
external token before the journal install path was updated. Keep this backup
until the recovered engine has been used for more than this pilot.

Flatnotes then passed two isolated managed-engine runs; the second persisted
[evidence](evidence/flatnotes-managed-content-2026-09-28.json) and entered
[catalog/v1-qualified-apps.json](../catalog/v1-qualified-apps.json). The
curated gate accepts **2/50** meaningful managed-engine app tasks (Memos and
Flatnotes); n8n remains lifecycle-only. The V1 task ledger is still **6/36
DONE (16.7%)**. The next app family needs a reviewed exact-state probe and a
single managed-engine run; do not treat Docker Desktop proof as equivalent.

This batch wires the Windows MCP sidecar into release staging. Seven focused
release-staging tests and sidecar JSON parsing pass; a real Tauri installer
build has not yet confirmed bundling. The user's uncommitted `CHANGELOG.md`
remains untouched. No full CI build was started for this batch, per the
owner's fast-check cadence.

## September 27 integration checkpoint

The checkout now lives under `D:/06 Projects`; older path references in this
handoff are historical. The user's uncommitted `CHANGELOG.md` is untouched.
The literal V1 task count is 6 DONE of 36 (16.7%); 50% requires 18 DONE.
Implementation through `ef3e178` passed
[Windows CI run 36328665761](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/36328665761):
minimum Rust, quality/UI tests, packaged CLI and release artifact staging.
This is build evidence, not signed V1 release approval.
See [parallel execution](plans/parallel-v1-execution.md) for bounded agent
lanes and the one-worker managed-engine constraint.

September 27 follow-up: A02 now has local agent-client list|enroll|revoke owner
commands. Credentials are returned once, stored only as hashes, and revocation
clears client grants so re-enrollment cannot revive access. A03 still has no MCP
transport. C04 candidates now carry a pinned definition URL, review stage,
manifest shape and remaining checks; this is a review preview, not install
approval. Focused gateway tests (8), GitHub-source tests (15), and a CLI
compile check passed. The workflow now runs its full Windows checks for PRs,
version tags and manual dispatch; normal main pushes use focused local checks
until the next milestone. The latest full green checkpoint is run 36330091524 (pre-MCP).

Next status-access slice: `agent-grant status <client-id> <installed-app-id>
--hours <1..720>` and `agent-grant revoke <client-id> <app-id>` now persist
bounded owner grants. A separate `local-store-mcp` development binary exposes
only `local_store_get_status` over stdio, taking the enrolled bearer from its
owner-configured environment. Credential, grant and audit checks run on every
call; client metadata is ignored. The binary is not packaged for V1 yet and
no install/content/write tools are exposed. Focused gateway tests (9), MCP
handler tests (2), both-bin compile and formatting checks passed. A grant
race with concurrent client revocation was closed by holding the credential
lock through the policy write. Keep A02/A03 PARTIAL.

A02 has a durable, locked internal policy store with versioned grants, redacted
bounded audit and fail-closed corruption handling. Eight focused tests pass;
external transport and V3 onboarding remain. C04 now offers opt-in bounded live
GitHub metadata and commit pinning behind the launcher command, with mocked
HTTP refusal tests; no install approval or V3 preview is implied. Q04 has a
shared Memos/Flatnotes exact-content probe with three passing fake-API tests;
Flatnotes still needs a real owned-WSL run. None of these PARTIAL tasks moved
to DONE. A03 now also has a transport-neutral status gateway that re-verifies
an owner-enrolled random bearer token on every request, checks a durable grant
and saves audit before dispatch; no MCP endpoint or V3 credential onboarding is
exposed. F02 now has typed launcher-state IPC, persisted onboarding viewed
steps and launcher recovery commands delegated to existing locked CLI paths.
Viewed steps are not consent. Real launcher action parity and V3-approved flow
remain. The integrated library suite passed 339 tests (two ignored), all-target
strict Clippy and Rust formatting passed, and the shared probe's three local
API tests passed. See the corresponding master ledger rows before claiming
progress.

Docker daemon was unavailable after the owner's disk migration, so no new real
app qualification ran. Start Docker Desktop from the Start menu before the
Flatnotes pilot; do not launch it inside a Claude/Codex session. The pnpm store
was moved to `D:/06 Projects/_build/pnpm-store`, Axonix UI and extension were
reinstalled/relinked, the extension build and UI TypeScript check passed, and
the old `D:/.pnpm-store` was removed after package/index verification. No
Compose or script bind mounts to the old `D:/Data/Madhav/...` layout were found
in Local Store or Axonix.

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

V1 requires 10 managed-engine-verified apps selected from the 52 current
offerings, a bundled engine, approved V2 frontend, signed Windows delivery, an
automatic updater, and agents managing the store and accessing every offered
app. The frozen 100-app roster is a future sourcing and replacement pool, not a
V1 release gate. The new signing requirement supersedes its earlier deferral.
Windows x64 is now the only active build and certification target. The existing
macOS/Linux implementation and release-staging code are retained for later;
do not spend current V1 work or CI capacity extending or certifying them.
The first Windows-only quality run exposed Python's inherited cp1252 decoding
of Cargo's captured output. The MSRV, licence and advisory gates now parse JSON
as UTF-8 bytes and use replacement decoding only for failure diagnostics.
The approved V2 handoff is now the release UI target; extend it for engine
and agent consent states without changing the frozen visual system casually.

Source-available apps remain allowed with accurate notices. The eight Umbrel
icons remain under the owner's existing retain-with-NOASSERTION decision;
that does not settle broad distribution rights. Shrimply remains excluded.

## Phase 1 implementation checkpoint

- E01: [development engine payload](../engine/README.md), exact Docker component
  pins, isolated builder/exporter and offline refusal tests. Export evidence and
  128-package inventory are in `docs/evidence/engine-*-development-2026-09-13.*`.
  Local artifacts: `.cache/engine/build-4c6cb9c7590146799bcec6e000d231bf/`.
  The rootfs remains a development payload without release provenance. Later owned-WSL Memos and n8n runs below prove it can boot and host apps; they do not approve E01 for release.
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

September 23 E03 setup preflight follow-up: the product now reserves a local
`wsl-data` path beside its per-machine `wsl-state` journal. Managed-engine status
reads free bytes available to the caller on that planned volume through the
Windows filesystem API; an existing journal's recorded install path takes
precedence. Unknown measurement remains unknown, never zero or sufficient.
No arbitrary minimum is used until a release payload and first-app disk budget
are measured. Fifteen focused bootstrap tests pass; parallel Windows tests now
use a monotonic suffix to avoid equal-clock-tick scratch-directory collisions.
Strict all-target/all-feature Clippy passes. The previous two Windows workflows
passed. Docker Desktop still had no responding daemon, so no payload rebuild ran.

September 23 Q02 volume follow-up: qualification now measures every declared
Compose named volume on the selected managed WSL engine, using Docker's exact
volume inspection and Compose ownership labels before a bounded in-distro `du`.
The three lifecycle samples retain idle/peak per-volume bytes. Missing, foreign,
unsupported-driver, truncated or unmeasurable volumes fail the resource step;
older evidence without the fields cannot be reused. This has fixture proof only:
31 qualification tests and strict all-target/all-feature Clippy passed; one
opt-in real-Docker test was skipped while the daemon was unavailable. The latest
Windows workflow before this change passed.
Next prove the sampler on the real owned distro, then add explicit resource
limits and repeat Q02 across release apps. Do not claim 50 verified apps yet.

September 23 follow-up: the Windows quality run for the volume sampler exposed
a clock-boundary flaky test in qualification's atomic evidence round trip. The
test now compares the same recorded evidence value instead of creating a new
timestamp in a later second. The normalized plan now accepts optional per-service
memory bytes, CPU millicores and PID ceilings and renders Compose `mem_limit`,
`cpus` and `pids_limit`; unset fields leave existing recipes unchanged. These
are mechanism only: no release app has a measured ceiling assigned yet. Next
select ceilings from real managed-engine resource samples, apply them to release
apps and confirm Docker enforces them. The earlier volume sampler also still
needs real managed-engine proof before Q02 can close.
The full `cargo test --locked` suite, formatting check and strict all-target,
all-feature Clippy pass locally. A home-folder test now skips when this
sandbox denies canonicalization of the profile path, which the validator also
requires; that removed an environment-only test failure.

September 23 A01/C04/E02/Q02 batch: `catalog/agent-access.json` now covers all
52 approved offerings with explicit unverified content access. `src/agent_access.rs`
consumes it, ties discovery to registry installs, reports unavailable grants
and unknown login state, and dispatches one typed in-process status tool for an
installed ID. It is not an externally callable agent gateway; A02 identity and
grants must precede MCP or IPC exposure. `src/github_source.rs` plus the
launcher-only `resolve_github_source` command maps a strict HTTPS GitHub repo URL
to an exact approved offering or a review-needed/ambiguous result. Unknown
repos are not inspected or installed yet, so C04 remains partial.

Qualification now accepts an explicit engine binding and verifies a managed
WSL journal, distro and in-distro ownership token before use. The opt-in
`tests/managed_qualification.rs` used the development proof journal under
`.cache/engine/real-wsl-proof/state`, not a product-installed journal. Memos
passed 18 lifecycle/resource steps, three samples, restart, keep-data reinstall
and cleanup; evidence is `docs/evidence/memos-managed-resource-2026-09-23.json`.
One failed attempt exposed WSL localhost forwarding holding the just-released
port briefly; the qualification loop now waits up to 30 seconds for that exact
port to become bindable. The passing run left no containers. This proves the
Memos runtime path and resource sampler, not a named volume (Memos has none).
The first n8n run failed at restart after measuring its named volume; the later
isolated rerun passed. The later Memos first-use run below supersedes the
answers-only Memos claim. Next apply measured limits, then wire product engine
selection and consented setup.

September 23 C01 refresh: `python scripts/build-candidate-queue.py` replayed
606 source definitions from the two checksum-pinned archives through the actual
Rust importers; `python scripts/rank-candidates.py` reused cached reach signals;
`python scripts/build-v1-roster.py` regenerated the acceptance matrix. Membership
remains 100 (52 offerings plus 48 planning candidates), with zero V1-qualified
or agent-content-access apps claimed. Five definitions formerly counted as
importable are now refused because their command values depend on environment
substitution the normalized plan does not perform: moneroblock, siyuan,
ghostfolio, Runtipi nocodb and CapRover outline. This is a real demotion, not
a reason to silently select replacements. Current top-150 ranking: 89 structurally
importable, 61 blocked. Queue tests and `build-v1-roster.py --check` pass.

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
September 23 Q02/Q03 follow-up: the isolated n8n managed-WSL qualification
passed on rerun: 18 lifecycle/resource steps, three samples, named-volume
measurement, restart, keep-data reinstall and exact cleanup. The earlier
120-second restart failure did not recur. The updated evidence file is
`docs/evidence/n8n-managed-resource-2026-09-23.json`. Along with Memos, this
makes two managed-engine lifecycle passes; n8n remains answers-only. WSL calls require
outside-sandbox execution on this host (`wsl.exe --status` otherwise reports
Access denied), so a sandbox-only readiness result is not host evidence.

September 23 Q01/Q04 follow-up: `scripts/memos-content-probe.mjs` uses the
pinned Memos 0.30 API to create the first admin and a private memo, then sign in
and read the exact memo after restart and keep-data reinstall. The real owned
WSL rerun passed all 18 steps; evidence is updated in
`docs/evidence/memos-managed-resource-2026-09-23.json`. The disposable proof
state is removed after qualification. `ScriptProbe` evidence now hashes script
source as well as its description, so changed probe code changes proof identity.
One of 50 release apps now has task-level proof; n8n has lifecycle proof only.
`catalog/v1-qualified-apps.json` curates only Memos today. Run
`python scripts/check-v1-qualified.py` to validate manifest, evidence and
probe hashes plus engine/freshness/task steps; CI runs it. `--release-gate`
intentionally exits nonzero at 1/50. Add an app only after a real managed-engine
task proof and review its exact evidence. The gate validates recorded evidence,
not an app's entire behavior or the signed release artifact.

September 23 E04 first slice: managed-engine status now exposes a daemon state
only after the journal and in-distro token agree. Explicit `engine repair` CLI
holds a cross-process operation lock, starts `docker.service` in the fixed
Local Store distro only if unresponsive, then rechecks Docker and Compose.
`engine status` returns the same typed status as the launcher. V3 repair UI
is still absent. Fixture tests cover healthy, unresponsive and ownership-refusal cases. Real
sleep/wake, crash/restart, coexistence and removal proof remain.

September 23 A02 first slice: `src/agent_policy.rs` provides in-process scoped
grants, expiry/revocation, OS-random one-use approvals for write/destructive
actions, and metadata-only audit records. `agent_access::call_app_tool` now
requires policy authorization before status dispatch. Focused policy/access
tests and strict Clippy pass. This is not an external agent gateway: identity
authentication, durable grant/audit storage, broker-held credentials and
owner approval UI remain. Do not expose owner mutation methods to MCP/IPC.

September 23 C04 follow-up: GitHub repository lookup now checks the pinned
candidate queue after approved-offering matching. Only an exact
`reviewed_repository_match` identity returns source ID, definition path,
revision and structural import status. This is review metadata, not install
approval or proof of first use. The linkding CapRover candidate test passes;
next add bounded live metadata inspection and a review/preview UI before any
new GitHub source can become installable.

Read [lessons.md](lessons.md) before Docker work. Run only one qualification
at a time, use private roots/project labels and never prune unrelated resources.
Choose the source explicitly where alternate definitions differ. Rebuild the
CLI before proving changed manifests. Pulling by digest does not restore a tag.

September 23 qualification scaling pivot: TypeSafe skill installed into Codex;
`scripts/triage-v1-tasks.py` used the owner-provided Jev key from outside the
repository to classify only public roster metadata. Its unreviewed 52-row
output is `catalog/v1-task-triage.json` (seven requests, 23,563 API tokens).
Read [qualification-at-scale.md](plans/qualification-at-scale.md) before more
per-app probes: build reusable fixture drivers and batch the existing managed
lifecycle harness. Jev is a planning/semantic aid, never release proof. The
offline `python scripts/qualification-preflight.py` queue shows 49/52 clean
metadata preflights; Adminer, Grafana and Vaultwarden have stale image reviews.
Those counts do not mean 49 installed or usable apps. The
Uptime Kuma experiment passed first-use setup but failed on a restart login
selector; its failed evidence and bespoke probe were removed. Counts remain
one task-qualified app of 50, two managed lifecycle passes. The owner's
uncommitted `CHANGELOG.md` remains untouched.

September 24 research follow-up: read
[qualification-at-1000-scale.md](research/qualification-at-1000-scale.md) before
more per-app probes. The strongest untried accelerator is constrained AI
browser action discovery followed by independent synthetic-marker assertions
and replay. Jev Ultrafast shows how Jev can select live UI controls with a
bounded executor; its early benchmark does not prove broad reliability. Pilot
across diverse apps first. Scale runtime checks with separate owned-WSL
workers, never concurrent runs on the present shared engine. No verification
count changed from this research.

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
