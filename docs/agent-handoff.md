# Agent handoff

Updated October 4, 2026. **Project shelved at the owner's request.** Do not start agents, reinstall dependencies, rebuild the engine or run qualifications unless the owner resumes development.

## Preserved checkout and GitHub

- Local checkout: `D:\06 Projects\local-store` (renamed from `dockwrap`). Quote paths containing spaces.
- Remote: [madhavsonkusare-a11y/local-store](https://github.com/madhavsonkusare-a11y/local-store), branch `main`.
- Published source tag: `source-v1.0.0` at `a53e370`; keep it immutable.
- [V1_TASKS.md](V1_TASKS.md) is the sole task ledger. Source milestone: **100%**. Historical Windows product: **23/31 DONE (74.2%)**, with eight acceptance gates still open. Deferred signing/update tasks do not count as completed.
- Approved interface: V2. Launch target: ten selected apps. The 100-member roster is a future planning pool, not the launch requirement. Docker Desktop is not a product prerequisite; the owned engine uses Docker/Compose internally.

## Cleanup and source preservation

The Windows app, current/legacy app and WebView profiles, owned `local-store-engine-v1` WSL distribution and disk, installers, `target`, `.cache`, `dist`, `gen`, `node_modules`, sidecar binaries, test output, external Local Store build cache, temporary fixtures, previews and Hermes scratch scripts were removed. About 80 GB of host free space was recovered in the first cleanup.

Two clean extra worktrees were removed; their commits survive locally in `refs/archive/shelved-2026-10-04/ls-pr3` and `refs/archive/shelved-2026-10-04/dazzling-wing-1e5ae9`. Tracked source, Docker build definitions, notices, approved assets, planning notes and evidence remain. The owner's CHANGELOG edits and pnpm lockfile were committed before this consolidation.

Docker Desktop was inspected after the owner started it. Seven unused images matched Local Store's engine Dockerfile history; those and 16 attributable build-cache records were removed. Penpot's seven containers, two volumes, network and in-use images were preserved. Three generic local-source cache records (about 90 MB) have insufficient ownership attribution and were retained. See the [cleanup receipt](evidence/project-shelf-docker-cleanup-2026-10-04.json). Docker disk compaction and unrelated tool/cache cleanup were not performed.

## Accepted work and remaining boundaries

The source has 53 offerings and 1,678 discovery entries. Ten selected apps have historical managed-engine task/lifecycle/resource proofs and bounded agent-content access. Useful access does not mean every feature or unrestricted writes. Permission/revocation/untrusted-content and protected backup proofs exist; current binaries and receipt freshness must be checked again after rebuilding.

The eight remaining rows are **E01, E02, E03, E04, F03, F04, R04 and R05**. Detailed acceptance stays in the master ledger:

- Engine package provenance, source mapping/license delivery and bootstrap/consent acceptance remain incomplete.
- E04 includes real running-Memos worker crash/recovery, but native close/sleep-wake acceptance remains open.
- F03's reviewed UI behavior and geometry pass within recorded scopes. Exact capture comparison still fails on sidebar anti-alias pixels; no tolerance waiver was approved.
- Full native WebView flows and a supported clean Windows host remain unavailable/unproven.
- R05 inspected actual private installer resources/notices and Tauri's NSIS marker transformation. That installer was not a certified clean-host product, and its file has now been deleted.

Useful final checkpoints: [local completion](evidence/windows-local-checkpoint-2026-10-04.json), [follow-up recovery/package inspection](evidence/windows-local-followup-2026-10-04.json), [V2 local acceptance](evidence/windows-v2-local-acceptance-2026-10-04.md), [engine source review](evidence/engine-source-review-2026-10-03.md), [launch agent coverage](evidence/launch-agent-coverage-2026-10-02.md).

## Resume only when requested

1. Read the master ledger, this handoff and the relevant architecture/design references. Check the current Git branch, owner edits and any other agent's activity before changing source.
2. Restore dependencies only as needed. Use the repository-local Rust target directory; do not set a global `CARGO_TARGET_DIR`. Keep shared Docker/Penpot and unrelated projects intact.
3. Rebuild payloads through the existing scripts and verify provenance/ownership before engine bootstrap. Deleted ignored artifacts cannot be recovered from proof receipts. The [engine reference](../engine/README.md) and [clean-host plan](plans/windows-clean-machine-test.md) explain prerequisites and limits.
4. Choose one bounded remaining acceptance gate. Use focused checks during development; reserve long integration/build runs for a concrete gate. Runtime proof uses the owned engine, and qualifications on a shared engine run serially.
5. If delegating, assign independent files to agents; reserve Cargo files, command integration, ledger and handoff for one integrator. Independent workers need separate owned environments before parallel runtime proof.
6. Record exact checks, failures and limitations. Historical evidence does not certify changed images/providers/binaries. Never count a partial or deferred row as DONE.

## Documentation consolidation

The former long handoff, stale parallel allocation, old 33-task status page and deleted-installer manual guide were consolidated into this file, the master ledger, architecture/qualification references and support guidance. Their original text is recoverable with `git show a63cf2b:docs/<path>`. No historical evidence JSON or approved design asset was discarded.
