# Backend app coverage architecture

Updated October 4, 2026. Development is paused. Current execution status is in [V1_TASKS.md](V1_TASKS.md). This document
replaces the stale B1–B9/V1–V6 queues with the architecture they established.
V1 targets 10 managed-engine-verified apps in native windows, with agent
access. The frozen 100-app roster is retained as a research and replacement
pool; roster membership does not make an app a V1 release candidate.

## Reuse the existing pipeline

Pinned discovery inputs and supported deployment definitions feed the existing
CapRover/Runtipi adapters, `DeploymentPlan`/`PlanTemplate`, setup review,
qualification, reviewed manifests, `offerings` lookup, and runtime transaction.
Keep a single installer and operation/registry locking scheme.

| Existing capability | Code / evidence | Accepted scope / remaining boundary |
| --- | --- | --- |
| Inventory, deduplication and ranking | `catalog/candidate-queue.json`, `catalog/candidate-ranking.json`, generator scripts | Refresh after capability changes; unresolved identities and alternate definitions must remain explicit |
| Deployment adapters | `src/importers/caprover.rs`, `src/importers/runtipi.rs` | Selected launch definitions are reviewed; unsupported fields remain refused; wider candidates need separate review |
| Normalized plans | `src/plan.rs`, `src/setup.rs` | Owned-engine selection and typed access exist; fresh native bootstrap remains incomplete |
| Setup, secrets and storage | Typed fields, persisted generated secrets, managed and consented shared folders, named volumes, text seeds | Scoped protected backup/restore and credential brokerage have proof; shared folders and universal recovery are not promised |
| Service ordering and addressing | Health dependencies, successful one-shot jobs, loopback auxiliary ports, internal networks | All-service proof exists for the selected ten; callbacks/TLS/auth remain app-specific |
| Immutable installs | Reviewed image/index digests and CI metadata | Evidence freshness and supported app upgrades |
| Qualification and promotion | `src/qualification.rs`, `src/templates`, `docs/evidence` | Selected ten have historical managed task/provider proofs; changed inputs and wider apps require new qualification |
| Transaction and recovery | `src/runtime`, `src/recovery.rs`, CLI recover/adopt | Launcher recovery has scoped proof; native V2/full clean-host acceptance remains open |

## Coverage rules

1. Rank canonical apps by reach and diversity, then choose the best supported
   definition for each. Do not add overlapping source totals or count variants
   as progress toward the ten-app launch cohort.
2. Screen image freshness, supported platforms, license/setup requirements and
   upstream deployment before a costly run. Registry metadata is not a security
   review and a recent rebuild is not proof of a current app version.
3. Import the complete supported semantics or return a reason. Do not drop
   authentication, health checks, mounts, startup commands or proxy behavior to
   make a definition parse. Prefer a reviewed first-party definition when the
   upstream package is genuinely unusable.
4. Build with immutable identities. A source/adapter/plan/image/engine/probe
   change cannot silently inherit stronger proof.
5. Promotion is a reviewed decision with matching evidence. Report zero-input,
   setup-assisted, lifecycle-tested, meaningful-task and agent-access counts
   separately. No HTTP 200 or app listing proves the entire user workflow.
6. Preserve per-app database isolation by default. Shared services, GPU/device
   access, host privileges and executable source hooks need explicit designs;
   they are not cheap importability toggles.

## GitHub URLs

Match a known approved app first. Otherwise resolve a public repository/ref to
a commit, inspect bounded supported files, and return a candidate with exact
provenance and limitations. Do not execute README commands or call an arbitrary
repository verified. Source-only builds require a separately reviewed existing
build provider, isolated credentials/network/storage and new proof. Private
repository credentials are a later contract.

## Umbrel and other sources

The [pinned Umbrel study](research/umbrel-implementation-reference.md) and
[import source survey](research/import-sources-study.md) explain which patterns
can be reused: versioned app repositories, lifecycle metadata, gateway routes,
first-use credentials, dependencies and managed storage. Linux platform hooks
cannot be copied as Windows deployment behavior, and source/asset reuse needs
its own license review. Prefer licensed definitions and documented app APIs.

## Historical implementation map

The former B1/B2/B3 inventory, first imported install and shared harness exist.
B4 capability support and B5 onboarding/routing remain app-specific. B7
reviewed GitHub candidate promotion is accepted in C04; arbitrary source builds
and automatic app updates remain outside the current launch commitment. New
deployment sources are not completed by growing discovery data.
Use C01–C05, Q01–Q05 and E01–E04 in the master ledger for the actual next work;
agent access is A01–A08. No parallel checklist lives here.
