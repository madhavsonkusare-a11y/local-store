# Local Store V1 release tasks

Updated September 13, 2026. **This is the only active release task ledger.**
The handoff records the next action; detailed plans explain implementation;
research documents are proposals, not additional commitments.

## Release contract and owner decisions

V1 is a Windows desktop app store that installs supported self-hosted apps,
opens them in native windows, manages its own container engine, and gives
agents controlled access to the store and installed apps.

| Decision | Source / consequence |
| --- | --- |
| 100 distinct tested or verified offerings, selected for reach and diversity | Owner decision recorded September 10 in the backend plan; retained. 52 currently offered is a milestone, not a revised target. |
| Bundled engine required | Owner confirmed September 12. WSL prerequisites may still require consent, administrator access or restart; do not promise zero prerequisites. |
| V3 frontend | Owner is developing V3. Do not implement V2 as the final UI. Its approved assets and interaction findings remain reference material. |
| Signed Windows delivery and automatic updater required | Owner confirmed September 12, superseding the September 8 deferral. Code signing and updater signatures are separate requirements. |
| Agents can manage the store and access every offered app | Owner confirmed September 12. Use a shared access layer with multiple backends; prove a useful access path per app rather than promise universal API coverage. |
| Windows x64 is the shipping target | Existing decision retained. macOS/Linux CI builds are not native support certification. |
| Source-available apps allowed, with accurate license information | Existing owner decision retained. This does not establish redistribution rights for every image or asset. |
| Dark mode only, native per-app windows, reuse existing code | Existing product direction retained. No replacement installer or custom language build system. |

## Current baseline

Measured from integration commit `720ce23d46a8bcfd7b401b5ef94f840296f74749`:

- **52 offerings:** 3 recipes and 49 approved templates; 57 template manifests total.
- **1,678 discovery entries**, each with a local icon or generated monogram.
- CapRover and Runtipi importers, first-party definitions, typed setup, persisted
  secrets, digest pins, managed/shared storage, dependency health checks,
  one-shot jobs, auxiliary loopback ports and internal networks already exist.
- Per-app and registry cross-process locks, cancellation, rollback, CLI recovery
  and adoption, readiness, native windows and packaged Windows evidence exist.
- The default Rust suite, Clippy, UI/axe checks, metadata checks and all three
  platform builds passed [CI run 34695407242](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/34695407242).
- **Not implemented:** bundled engine, V3 production UI, agent gateway, broad
  meaningful first-use proof, signed delivery or automatic launcher updates.

The generic app proof demonstrates startup, an actionable page, persistence and
cleanup. It does not establish that every app can finish a real task. Some
individual recipes/probes have stronger evidence; inspect them separately.
There is no defensible overall percentage complete: the new engine, agent and
frontend requirements materially expand V1 beyond the old 33 tasks.

## Status and completion rules

`TODO` means no accepted completion evidence. `PARTIAL` names what exists.
`BLOCKED` names an external input. `DONE` requires the acceptance result and a
commit/evidence link. Planning, a mocked screen, parsing a definition, and an
image pull are not task completion.

Every implementation batch updates this file and the short handoff with:
task ID, files/commit, checks and result, unresolved limitation, next task.
Use one bounded batch at a time. Do not run concurrent Docker qualifications.

## 0 — Integration and scope

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| R01 | DONE | PRs #3–#6 merged; main `7d9e0d3` matches tested head `720ce23` exactly. Remaining feature/design branches removed after ancestry verification. Archive tag and saved drafts preserved; no open PRs. | — |
| R02 | DONE | Published this sole ledger, [documentation index](README.md) and short [handoff](agent-handoff.md). Retired seven superseded documents, organized plans/research, retained evidence and frozen V2 assets, and checked local links. | — |
| R03 | DONE | Froze [100 canonical apps](v1-app-roster.md) with [full acceptance matrix](../catalog/v1-roster.json), explicit membership and upstream snapshots. Offline generator validates 52 baseline + 48 candidates and input drift. Resources, managed-engine proof and agent access remain pending, not implied by selection. | R02 |

## 1 — Runtime foundation and proof contract

