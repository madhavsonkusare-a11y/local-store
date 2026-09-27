# Parallel V1 execution

The release ledger is [V1_TASKS.md](../V1_TASKS.md). This is a work allocation plan, not a second source of completion status. As of September 27, 2026, 6 of 36 rows are DONE. A literal 50% task milestone requires 18 DONE; partial implementation, mocked API tests, and metadata preflight do not increment it.

## Independent lanes

| Lane | Owns | Acceptance output | Dependency / shared resource |
| --- | --- | --- | --- |
| Engine | E01–E04, then Q02 | Reproducible signed-source payload, clean Windows proof, owned WSL recovery, resource limits | One Windows/WSL host at a time; Docker Desktop must be available before real proof |
| App qualification | Q03–Q04, C03 | Reusable task probes and reviewed, current managed-engine evidence per app | Sole owner of the qualification engine; no parallel runs on today's shared distro |
| Agent access | A02–A05, later A06/A08 | Durable policy, authenticated MCP gateway, reviewed providers, denial and revocation proof | Keep grants and identity server-side; real provider proof needs installed apps |
| GitHub/catalog | C01–C02/C04 and provenance review | Bounded, pinned source inspection, measured importer coverage, preview contract | Offline work independent of WSL; no automatic approval of a repository URL |
| Product contracts | F02, C05, A07 | Typed backend projections and state contracts | V3 implementation waits for owner handoff (F01) |
| Delivery | S01–S05, R04–R05 | Owner-controlled signing/update secrets, signed artifacts, clean release run | S01/S03 need owner/provider inputs; never share signing material with app qualification |

Assign file ownership per batch and give each agent a narrow acceptance test. Agents can develop probes, policy, source inspection and UI contracts concurrently; one integrator reviews conflicts and updates the sole ledger after evidence lands. Treat Cargo.toml/Cargo.lock, central commands, docs/V1_TASKS.md and docs/agent-handoff.md as integration-owned files to avoid simultaneous edits. Run one Rust compile/test batch after code lanes settle instead of contending for the target directory.

## Development check cadence

During implementation, run the smallest focused test for the touched boundary and
one compile check when a CLI or IPC surface changes. Do not wait for a full
Windows release build after every main commit. The full build workflow remains
available for PRs, version tags and manual dispatch; run it at integration
milestones and before release. Keep runtime and app proof gates unchanged: a
mocked test is never a managed-engine qualification.

## Throughput strategy

1. Group release apps by task shape and reuse a small number of version-pinned adapters. Generate candidate probes from catalog metadata, but require an independent exact-state assertion after restart and keep-data reinstall. A browser or Jev may discover steps; it cannot certify its own success.
2. Separate cheap preflight from costly installation: source/image/license/platform review, plan rendering, ownership checks, then one isolated managed-engine qualification. Fail early without calling a preflight result verified.
3. Make qualification workers independent before parallelizing real apps: one owned WSL distro, data root, port range, project namespace, engine token, evidence output and resource budget per worker. Test worker creation/teardown before running concurrently. The current single owned distro remains serial.
4. Prioritize tasks that unblock other rows (E01–E04, A02/A03, Q04/Q05) and app families that prove a reusable adapter. Prefer 50 fully evidenced release apps over 1000 catalog listings with no first-use proof.
5. Keep evidence machine-checkable and current. A second reviewer or agent may inspect logs and hashes, but the release gate decides from immutable evidence and the curated ledger, not the qualifying agent's narrative.

## Next milestone

The first independent slices landed: durable A02 state, bounded C04 metadata inspection and a shared content probe. The combined Rust library suite, strict Clippy, formatting and local content tests passed; no partial task was promoted. When Docker Desktop is running from the Start menu after its disk move, run Flatnotes in isolation as the second task-qualified app, investigate any probe mismatch, and expand only from a passing adapter. Next complete authenticated policy use and the MCP transport, while engine packaging/proofs and V3 backend projections proceed in separate file lanes. The 18-DONE milestone remains contingent on engine proofs, an approved V3 handoff and owner-controlled signing/update credentials; it cannot be reached by relabeling partial work.
