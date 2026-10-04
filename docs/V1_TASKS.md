# Local Store V1 release tasks

Updated October 4, 2026. **This is the sole release task ledger; work is currently shelved.**
The handoff records the next action; detailed plans explain implementation;
research documents are proposals, not additional commitments.

Development is paused at the owner's request on October 4, 2026. Historical Windows progress remains **23/31 (74.2%)**. Generated installers, app data and the managed engine were removed; see [agent-handoff.md](agent-handoff.md) before resuming.

## Release contract and owner decisions

The product goal is a Windows desktop app store that installs supported self-hosted apps,
opens them in native windows, manages its own container engine, and gives
agents controlled access to the store and installed apps. The source is
already public on [GitHub](https://github.com/madhavsonkusare-a11y/local-store).
The **V1 GitHub milestone is published as a source-only release**: a tagged snapshot with
accurate docs, license/notices, and a reviewed source archive. It has no Windows
installer or automatic update channel. Windows product development is paused after that source milestone; its functional gates below are not
prerequisites to publishing source.

| Decision | Source / consequence |
| --- | --- |
| 10 managed-engine-verified apps for the Windows product | Owner reduced the product launch target from 50 to 10 on September 30. The 10-app proof gate now passes; the frozen 100-app roster remains future research and replacement capacity. |
| Bundled engine required for the Windows product | Owner confirmed September 12. WSL prerequisites may still require consent, administrator access or restart; do not promise zero prerequisites. It does not block the source-only tag. |
| Approved V2 frontend for the Windows product | Owner selected the already approved V2 handoff on September 28; there is no V3. Extend V2 for bundled-engine and agent flows. It does not block the source-only tag. |
| Source-only V1 GitHub release; no signing keys | Owner confirmed September 30. The repository is already public. The published V1 release tag contains source only, with no installer artifact, automatic updater, or signing requirement. Windows distribution and signed automatic updates are later milestones. |
| Agents can manage the store and access every offered app in the Windows product | Owner confirmed September 12. Use a shared access layer with multiple backends; prove a useful access path per app rather than promise universal API coverage. |
| Windows x64 is the only active binary target | Owner reconfirmed September 19. Existing macOS/Linux code and staging support remain intact for later scope. The source-only release contains no platform binary. |
| Source-available apps allowed, with accurate license information | Existing owner decision retained. This does not establish redistribution rights for every image or asset. |
| Dark mode only, native per-app windows, reuse existing code | Existing product direction retained. No replacement installer or custom language build system. |

## Current baseline

The integration baseline began at commit `720ce23d46a8bcfd7b401b5ef94f840296f74749`;
later evidence and task completions are recorded below:

- **53 offerings:** 3 recipes and 50 approved templates; 58 template manifests total.
- **1,678 discovery entries**, each with a local icon or generated monogram.
- CapRover and Runtipi importers, first-party definitions, typed setup, persisted
  secrets, digest pins, managed/shared storage, dependency health checks,
  one-shot jobs, auxiliary loopback ports and internal networks already exist.
- Per-app and registry cross-process locks, cancellation, rollback, CLI recovery
  and adoption, readiness, native windows and packaged Windows evidence exist.
- The default Rust suite, Clippy, UI/axe checks, metadata checks and all three
  platform builds passed [CI run 34695407242](https://github.com/madhavsonkusare-a11y/local-store/actions/runs/34695407242).
- **Remaining Windows product work:** engine provenance/source delivery and
  packaging/setup, native lifecycle acceptance, the strict approved V2 capture
  gate, complete native flows and a clean Windows release-candidate run.
  Scoped agent access and protected backup/recovery have accepted evidence. The selected 10-app managed task gate
  now passes; signing and automatic updates are later work.

The generic app proof demonstrates startup, an actionable page, persistence and
cleanup. It does not establish that every app can finish a real task. Some
individual recipes/probes have stronger evidence; inspect them separately.
The public repository and [tagged source-only V1 release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0)
are complete. In the separate Windows-product roadmap below,
**23 of 31 active rows are DONE (74.2%)**. The October 1–2 batch completes
resource ceilings for all ten apps, protected backup/restore, owner agent
controls, evidence-derived capability display and real launcher recovery.
A06 now proves useful content access for all ten launch apps; A08 proves the
untrusted-content and bounded recovery contract. The earlier 70% target was met with 22 accepted rows. C04 now completes
reviewed GitHub candidate promotion with real Linkding evidence; eight active
rows remain. The project is now paused; earlier percentage targets do not authorize further development.
Five signing/update rows are deferred, not completed,
and are excluded from that product-work denominator. Partial work earns no
fraction of a DONE row. Task count is not a calendar-time or risk estimate.

## October 4 local completion checkpoint

The owner cannot currently provide a clean Windows machine. Local implementation,
source/notices preparation, browser behavior and temporary-engine removal are
complete within their recorded scopes. [Checkpoint](evidence/windows-local-checkpoint-2026-10-04.json)
and [local acceptance review](evidence/windows-v2-local-acceptance-2026-10-04.md)
separate finished local work from the remaining strict capture exception,
native/sleep-wake/clean-host acceptance, upstream engine licensing/build inputs,
and later binary delivery. No task was removed from the denominator: **23/31
DONE (74.2%)** remains the full Windows-product score. Partial gates receive no
fractional credit. The source-only GitHub milestone stays **100%**.

The [October 4 follow-up](evidence/windows-local-followup-2026-10-04.json) adds
actual running-Memos worker recovery and actual private installer extraction.
E04/R05 remain PARTIAL because native/sleep-wake/installed/clean-host gates are
still open. The capture investigation retained no experimental interface changes.

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
| R03 | DONE | Froze a [100-app planning roster](v1-app-roster.md) with [full acceptance matrix](../catalog/v1-roster.json), explicit membership and upstream snapshots. The refreshed generator validates 53 existing offerings + 47 candidates without changing the original 100-member identity set. It is a future sourcing/replacement pool; V1 targets ten selected apps. Planning placeholders do not supersede the qualified app/provider ledgers or imply proof for the other members. | R02 |

## 1 — Runtime foundation and proof contract

Detailed design: [bundled engine](plans/bundled-engine.md) and
[qualification](plans/qualification.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| E01 | PARTIAL | Exact 138-package development payload and independent signed provenance remain unchanged. The [source/notices companion](evidence/engine-source-review-2026-10-03.md) now retains 89 Ubuntu source identities / 282 signed-index-verified artifacts, nine upstream archives, 120 checksum-verified module archives and 1,423 notice files. Independent reread verifies all 411 source artifacts; eight boundary checks pass. [Delivery instructions](../engine/SOURCE_DELIVERY.md) preserve the local 622,909,440-byte source archive and separate notices. Distribution approval remains false: linked Moby extensions lacks recorded licensing metadata; static docker-init lacks established exact C-library/relink inputs; equivalent public source/notices access and supported clean-host delivery remain unproven. No payload/locks changed. | R02 |
| E02 | PARTIAL | Saved per-app engine bindings route lifecycle, rollback, qualification and recovery without ambient context changes. New installs require the explicitly selected owned WSL engine and fresh journal/token/daemon checks. [Actual explicit selection](evidence/windows-self-engine-selection-2026-10-01.json) and all ten current managed task/resource receipts pass while Docker Desktop is stopped; it is not a prerequisite. Existing app bindings remain immutable. Legacy adoption/coexistence receipts remain historical evidence. Fresh Windows product bootstrap and clean-host acceptance remain. | E01 |
| E03 | PARTIAL | Native fixed-path setup, explicit consent, bounded import/retry/verification/repair, token/journal ownership and prerequisite/disk refusal are implemented. [Actual fresh development payload](evidence/windows-fresh-payload-2026-10-02.json) boots all 138 packages and daemon in 28.84 seconds. The [October 4 unique fixture](evidence/windows-engine-removal-2026-10-04.json) also runs real bootstrap/import/verification before the production removal transaction; test-only name/path translation is explicit. Neither proves literal fixed-name product setup, missing/restricted Windows prerequisites or clean-host support. The [VM plan](plans/windows-clean-machine-test.md) remains preparatory; the owner currently cannot provide another Windows machine. Preserve the existing owned engine rather than replace it for a test. | E02 |
| E04 | PARTIAL | Fixed hidden owned-worker supervision, singleton locks, fresh ownership/selection rechecks and bounded concurrent-start wait are implemented. Actual [CLI exit/95-second idle](evidence/windows-engine-supervision-2026-10-02.json) and [owner-worker stop/crash/three-call restart](evidence/windows-engine-restart-2026-10-03.json) pass with unchanged native state/containers. [Empty product-owned removal](evidence/windows-engine-removal-check.md) adds explicit exact-name consent, installed/retained-data/container/volume refusal, operation/registry/worker locks, repeated ownership checks and preserved uncertain acknowledgments. Seven focused tests and the [actual temporary engine transaction](evidence/windows-engine-removal-2026-10-04.json) pass in 14.915 seconds with unchanged real inventory/identity files. The test-only namespace never targets the production engine. The [October 4 running-Memos proof](evidence/windows-engine-workload-restart-2026-10-04-045856.json) now preserves exact private content and the same running container through worker crash/recovery and ownership-safe cleanup. Native close, Windows sleep/wake and literal fixed-name clean-host removal remain unverified. | E03 |
| Q01 | DONE | Evidence schema 2 fingerprints the exact source locator, adapter and revision; normalized plan; requested and resolved images; OS/architecture; selected engine; Compose version; first-use probe (including actual script source for script probes); and evidence level. Batch scheduling reruns changed inputs, retains current passes and optionally retries current failures. Qualification proof expires after 30 days, source review after 90 days and image observation after 30 days; missing, malformed or implausibly future dates refuse. Older JSON remains readable history without retaining stronger claims. Focused fingerprint and freshness tests and strict Clippy pass. | E02 |
| Q02 | DONE | All ten selected apps now have reviewed per-service memory, CPU and PID ceilings and fresh managed-engine task proofs checking actual container limits after install, restart and keep-data reinstall. Each receipt measures idle/peak memory, owned bind/named-volume storage and immutable image size; uncertain ownership or cleanup fails. [Curated proof ledger](../catalog/v1-qualified-apps.json) validates all ten current manifest/plan/probe hashes. Limits bound the measured launch workloads; they do not promise that large photo libraries, transcoding or workflows fit the same sizing. | Q01 |
| Q03 | DONE | The selected 10 of 53 current offerings (Memos, n8n, Flatnotes, Gitea, Immich, Jellyfin, Kanboard, PrivateBin, Uptime Kuma and WordPress) passed isolated managed-WSL lifecycle/resource runs with recorded engine identity, restart, keep-data reinstall and ownership-safe cleanup. The [curated ledger](../catalog/v1-qualified-apps.json) and `python scripts/check-v1-qualified.py --release-gate` validate all 10; the last two proofs landed in `44c64b6` and `05e2e62`, and bounded Flatnotes was re-proven in `1e9125e`. The other 43 offerings are discovery/replacement candidates; Linkding has separate reviewed candidate task proof. Q02 owns limits for the remaining nine; E04 owns engine resilience. | E02, Q01 |
| Q04 | DONE | All 10 selected apps passed a meaningful exact-state task after install, restart and keep-data reinstall: Memos private memo, Flatnotes note/login, Uptime Kuma HTTP monitor, PrivateBin encrypted paste, Kanboard moved task, n8n workflow result, Gitea private repository/commit, WordPress published post, Jellyfin exact audio playback and Immich exact photo search/download. The [curated ledger](../catalog/v1-qualified-apps.json) fingerprints each task probe and evidence file; the 10/10 release-count validator passes. Kanboard's default admin credential and agent-content access remain separate security/agent gates, not extra app-task proof. | Q03 |
| Q05 | DONE | [Protected backup primitives](../src/backup.rs) quiesce every reviewed service, verify owned bind data/local named volumes and include exact generated credentials. Bounded archives reject unsafe paths/types and are protected with Windows DPAPI; plaintext staging is removed. [Real Memos and Gitea proof](evidence/windows-backup-restore-2026-10-01.json) restores private content into distinct fresh projects, including Gitea's two named volumes and exact credential bytes, then resumes the unchanged source. Occupied targets and shared host folders are refused; nine focused tests pass. This supports the app storage types currently exposed to agent writes, not arbitrary host folders, external effects or automatic app-version rollback. | E04, Q04 |

## 2 — Agentic app store

Detailed design and coverage contract: [agent access](plans/agent-access.md).
This is now V1 scope. The older [agent research](research/agent-platform-and-differentiation.md)
contains options, not authorization to implement every proposed feature.

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| A01 | DONE | `catalog/agent-access.json` has one fail-closed coverage row per 52 approved offerings. The Rust directory consumer rejects missing/duplicate rows, lists only installed apps, describes content method/setup and unavailable login/grant state without inventing access, lists a bounded management tool, and dispatches typed status lookup only for an installed ID. This is an in-process broker contract, not an externally exposed agent gateway; A02/A03 must add identity, grants and transport before any agent calls it. Three focused tests and strict Clippy pass. | R03, Q01 |
| A02 | DONE | Identity, exact client/app/action grants, expiry/revocation, redacted durable audit and one-use owner approval are enforced by the shared gateway. [Owner controls](evidence/windows-agent-controls-2026-10-01.md) provide explicit enrollment and private one-use manual configuration export; no automatic connection or default grant. [Real Memos content proof](evidence/memos-agent-content-2026-10-01.json) exercises broker-held DPAPI credentials, exact approved private writes, cross-client/replay denial, token replacement and revocation. The install/uninstall queue binds approvals to the credential generation, reviewed plan and exact engine. Providers/transport acceptance remain A03–A06; grants are not a sandbox against same-user shell access. | A01 |
| A03 | DONE | Owner-enrolled stdio MCP rechecks credentials, exact grants and durable audit on every call; scoped discovery and start/stop reuse the owned engine and app operation locks. [Real lifecycle proof](evidence/memos-agent-lifecycle-2026-09-30.json) and [approved mutation proof](evidence/memos-agent-mutations-2026-10-01.json) pass actual install, exact private memo, one-use approval/replay refusal, keep-data uninstall/reinstall, running cancellation, revocation and isolated cleanup. Queue contention is bounded and identified as safe to retry only before a mutation can start; audit failure records a terminal failure before dropping the held queue. Imported setup-assisted installs and permanent deletion still require the owner; no arbitrary GitHub/code execution or automatic crash replay. [Contract and limits](evidence/windows-agent-mutations-2026-10-01.md). | A02, E02 |
| A04 | DONE | Four compiled, reviewed access kinds now have actual owned-Windows proofs: [Memos typed API](evidence/memos-agent-content-2026-10-01.json), [official n8n MCP](evidence/n8n-agent-content-2026-10-02.json), [networkless PrivateBin browser](evidence/privatebin-agent-content-2026-10-02.json), and [explicit Flatnotes Markdown files](evidence/flatnotes-agent-files-2026-10-02.json). Browser image/code/profile are pinned; non-root Chromium retains its sandbox with zero capabilities, no network, no host profile/socket/CDP or arbitrary code. A bounded app-only broker supplies reviewed resources. File access rejects credentials, other folders, traversal, links and writes. Protected app connections and separate exact expiring grants never imply permission; reconnection requires fresh grants. Provider-specific supported actions and packaging limits are documented; this is not all-app or native WebView acceptance. | A03 |
| A05 | DONE | Three distinct real app access paths pass reads, writes and refusal/revocation/audit checks: Memos creates an owner-approved private memo; official n8n MCP creates a reviewed fixed-column data table without arbitrary workflow code; isolated PrivateBin DOM creates and reads an approved non-burning one-day paste. Exact text survives restart; cross-client/replay, destructive/foreign-route actions and old grants after replacement/reconnect are refused. Credentials and paste fragments stay protected and absent from responses/audit. [Current source-bound checker](../scripts/check-agent-provider-evidence.py) refuses changed, failed, stale or incorrectly owned receipts. Scope is these useful actions, not complete upstream functionality or every install stage. | A04, Q04 |
| A06 | DONE | All ten selected launch apps have current-source actual content proofs: Memos, n8n and PrivateBin support scoped reads and exact approved writes; Flatnotes, Gitea, Immich, Jellyfin, Kanboard, Uptime Kuma and WordPress provide useful read-only paths. [Coverage matrix](evidence/launch-agent-coverage-2026-10-02.md) declares actions, setup and limits; [strict checker](../scripts/check-agent-provider-evidence.py) passes 10/10. The production directory/capability display validates exact provider/test/fixture hashes and 30-day freshness, withholding changed/failed/stale claims without inventing login or client grants. Eight focused Rust directory/proof/readiness checks and targeted UI checks pass. The other 42 offerings remain unverified for content. | A05, C03 |
| A07 | DONE | V2 Settings integrates explicit agent connections, expiring exact grants, pending install/uninstall and private-content approvals, one-use credential export, revocation and bounded redacted audit. Four owner-control browser checks and three content-control checks pass, including accessibility, consent, expiry/unavailable apps and lost export sessions. Owner unit tests cover rotation, stale cancellation, restart and revocation. Configuration is manual; no client files are edited, no clipboard export or implicit all-app connection occurs. [Scope and evidence](evidence/windows-agent-controls-2026-10-01.md). | A02, F01 |
| A08 | DONE | [Actual content-boundary evidence](evidence/agent-content-boundaries-2026-10-02.md) verifies malicious private WordPress instructions remain data and cannot grant permissions, expose another app credential, select a foreign URL or enable writes; Memos/PrivateBin/n8n enforce exact approvals, typed actions, replay/revocation boundaries and secret-free audit. The proven local backup contract is limited to consistent Memos/Gitea storage restores; external effects and automatic universal undo are excluded. Settings displays these recovery limits and the residual unrestricted same-Windows-user risk. Targeted owner-content UI refusal/accessibility checks pass; this does not claim external-model robustness or a process sandbox. | A05, Q05, A07 |

## 3 — App coverage and GitHub input

Architecture: [backend coverage](backend-app-coverage-plan.md).

| ID | Status | Task and acceptance | Depends on |
| --- | --- | --- | --- |
| C01 | DONE | Regenerated the 606-definition candidate queue and reach ranking offline from checksum-pinned source archives through the current Rust importers, then regenerated the 100-member planning roster without changing membership or promoting a definition. Five previously importable source definitions are now refused by current command-variable rules (moneroblock, siyuan, ghostfolio, Runtipi nocodb and CapRover outline); the top-150 structural/reach set has 89 importable and 61 blocked. Alternate definitions remain separate, and overlapping source counts are not presented as verified unique apps. Queue tests and roster drift check pass. Refresh again whenever importers or pinned sources change. | R03 |
| C02 | DONE | The selected ten reuse existing reviewed recipes/imported definitions and normalized bind/named-volume plans. Narrow per-service resource review rejects duplicate, unreviewed or invalid limits; missing/foreign storage and image identities remain fail-closed. Fresh real task receipts now cover 10/10 selected apps with enforced limits and restart/reinstall/cleanup. No new binary-seed capability is needed by this launch roster. Nextcloud/Calibre-Web remain future candidates, not silently added launch requirements. [Qualified ledger](../catalog/v1-qualified-apps.json). | C01, E02 |
| C03 | DONE | The unchanged launch cohort contains 10 current owned-Windows task/lifecycle/resource proofs, checked by `python scripts/check-v1-qualified.py --release-gate`. [Actual production projection](evidence/windows-launch-collection-2026-10-02.json) reports 7 zero-input installs, 3 setup-assisted installs and 10 current verified app tasks; launcher Settings displays these separately and refuses incomplete/stale proof. App-specific required answers, folders and first-account cautions appear before download. Zero-input installation does not remove app login/onboarding. Capability display and reviewed resource limits pass C02/C05; agent access, native flows and clean-machine delivery retain their separate A06/F04/R04 gates. Other offerings remain previews; replacing this cohort requires a reviewed frozen-roster promotion. | Q03, Q04, C02 |
| C04 | DONE | Strict launcher-only HTTPS GitHub inspection bounds redirects/size/time, verifies canonical identity and pins the default-branch commit; unsafe/unsupported/stale candidates remain noninstallable. [Linkding review](evidence/linkding-candidate-review-2026-10-02.md) completes the new-candidate path: pinned source archive, maintained same upstream immutable image, required private setup, real bookmark task/restart/keep-data reinstall/resource/cleanup proof, separate six-hash named review, explicit registration and [actual live ApprovedMatch](evidence/linkding-github-approved-2026-10-02.json). Readiness validates its separate candidate task ledger; content access stays unverified. All 39 focused Rust, 13 Python and six GitHub UI checks pass. The 100 identities remain frozen (53 offerings + 47 candidates); selected launch ten unchanged. This does not promise arbitrary source builds or turn metadata into installation approval. | A01, C01 |
| C05 | DONE | [Readiness projection](../src/launch_readiness.rs) checks exact current manifest, normalized plan, source/image freshness and task probe before displaying Windows task/lifecycle claims. V2 cards, details and installation review distinguish zero-input/setup-assisted installs, verified tasks and separately proven read/read-write app-content access; required answers/accounts/keys/GPU cautions appear before download. Stale/missing proof becomes an install preview rather than a verification badge. Targeted readiness/browser tests pass; provider acceptance cannot be inferred from a management tool. | Q01, A01, F01 |

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
| F02 | DONE | Launcher-only IPC projects live owned-engine state, saved-app counts, onboarding viewed steps and verified retained setups. Engine setup and recovery retain explicit consent/ownership checks. [Actual recovery parity proof](evidence/windows-launcher-recovery-2026-10-01.json) installs Memos, loses only the fixture registry, adopts retained setup and reads exact private content, then keeps data through uninstall/discard/re-adoption. Bystanders remain unchanged. First-run records explanation views without granting permissions; result UX uses actual saved app facts and explicit Open. Targeted UI tests pass. Native WebView/full fresh-machine flows remain F04/R04. | E02, A02 |
| F03 | PARTIAL | The approved dark V2 now drives routed Settings, factual rail engine state, loading compositions, compact selected rows and detail-header/Manage actions with operation focus preserved. Reading labels meet the frozen floor; first-run/results, pagination, exact-name deletion and agent consent remain truthful. [Local acceptance review](evidence/windows-v2-local-acceptance-2026-10-04.md) traces 86 production states; 144 behavior/state/reading checks pass and five shell baselines were inspected. Freeze passes 135 tokens / three contexts / five fonts / nine screens / 88 states. Default/minimum/scaled route geometry, accessibility and offline assets pass. The [final three-case capture check](evidence/windows-v2-capture-exception-2026-10-04.json) still differs on 102–114 sidebar anti-alias pixels despite identical geometry/styling. The frozen exact-byte capture gate remains unmet; no waiver or all-159-pass claim. Native acceptance is F04. | F01, F02 |
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
| R04 | BLOCKED | A supported clean Windows machine is unavailable, confirmed by the owner October 3. The [VM plan](plans/windows-clean-machine-test.md) explains current Windows Home nesting limits and a 100–120 GiB planning budget. No VM/edition/feature change occurred. On an eventual supported clean host, verify install/first launch, fixed-name engine bootstrap, restricted/failed prerequisites, native app windows, protocol/shortcut, agent access and uninstall with data preservation. Source-only publication is already complete. | F04, C03, A06, A08 |
| R05 | PARTIAL | [Local security/privacy/support and notices review](evidence/windows-security-notices-review-2026-10-03.md) is complete: both missing font OFLs/root MIT are configured; exact Rust notice inventory covers 512 locked packages, zero Windows-target notice-file gaps and five Cargo.lock-verified MPL source archives. Offline notice checks and the 39-file packaging configuration guard pass. [Support guidance](support.md) states tested boundaries. Historical staged resources correctly fail current notice requirements; the [current unsigned private bundle](evidence/windows-private-bundle-2026-10-04.json) now passes launcher/sidecar identity, all 39 exact staged notice files and generated installer mappings. [Actual archive inspection](evidence/windows-private-installer-extraction-2026-10-04-051651.json) now verifies 41 resources / 39 notices and the exact Tauri NSIS marker transformation in the launcher, with an unchanged connector. One NSIS helper listing/extraction size discrepancy is recorded; installed helper behavior is not certified. The installer was not executed, installed or published. R04 clean-host and any eventual published/downloaded Windows artifact acceptance remain outstanding; no installer is public. | R04, Q05 |

## Execution order after an explicit resume

1. G01–G04 are complete. Keep `source-v1.0.0` immutable; its source-publication
   evidence does not certify a Windows installer or complete product flows.
2. Continue E01–E04 for the usable Windows product. Q02–Q04's 10-app
   managed task proof is complete; keep it fresh when definitions or images move.
3. C04 is complete. Finish F03–F04 for reviewed GitHub candidate promotion and the
   complete approved V2 experience. A01–A08 now pass their scoped acceptance;
   keep all ten provider/task proofs current. Run Docker qualifications serially on
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

The former `upgrade-status.md` was retired during October 4 consolidation.
The original Hermes plan and full traceability table remain recoverable from
Git history (`a63cf2b:docs/upgrade-status.md`) and the retained local planning
notes. Their earlier V3/signing/50–100-app requirements are superseded by the
owner decisions above; they are not a second backlog.

| Original task group | Current authority |
| --- | --- |
| 1–6: docs, metadata, tests, modules and identity | R01–R03, F01–F03; source baseline retained |
| 7–15: installed apps, storage, catalog, recipes, transaction and lifecycle | E01–E04, Q01–Q05, C01–C05 |
| 16–20: navigation, activation, shortcuts, commands and CLI | F02/F04, A01–A03, R04; native acceptance remains distinct |
| 21–28: frontend, UX, accessibility, CSP and icons | Approved V2, F01–F04, A02/A08; design approval is not runtime proof |
| 29: CI and release pipeline | R01/R05, published source gates G01–G04 |
| 30: signed updates | S01–S05 deferred to later Windows distribution |
| 31–33: packaged smoke, user docs and release-candidate gate | R04/R05 for the Windows product; source publication is complete |

The managed engine and agent gateway add requirements beyond the original
plan. The frozen 100-member roster is future expansion capacity; V1 requires
ten selected apps. Shelving and cleanup do not waive an unfinished gate.