Detailed design: [bundled engine](plans/bundled-engine.md) and
[qualification](plans/qualification.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| E01 | PARTIAL | [Development payload](../engine/README.md) built/exported with five checksum-pinned engine packages and an exact 128-package inventory. Finish full dependency locking, signed-index provenance, rootfs notices/source obligations and supported Windows/WSL matrix before release approval. [Recorded checkpoint](plans/bundled-engine.md#e01-verification-checkpoint--september-13-2026). | R02 |
| E02 | PARTIAL | New installs persist local endpoint bindings; lifecycle, rollback, qualification and recovery use them, including post-deletion checks. [Real Memos context-change regression](evidence/engine-binding-memos-2026-09-13.json) and [real legacy adoption](evidence/engine-adoption-memos-2026-09-14.json) pass. Schema-2 WSL binding, typed Compose/bind/seed projection, lifecycle routing, rollback and dual-path recovery have fixture coverage. [Real managed-engine Memos proof](evidence/engine-wsl-coexistence-2026-09-14.json) passes pinned pull, projected bind mount, health, Windows loopback, restart and owned cleanup while Docker Desktop coexists. Product selection gating and full qualification through the selected managed binding remain. | E01 |
| E03 | PARTIAL | Journal, collision preflight, rootfs verification, bounded import and post-import identity/component/daemon verification are implemented. Recovery, immutable retry and token-gated unregistration authorization refuse uncertain ownership. [Real-host evidence](evidence/engine-wsl-coexistence-2026-09-14.json) proves verified import, token identity, exact packages, Docker/Compose readiness and a real app lifecycle. Prerequisites/elevation, product wiring and a clean Windows-without-Docker-Desktop proof remain. | E02 |
| E04 | TODO | Implement supervision, coexistence, repair and removal. Never stop an unrelated distro/daemon; do not stop background apps merely because the launcher closes. Engine removal and app-data removal are separate, explicit decisions. Test sleep/wake, crash, restart and existing Docker Desktop coexistence. | E03 |
| Q01 | PARTIAL | Evidence schema 2 fingerprints source kind/revision, normalized plan, requested and resolved images, OS/architecture, selected engine, Compose version, first-use probe and evidence level. Resume accepts only schema-2 records with identity, and exact identity comparison invalidates a pass when any proof-defining input changes; older JSON remains readable history. Wire current-identity comparison into batch orchestration and define freshness policy for time-sensitive upstream/image claims. | E02 |
| Q02 | PARTIAL | All-service count, running/health and successful-job checks now run after install/restart/reinstall; uncertain cleanup inventory fails qualification. [Real Docker stopped-worker regression](evidence/service-health-regression-2026-09-13.json) passes. Finish startup, image/disk and memory measurements with their limits, then prove on the managed engine. | Q01 |
| Q03 | TODO | Requalify all current offerings sequentially against the managed engine and stronger checks. Record failures/demotions, ownership-safe cleanup and engine identity. Do not inherit Docker Desktop proof across the engine swap. | E04, Q02 |
| Q04 | TODO | Build application-level probes for account creation/login and representative content. For AI apps, reuse a maintained mock HTTP server or fixture framework for deterministic model responses; distinguish stub integration from real-provider compatibility. Prove persistence of content, not merely a marker file. | Q02 |
| Q05 | TODO | Establish consistent backup/restore primitives for the app types V1 exposes to agent writes or updates. Include bind data, named volumes and credentials; quiesce the app or use its native backup. Restore into a fresh isolated install and read the content. Live folder copies are not database backup proof. | E04, Q04 |

## 2 — Agentic app store

Detailed design and coverage contract: [agent access](plans/agent-access.md).
This is now V1 scope. The older [agent research](research/agent-platform-and-differentiation.md)
contains options, not authorization to implement every proposed feature.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| A01 | TODO | Define one app capability directory and broker contract: list installed apps, describe available access methods/tools, invoke a typed tool. Every offering has a coverage row and required login/grant state. Tool discovery follows installed state; avoid dumping thousands of tools into model context. | R03, Q01 |
| A02 | TODO | Implement identity, grants, expiry/revocation, risk classes, approvals and redacted durable audit. Secrets stay in a broker/OS-backed store; logs and app content are untrusted. Test cross-app denial, revoked clients, forged approvals and credential leaks. Grants are not a sandbox against same-user shell access. | A01 |
| A03 | TODO | Expose store operations through a maintained MCP SDK and existing Rust functions. Start with stdio; HTTP requires authenticated sessions and Host/Origin checks. Preserve operation locks, cancellation and ownership checks. Agents cannot self-grant access or turn discovery entries into approved installs. | A02, E02 |
| A04 | TODO | Add access providers: official MCP proxy, typed app API/OpenAPI adapter, isolated browser session, and explicitly granted app data access. Pin/review providers and declare their capabilities. Browser sessions have app-scoped network/profile access and no launcher IPC or remote debugging on production WebViews. | A03 |
| A05 | TODO | Prove three different paths: an app API (e.g. Memos/Kanboard), an official MCP integration, and a web-only app through the isolated browser. Demonstrate read, write, denied destructive operation, revocation and audit with real installed apps. Use capability review to choose the actual three. | A04, Q04 |
| A06 | TODO | Complete agent coverage for every release offering. Each app must have a useful demonstrated access path and declare supported actions, setup and limitations. An inaccessible app blocks this gate until access is solved or the owner changes the roster/scope. Management-only access must not be labelled content access. | A05, C03 |
| A07 | TODO | Integrate agent connection, grants, pending approvals, credential onboarding and audit into V3 contracts. Back up client configuration before explicitly requested auto-configuration; never silently connect all detected agents. Test consent, expiry, inaccessible app and session recovery. | A02, F01 |
| A08 | TODO | Test prompt injection boundaries and recovery: malicious app content cannot grant permissions, reveal another app's secrets or bypass approval. Snapshot/restore only where consistent recovery is proven; external actions cannot be undone. Document and display residual same-user risks. | A05, Q05, A07 |

## 3 — App coverage and GitHub input

Architecture: [backend coverage](backend-app-coverage-plan.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| C01 | PARTIAL | Refresh candidate/ranking reports after importer changes and screen source/image maintenance before costly runs. Existing generators work; remeasure for the V1 roster and preserve alternate definitions. No sum of overlapping source counts becomes a unique-app total. | R03 |
| C02 | TODO | Add only capabilities needed by the selected roster, reusing existing definitions and normalized plans. Each change needs refusal tests, measured incremental coverage and one real proof. Nextcloud's program files need named storage; avoid broad binary-seed work just to force Calibre-Web through. | C01, E02 |
| C03 | PARTIAL | Reach 100 distinct accepted offerings: at least 48 additions from today's 52, plus replacements for any demotions. Every release offering passes the new managed-engine lifecycle and setup gates; separately list zero-input, setup-assisted and meaningful-task verification counts. | Q03, Q04, C02 |
| C04 | TODO | Resolve a public GitHub URL to a known approved app first; otherwise inspect bounded, commit-pinned deployment metadata and return a reviewable candidate/reason. Demonstrate known match, supported new candidate, redirects/invalid input and unsupported repo. Never run README commands. | A01, C01 |
| C05 | TODO | Show evidence-derived install/proof/agent capabilities in V3. Do not confuse catalog presence, image-platform metadata, lifecycle testing or agent tool presence with successful first use. External keys/accounts/GPU needs must appear before download. | Q01, A01, F01 |

Arbitrary source-only builds are a later extension unless the roster requires
them. When needed, evaluate an existing build provider and prove isolation;
a containerized final app does not make an untrusted build safe.

## 4 — V3 frontend and product contracts

V3 design is owned by Madhav. [V2 handoff](design/v2/HANDOFF.md) is reference,
not the active visual specification. V2's minimum window and screen decisions
are not automatically V3 decisions.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| F01 | BLOCKED | Obtain the V3 handoff: tokens/assets, screen/state inventory, viewport policy, keyboard/reduced-motion rules, engine setup, proof labels, agent grants/approvals and recovery. Record approval and which V2 decisions are retained. Backend contracts can proceed while design is in progress. | Owner V3 design |
| F02 | TODO | Add typed projections and persisted onboarding state, plus launcher recovery actions that reuse CLI ownership checks under locks. Use real backend values for summaries; do not parse Compose in UI. Distinguish operation activity from security audit. | E02, A02 |
| F03 | TODO | Implement V3 in usable increments over existing API, operations, readiness, setup and recovery modules. Preserve native app windows and accurate busy/error/cancel semantics. No prototype fixtures, annotations or fabricated metrics ship. | F01, F02 |
| F04 | TODO | Validate complete flows: fresh engine setup → install → app task → agent access → restart → recovery/removal. Preserve behavioral tests, replace only design-obsolete snapshots, run axe/focus/motion/asset checks at V3-approved sizes and in the Windows WebView. | F03, A07, C05 |

## 5 — Signing, updates and release

Publication procedure: [PUBLISH.md](../PUBLISH.md). This plan does not generate
keys, submit applications, incur costs or publish a release.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| S01 | BLOCKED | Choose an eligible Windows signing provider and establish owner-controlled identity/secrets. Reverify provider conditions, including bundled data and source-available offerings. Record certificate/identity ownership and secure CI access. | Owner/provider enrollment |
| S02 | TODO | Sign Windows binaries/installers in CI; verify signature, publisher, timestamp and artifact provenance on downloaded artifacts. Code signing alone is not a guarantee of SmartScreen reputation. | S01 |
| S03 | BLOCKED | Establish owner-controlled updater keys and HTTPS channel, then implement opt-in/deferrable signed updates with error/retry behavior. Reject invalid signature, altered payload and wrong-channel/version data. Never commit private keys. | Owner keys/channel |
| S04 | TODO | Prove launcher and managed-engine upgrades separately. Preserve registry, app data, secrets and running-app policy through failure/restart. Define engine rollback/repair compatibility; do not assume database migrations are reversible. | S03, E04, Q05 |
| S05 | TODO | Prove supported app-version upgrades for the launch maintenance promise. At minimum document withheld updates and a supported restore route; do not claim universal rollback from reinstall tests. | Q05 |
| R04 | TODO | Run clean Windows release-candidate install, first launch, engine bootstrap, native app windows, protocol/shortcut, agent access, signed update and uninstall with data preservation. Include restricted/failed prerequisites and supported older-version migration. | F04, S02, S04, C03, A06, A08 |
| R05 | TODO | Release review: app/asset notices, security/privacy and support limits match code; every required row above has evidence. Download actual tagged artifacts and verify signatures, hashes and attestation. Publish only with explicit owner authorization. | R04, S05 |

## Recommended execution order

1. Finish R01 and commit this documentation cleanup (R02).
2. E01/E02 + Q01/Q02: choose engine packaging, add the seam and evidence contract.
   In the same planning batch, establish A01 so proof records cover agent access.
3. E03/E04 and Q03: prove the managed engine before spending time qualifying 48+
   more apps against the old runtime. Start S01/S03 enrollment in parallel with
   owner work because credentials and providers have external lead time.
4. A02–A05 and Q04/Q05: demonstrate store control and three distinct app-access
   methods with meaningful tasks, permissions and recovery.
5. C01–C05 and A06: expand in bounded, reviewed batches toward 100, with agent
   access measured alongside installation. Do not schedule a second proof run
   over the same host while one is active.
6. Integrate F02 contracts as needed; F03/F04 start only after V3 approval.
7. Finish security, signing/update proof and the actual Windows release gate.

## Explicitly outside the current V1 commitment

Native macOS/Linux support certification; a public community publishing
service; arbitrary GitHub source builds; paid tiers/Store sales; cross-machine
management; Tailscale/remote hosting; automatic sleeping apps; global SQL/exec
access; unrestricted agent browser control; automatic image upgrades without
application migration evidence. None is implemented by merely being described
in a research paper or prototype.

## Relationship to the original 33 tasks

[The reconciled Hermes ledger](upgrade-status.md) records the original work.
It is a historical traceability table, not a second queue. The former signed
update deferral is revoked by the current owner decision; tasks 29–31/33 must
be re-proven for the actual signed V1 candidate. The 100-app goal, managed
engine, agent gateway and V3 work are additional requirements.
