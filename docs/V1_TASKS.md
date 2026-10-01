# Local Store V1 release tasks

Updated October 1, 2026. **This is the only active release task ledger.**
The handoff records the next action; detailed plans explain implementation;
research documents are proposals, not additional commitments.

## Release contract and owner decisions

The product goal is a Windows desktop app store that installs supported self-hosted apps,
opens them in native windows, manages its own container engine, and gives
agents controlled access to the store and installed apps. The source is
already public on [GitHub](https://github.com/madhavsonkusare-a11y/local-store).
The **V1 GitHub milestone is published as a source-only release**: a tagged snapshot with
accurate docs, license/notices, and a reviewed source archive. It has no Windows
installer or automatic update channel. The Windows product remains active
development after that source milestone; its functional gates below are not
prerequisites to publishing source.

| Decision | Source / consequence |
| --- | --- |
| 10 managed-engine-verified apps for the Windows product | Owner reduced the product launch target from 50 to 10 on September 30. The 10-app proof gate now passes; the frozen 100-app roster remains future research and replacement capacity. |
| Bundled engine required for the Windows product | Owner confirmed September 12. WSL prerequisites may still require consent, administrator access or restart; do not promise zero prerequisites. It does not block the source-only tag. |
| Approved V2 frontend for the Windows product | Owner selected the already approved V2 handoff on September 28; there is no V3. Extend V2 for bundled-engine and agent flows. It does not block the source-only tag. |
| Source-only V1 GitHub release; no signing keys | Owner confirmed September 30. The repository is already public. The next V1 release tag contains source only, with no installer artifact, automatic updater, or signing requirement. Windows distribution and signed automatic updates are later milestones. |
| Agents can manage the store and access every offered app in the Windows product | Owner confirmed September 12. Use a shared access layer with multiple backends; prove a useful access path per app rather than promise universal API coverage. |
| Windows x64 is the only active binary target | Owner reconfirmed September 19. Existing macOS/Linux code and staging support remain intact for later scope. The source-only release contains no platform binary. |
| Source-available apps allowed, with accurate license information | Existing owner decision retained. This does not establish redistribution rights for every image or asset. |
| Dark mode only, native per-app windows, reuse existing code | Existing product direction retained. No replacement installer or custom language build system. |

## Current baseline

The integration baseline began at commit `720ce23d46a8bcfd7b401b5ef94f840296f74749`;
later evidence and task completions are recorded below:

- **52 offerings:** 3 recipes and 49 approved templates; 57 template manifests total.
- **1,678 discovery entries**, each with a local icon or generated monogram.
- CapRover and Runtipi importers, first-party definitions, typed setup, persisted
  secrets, digest pins, managed/shared storage, dependency health checks,
  one-shot jobs, auxiliary loopback ports and internal networks already exist.
- Per-app and registry cross-process locks, cancellation, rollback, CLI recovery
  and adoption, readiness, native windows and packaged Windows evidence exist.
- The default Rust suite, Clippy, UI/axe checks, metadata checks and all three
  platform builds passed [CI run 34695407242](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/34695407242).
- **Remaining Windows product work:** bundled-engine packaging/setup, complete approved V2
  flows, useful agent access across the release roster, backup/recovery and a
  clean Windows release-candidate run. The selected 10-app managed task gate
  now passes; signing and automatic updates are later work.

The generic app proof demonstrates startup, an actionable page, persistence and
cleanup. It does not establish that every app can finish a real task. Some
individual recipes/probes have stronger evidence; inspect them separately.
The public repository and [tagged source-only V1 release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0)
are complete. In the separate Windows-product roadmap below,
**9 of 31 active rows are DONE (29.0%)**. Two qualification rows now close
because all 10 selected apps have current managed-engine lifecycle and
meaningful-task evidence. Five signing/update rows are deferred, not completed,
and are excluded from that product-work denominator. Partial work earns no
fraction of a DONE row. Task count is not a calendar-time or risk estimate.

## Status and completion rules

`TODO` means no accepted completion evidence. `PARTIAL` names what exists.
`BLOCKED` names an external input for an in-scope task. `DEFERRED` means the
owner moved a task beyond this GitHub V1 milestone; it is excluded from the
percentage, never counted as done. `DONE` requires the acceptance result and a
commit/evidence link. Planning, a mocked screen, parsing a definition, and an
image pull are not task completion.

Every implementation batch updates this file and the short handoff with:
task ID, files/commit, checks and result, unresolved limitation, next task.
Use one bounded batch at a time. Do not run concurrent Docker qualifications.

## Source-only V1 GitHub release gate

This publication gate is **complete** for
[source-v1.0.0](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0),
published October 1 at `a53e370d6b6280eb3d54978ce92e94a932450025`.
The tag is outside the `v*` installer workflow and remains immutable.
See [verification evidence](evidence/source-release-verification-2026-10-01.json)
and [PUBLISH.md](../PUBLISH.md). Future source releases need a newly reviewed
commit and tag, without implying Windows product completion.

| ID | Status | Acceptance |
| --- | --- | --- |
| G01 | DONE | [GitHub repository](https://github.com/madhavsonkusare-a11y/local-store) is public, with `main` as the default branch; verified September 30. This is source visibility, not a V1 tag. |
| G02 | DONE | [October 1 source audit](evidence/source-release-audit-2026-10-01.md) reviews secrets, notices, build instructions and limitations. Pinned Gitleaks passes on the final tagged source; 512 Rust dependency licenses pass. Eight undeclared-license icons use existing monograms, CapRover's original license is preserved, and corrected embedded-source build/CLI, 12 catalog tests, icon/catalog checks and 10-app validation pass. [Published-source verification](evidence/source-release-verification-2026-10-01.json) records the final result. |
| G03 | DONE | Saved [V1 source release notes](releases/source-v1.0.0.md) and selected `source-v1.0.0`, outside the `v*` installer workflow. The owner's October 1 request to complete the source release authorizes publication. The annotated tag records the exact reviewed source commit. No installer, signing key or updater endpoint is needed. |
| G04 | DONE | [GitHub Release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0) is public and latest, at audited commit `a53e370`. Both downloaded ZIP and tar.gz contain exactly 2,700 files; every file matches its committed Git blob. Notes match the saved Markdown, uploaded asset count is zero, and no installer or private-key file is in either archive. The published ZIP passes secret and icon checks; catalog generation was checked in the full checkout because it requires Git history. [Machine-readable proof](evidence/source-release-verification-2026-10-01.json) records tag/commit, archive hashes and inventory. |

**Source-release progress: 4/4 DONE (100%).** The percentage is deliberately
separate from the Windows-product roadmap below. The source milestone is
complete; engine setup, complete V2 flows and agent access remain in development.

## 0 — Integration and scope

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| R01 | DONE | PRs #3–#6 merged; main `7d9e0d3` matches tested head `720ce23` exactly. Remaining feature/design branches removed after ancestry verification. Archive tag and saved drafts preserved; no open PRs. | — |
| R02 | DONE | Published this sole ledger, [documentation index](README.md) and short [handoff](agent-handoff.md). Retired seven superseded documents, organized plans/research, retained evidence and frozen V2 assets, and checked local links. | — |
| R03 | DONE | Froze a [100-app planning roster](v1-app-roster.md) with [full acceptance matrix](../catalog/v1-roster.json), explicit membership and upstream snapshots. Offline generator validates 52 baseline + 48 candidates and input drift. It is a future sourcing and replacement pool; V1 release requires 10 verified selections from the current 52 offerings. Resources, managed-engine proof and agent access remain pending, not implied by selection. | R02 |

## 1 — Runtime foundation and proof contract

Detailed design: [bundled engine](plans/bundled-engine.md) and
[qualification](plans/qualification.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| E01 | PARTIAL | [Development payload](../engine/README.md) built/exported with five checksum-pinned engine packages. Its exact 128-package inventory is now a full fail-closed build lock: Dockerfile and exporter reject installed-package drift. The lock matches the recorded development payload. A dated Ubuntu snapshot now makes a clean build reproduce all 128 locked versions, including `libapparmor1` .7; [September 30 evidence](evidence/engine-snapshot-development-2026-09-30.json) records the 570,632,192-byte rootfs and matching package-inventory hash. The earlier [drift diagnostic](evidence/engine-inventory-drift-2026-09-28.json) remains historical. A second clean build captured hashes for 34 Ubuntu .deb files and 39 APT index files in the [provenance manifest](evidence/engine-ubuntu-provenance-2026-09-30.json), while the exact 128-package lock still matched. Independently verify signed indexes, reconcile archive bytes to the transitive lock, review rootfs notices/source obligations and prove the supported Windows/WSL matrix before release approval. [Recorded checkpoint](plans/bundled-engine.md#e01-verification-checkpoint--september-13-2026). | R02 |
| E02 | PARTIAL | New installs persist local endpoint bindings; lifecycle, rollback, qualification and recovery use them, including post-deletion checks. [Real Memos context-change regression](evidence/engine-binding-memos-2026-09-13.json) and [real legacy adoption](evidence/engine-adoption-memos-2026-09-14.json) pass. Schema-2 WSL binding, typed Compose/bind/seed projection, lifecycle routing, rollback and dual-path recovery have fixture coverage. [Real managed-engine Memos proof](evidence/engine-wsl-coexistence-2026-09-14.json) passes pinned pull, projected bind mount, health, Windows loopback, restart and owned cleanup while Docker Desktop coexists. An explicitly selected WSL binding now has a [full lifecycle/resource proof](evidence/memos-managed-resource-2026-09-23.json) after journal, distro and token ownership checks. Product selection gating and clean Windows proof remain. | E01 |
| E03 | PARTIAL | Journal, collision preflight, rootfs verification, bounded import and post-import identity/component/daemon verification are implemented. Recovery, immutable retry and token-gated unregistration authorization refuse uncertain ownership. A Verified journal now reports Ready only when the bounded external token still matches the journal and a direct in-distro comparison succeeds; mismatch returns ManualReview. The launcher reports localized-output-independent WSL prerequisites and read-only free space on the planned local WSL data volume (or unknown); no unsupported disk minimum is asserted. [Real-host evidence](evidence/engine-wsl-coexistence-2026-09-14.json) proves verified import, token identity, exact packages, Docker/Compose readiness and a real app lifecycle. Setup consent/elevated execution, a measured disk minimum, recovery actions and a clean Windows-without-Docker-Desktop proof remain. | E02 |
| E04 | PARTIAL | The launcher engine-status contract and `engine status` CLI distinguish unchecked, responsive and unresponsive daemon state, probing only after verified WSL ownership. Explicit `engine repair` holds a cross-process operation lock, starts Docker only inside that owned distro, skips an already responsive daemon and rechecks readiness. Fixture tests cover ownership refusal and exact command targeting. A first V2 Settings status/repair surface now uses this IPC and shows repair only for verified, unresponsive ownership; focused browser proof passes. Coexistence, removal and sleep/wake/crash/restart proof remain; launcher close must not stop background apps. Engine and app-data removal are separate decisions. | E03 |
| Q01 | DONE | Evidence schema 2 fingerprints the exact source locator, adapter and revision; normalized plan; requested and resolved images; OS/architecture; selected engine; Compose version; first-use probe (including actual script source for script probes); and evidence level. Batch scheduling reruns changed inputs, retains current passes and optionally retries current failures. Qualification proof expires after 30 days, source review after 90 days and image observation after 30 days; missing, malformed or implausibly future dates refuse. Older JSON remains readable history without retaining stronger claims. Focused fingerprint and freshness tests and strict Clippy pass. | E02 |
| Q02 | PARTIAL | All-service count, running/health and successful-job checks run after install/restart/reinstall; uncertain cleanup inventory fails qualification. Evidence records time-to-first-answer, idle/peak memory, managed bind and named-volume storage, and immutable image virtual size. Named volumes require verified project/volume labels and local driver; missing, foreign, partial and unmeasurable volumes fail. Historical evidence without volume measurements is not reusable. [Real managed-engine Memos](evidence/memos-managed-resource-2026-09-23.json) and [n8n](evidence/n8n-managed-workflow-2026-09-30.json) each passed 18 steps; [Gitea](evidence/gitea-managed-repository-2026-09-30.json), [WordPress](evidence/wordpress-managed-post-2026-09-30.json) and [Jellyfin](evidence/jellyfin-managed-media-2026-09-30.json) each passed 19 with named volumes; [Immich](evidence/immich-managed-photo-2026-09-30.json) passed 21 with four services. Each has three resource samples, restart/reinstall and exact cleanup. Per-service memory, CPU and PID ceilings render from the plan; [Flatnotes](evidence/flatnotes-managed-content-2026-09-30.json) now has reviewed 512 MiB, 2 CPU and 512 PID limits, checked against Docker's actual container configuration after install, restart and keep-data reinstall. Select and prove safe limits for the other nine apps before Q02 closes. | Q01 |
| Q03 | DONE | The selected 10 of 52 offerings (Memos, n8n, Flatnotes, Gitea, Immich, Jellyfin, Kanboard, PrivateBin, Uptime Kuma and WordPress) passed isolated managed-WSL lifecycle/resource runs with recorded engine identity, restart, keep-data reinstall and ownership-safe cleanup. The [curated ledger](../catalog/v1-qualified-apps.json) and `python scripts/check-v1-qualified.py --release-gate` validate all 10; the last two proofs landed in `44c64b6` and `05e2e62`, and bounded Flatnotes was re-proven in `1e9125e`. The other 42 offerings are unqualified discovery/replacement candidates. Q02 owns limits for the remaining nine; E04 owns engine resilience. | E02, Q01 |
| Q04 | DONE | All 10 selected apps passed a meaningful exact-state task after install, restart and keep-data reinstall: Memos private memo, Flatnotes note/login, Uptime Kuma HTTP monitor, PrivateBin encrypted paste, Kanboard moved task, n8n workflow result, Gitea private repository/commit, WordPress published post, Jellyfin exact audio playback and Immich exact photo search/download. The [curated ledger](../catalog/v1-qualified-apps.json) fingerprints each task probe and evidence file; the 10/10 release-count validator passes. Kanboard's default admin credential and agent-content access remain separate security/agent gates, not extra app-task proof. | Q03 |
| Q05 | TODO | Establish consistent backup/restore primitives for the app types V1 exposes to agent writes or updates. Include bind data, named volumes and credentials; quiesce the app or use its native backup. Restore into a fresh isolated install and read the content. Live folder copies are not database backup proof. | E04, Q04 |

## 2 — Agentic app store

Detailed design and coverage contract: [agent access](plans/agent-access.md).
This is now V1 scope. The older [agent research](research/agent-platform-and-differentiation.md)
contains options, not authorization to implement every proposed feature.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| A01 | DONE | `catalog/agent-access.json` has one fail-closed coverage row per 52 approved offerings. The Rust directory consumer rejects missing/duplicate rows, lists only installed apps, describes content method/setup and unavailable login/grant state without inventing access, lists a bounded management tool, and dispatches typed status lookup only for an installed ID. This is an in-process broker contract, not an externally exposed agent gateway; A02/A03 must add identity, grants and transport before any agent calls it. Three focused tests and strict Clippy pass. | R03, Q01 |
| A02 | PARTIAL | In-process grants scope client/app/action, expiry and revocation; OS-random one-use approvals gate writes/destructive actions. The internal [policy store](../src/agent_policy/store.rs) locks across processes, atomically persists versioned grants and bounded redacted audit, refuses corrupt/future files, and keeps approval tickets memory-only. Owner-only credential primitives store hashes of random bearer tokens; the read-only gateway verifies the presented credential on every request, then persists grant audit before status dispatch. Focused spoofing/revocation/redaction tests pass. A local owner CLI now enrolls, lists and revokes hashed bearer credentials; revocation removes old grants before re-enrollment. A second local owner CLI grants or revokes status for an enrolled client and installed app with a 1–720 hour expiry. V2 credential onboarding, broker-held app secrets and approved write/destructive actions remain. Grants are not a sandbox against same-user shell access. | A01 |
| A03 | PARTIAL | A [transport-neutral gateway](../src/agent_gateway.rs) verifies an owner-enrolled bearer credential on each request, checks a persisted status grant and saves audit before installed-app status lookup. Nine focused gateway/auth tests and four MCP handler tests pass, including spoofed metadata, live revocation, bounded status grants and re-enrollment denial. A development-only stdio MCP binary exposes read-only installed-app status; every call rechecks the owner-enrolled bearer credential, persisted grant and audit. Client metadata cannot identify a caller. Windows release staging now requires a target-named MCP sidecar and includes it in Tauri bundle configuration, with focused staging tests; grant-scoped MCP discovery lists only installed apps with current status grants, rechecking credential and revocation on each call; an unsigned local installer build passed launcher/sidecar identity checks. Owner-granted 1–24 hour start/stop scopes now authorize MCP lifecycle calls with persisted per-call audit and a per-app operation lock across registry read and dispatch. [Real managed-engine Memos proof](evidence/memos-agent-lifecycle-2026-09-30.json) now passes owner-granted MCP stop/start/status, revocation denial and secret-free audit within an isolated install, then restart/reinstall/cleanup. Cancellation and approved install/uninstall remain. Agents cannot self-grant access or turn discovery entries into approved installs. | A02, E02 |
| A04 | TODO | Add access providers: official MCP proxy, typed app API/OpenAPI adapter, isolated browser session, and explicitly granted app data access. Pin/review providers and declare their capabilities. Browser sessions have app-scoped network/profile access and no launcher IPC or remote debugging on production WebViews. | A03 |
| A05 | TODO | Prove three different paths: an app API (e.g. Memos/Kanboard), an official MCP integration, and a web-only app through the isolated browser. Demonstrate read, write, denied destructive operation, revocation and audit with real installed apps. Use capability review to choose the actual three. | A04, Q04 |
| A06 | TODO | Complete agent coverage for every release offering. Each app must have a useful demonstrated access path and declare supported actions, setup and limitations. An inaccessible app blocks this gate until access is solved or the owner changes the roster/scope. Management-only access must not be labelled content access. | A05, C03 |
| A07 | TODO | Integrate agent connection, grants, pending approvals, credential onboarding and audit into approved V2 contracts. Back up client configuration before explicitly requested auto-configuration; never silently connect all detected agents. Test consent, expiry, inaccessible app and session recovery. | A02, F01 |
| A08 | TODO | Test prompt injection boundaries and recovery: malicious app content cannot grant permissions, reveal another app's secrets or bypass approval. Snapshot/restore only where consistent recovery is proven; external actions cannot be undone. Document and display residual same-user risks. | A05, Q05, A07 |

## 3 — App coverage and GitHub input

Architecture: [backend coverage](backend-app-coverage-plan.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| C01 | DONE | Regenerated the 606-definition candidate queue and reach ranking offline from checksum-pinned source archives through the current Rust importers, then regenerated the 100-member planning roster without changing membership or promoting a definition. Five previously importable source definitions are now refused by current command-variable rules (moneroblock, siyuan, ghostfolio, Runtipi nocodb and CapRover outline); the top-150 structural/reach set has 89 importable and 61 blocked. Alternate definitions remain separate, and overlapping source counts are not presented as verified unique apps. Queue tests and roster drift check pass. Refresh again whenever importers or pinned sources change. | R03 |
| C02 | TODO | Add only capabilities needed by the selected roster, reusing existing definitions and normalized plans. Each change needs refusal tests, measured incremental coverage and one real proof. Nextcloud's program files need named storage; avoid broad binary-seed work just to force Calibre-Web through. | C01, E02 |
| C03 | PARTIAL | Release 10 selected offerings from today's 52 baseline. [Curated qualification ledger](../catalog/v1-qualified-apps.json) and `python scripts/check-v1-qualified.py` validate manifest/evidence/probe hashes, managed-Windows identity, freshness, lifecycle steps and task proof in CI. The 10-app proof-count gate now passes: Memos private memo, Flatnotes exact note, Gitea private repository and commit, Immich exact photo upload/search/download, Jellyfin exact sample playback, Kanboard moved task, PrivateBin encrypted paste, Uptime Kuma HTTP monitor, n8n saved workflow and WordPress published post. Release still needs capability display, app-specific setup polish, resource limits and the other V1 gates. Separately list zero-input, setup-assisted and verified-task counts. Replace a demotion only through a reviewed promotion from the frozen planning roster. | Q03, Q04, C02 |
| C04 | PARTIAL | Launcher-only GitHub lookup parses strict HTTPS repository URLs and resolves exact approved/reviewed identities without installing candidate definitions. Opt-in live GitHub API inspection now bounds response size, time and redirects, verifies canonical repository identity and pins a default-branch commit before final matching; mocked HTTP refusal tests cover alias and ref confusion. A candidate review projection now verifies pinned source-archive provenance and shows the archive digest, definition source, structural/maintenance/source/qualification stage, manifest shape and remaining checks without approving installation. Focused offline refusal tests and an opt-in live GitHub boundary test pass. No live host proof, end-to-end candidate promotion pipeline or V2 install-preview UI exists yet. Metadata is review context, never install approval or first-use proof. | A01, C01 |
| C05 | TODO | Show evidence-derived install/proof/agent capabilities in V2. Do not confuse catalog presence, image-platform metadata, lifecycle testing or agent tool presence with successful first use. External keys/accounts/GPU needs must appear before download. | Q01, A01, F01 |

The owner's September 30 launch priority is **popular, widely used apps**.
The ten proven apps are Memos, Flatnotes, Kanboard, PrivateBin, Uptime Kuma,
[n8n](https://github.com/n8n-io/n8n), [Gitea](https://github.com/go-gitea/gitea),
[WordPress](https://wordpress.org/), [Jellyfin](https://github.com/jellyfin/jellyfin)
and [Immich](https://github.com/immich-app/immich). All ten passed a managed-engine
task through restart and keep-data reinstall. n8n passed a saved workflow and exact
execution-result task; Gitea passed a private repository and exact commit;
WordPress passed an exact published post; Jellyfin passed exact sample playback;
Immich passed exact photo search and download. Keep proof fresh as images and
definitions change. If a selected app must be demoted, review a widely used
alternative such as Vaultwarden or Grafana before changing the qualified ledger.
changedetection.io is excluded from this launch shortlist at the owner's request.

Arbitrary source-only builds are a later extension unless the roster requires
them. When needed, evaluate an existing build provider and prove isolation;
a containerized final app does not make an untrusted build safe.

## 4 — Approved V2 frontend and product contracts

The [approved V2 handoff](design/v2/HANDOFF.md) is the active visual specification
per the owner's September 28 decision. Extend its engine and agent permission
states within the frozen visual system, with truthful status and proof labels.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| F01 | DONE | The owner selected the already [approved V2 handoff](design/v2/HANDOFF.md) for V1 on September 28; no V3 handoff is required. Its freeze supplies tokens/assets, screen/state inventory, viewport, keyboard and motion rules. F02/F03 must extend engine and agent permission states without inventing proof. | Owner decision |
| F02 | PARTIAL | A [launcher projection](../src/launcher_projection.rs) and launcher-only IPC now expose typed live engine status, installed-app counts, verified retained-setup snapshots and persisted onboarding viewed steps; viewed steps are not consent or engine readiness. Launcher repair/adopt/discard commands delegate to existing locked CLI recovery functions and recheck ownership. Focused projection/store tests, the combined library suite and all-target strict Clippy pass. The V2 Settings engine status/repair surface is wired to live IPC with a focused browser state-transition test. V2 onboarding vocabulary/consent, real launcher recovery parity and result UX remain. | E02, A02 |
| F03 | PARTIAL | The production launcher now loads the frozen V2 token sheet and approved fonts/brand mark; the current shell uses V2 canvas, type, spacing and navigation values. Focused dark-mode/keyboard/Discover checks pass. Settings now has a managed-engine status/repair row; an unsigned local NSIS bundle built and passed launcher-versus-sidecar identity verification on September 29. The V2 Overview reads live saved/managed/linked/attention counts, and My Apps now has a selectable master/detail view with actual app state, on-demand logs, keyboard selection and truthful linked-app status; focused browser/axe checks pass. Implement approved V2 in usable increments over existing API, operations, readiness, setup and recovery modules; extend the frozen visual system for bundled-engine and agent consent states. Preserve native app windows and accurate busy/error/cancel semantics. No prototype fixtures, annotations or fabricated metrics ship. | F01, F02 |
| F04 | TODO | Validate complete flows: fresh engine setup → install → app task → agent access → restart → recovery/removal. Preserve behavioral tests, replace only design-obsolete snapshots, run axe/focus/motion/asset checks at V2-approved sizes and in the Windows WebView. | F03, A07, C05 |

## 5 — Later Windows distribution and product release

The source-only GitHub release uses G01–G04 above. The rows here track later
Windows distribution and product maintenance. No signing provider, private key
or HTTPS update channel is needed to publish the source milestone.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| S01 | DEFERRED | Before signed Windows distribution, choose an eligible code-signing provider and owner-controlled identity/secrets; verify provider terms and secure CI access. This is not a source-release gate. | Later owner/provider enrollment |
| S02 | DEFERRED | Sign Windows binaries/installers in CI and verify publisher, timestamp and downloaded-artifact provenance. Code signing alone does not guarantee SmartScreen reputation. | S01 |
| S03 | DEFERRED | Before automatic updates, establish owner-controlled updater keys and an HTTPS channel; implement opt-in/deferrable signed updates and reject tampering, wrong channel/version and failed downloads. Never commit private keys. | Later owner keys/channel |
| S04 | DEFERRED | Prove launcher and managed-engine upgrades separately, preserving registry, app data, secrets and running-app policy through failure/restart. Define repair/rollback compatibility without assuming database migrations reverse. | S03, E04, Q05 |
| S05 | DEFERRED | Before promising supported app-version upgrades, prove the update and restore route; do not infer rollback from keep-data reinstall tests. The source-only release makes no automatic app-update promise. | Q05 |
| R04 | TODO | For a later Windows product preview, run a clean-machine install, first launch, engine bootstrap, native app windows, protocol/shortcut, agent access and uninstall with data preservation. Include restricted/failed prerequisites. This is not required to publish source. | F04, C03, A06, A08 |
| R05 | TODO | Before a later Windows product release, review app/asset notices, security/privacy and support limits against actual code, and verify the downloaded Windows artifacts. The source-only release review is G02–G04. | R04, Q05 |

## Recommended execution order

1. G01–G04 are complete. Keep `source-v1.0.0` immutable; its source-publication
   evidence does not certify a Windows installer or complete product flows.
2. Continue E01–E04 and Q02 for the usable Windows product. Q03/Q04's 10-app
   managed task proof is complete; keep it fresh when definitions or images move.
3. Complete A02–A08, C03–C05 and F02–F04 for the approved V2 experience and
   useful, permissioned agent access. Run Docker qualifications serially on
   the shared host.
4. Run R04/R05 before distributing a Windows product preview. Start S01–S05
   only when a signed, automatic-update distribution is actually planned.

## Explicitly outside the current V1 commitment

Native macOS/Linux support certification; a public community publishing
service; arbitrary GitHub source builds; paid tiers/Store sales; cross-machine
management; Tailscale/remote hosting; automatic sleeping apps; global SQL/exec
access; unrestricted agent browser control; automatic image upgrades without
application migration evidence; Windows installer publication and automatic
updates for the source-only milestone. None is implemented by merely being described
in a research paper or prototype.

## Relationship to the original 33 tasks

[The reconciled Hermes ledger](upgrade-status.md) records the original work.
It is a historical traceability table, not a second queue. The September 30
source-only decision defers signed Windows delivery and automatic updates;
the old tasks 29–31/33 apply to a later Windows distribution. The managed engine, agent
gateway and V2 work are additional requirements; the 100-app roster is retained
for future expansion rather than required for V1 release.
