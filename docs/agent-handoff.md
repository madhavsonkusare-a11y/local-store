# Agent handoff

Updated September 14, 2026. Start with [V1_TASKS.md](V1_TASKS.md); it is the
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

V1 requires 100 distinct tested/verified offerings (existing target), a bundled
engine, owner-led V3 frontend, signed Windows delivery, an automatic updater,
and agents managing the store and accessing every offered app. The new signing
requirement supersedes its earlier deferral. V2 is reference material; do not
start its production integration while V3 is being designed.

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
- The previous main CI failure was a stale mandatory `00_Design_Notes.md` path
  in `tests/brand_strings.rs`; the current docs remain recursively scanned.
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

Complete Q01 identity/freshness and Q02 RAM/disk/startup measurements before Q03's
52-app requalification. Do not call Docker Desktop fixtures managed-engine proof.
V3 and owner/provider signing credentials remain external inputs. No signing key,
account, spending or release publication was created by this batch.

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
