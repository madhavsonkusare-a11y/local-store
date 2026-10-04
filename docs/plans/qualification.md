# App qualification contract

Updated October 4, 2026. Project paused. Scope and completion live in [V1_TASKS.md](../V1_TASKS.md), Q01–Q05/C01–C05. This reference replaces the earlier proposed checklist; future scaling detail is in [qualification-at-scale.md](qualification-at-scale.md).

## Current boundary

The source contains 53 offerings, but the Windows launch target is **ten selected apps**. Historical owned-engine receipts cover meaningful tasks, all-service health, resources, restart, keep-data reinstall and cleanup for Memos, Flatnotes, Kanboard, PrivateBin, Uptime Kuma, n8n, Gitea, WordPress, Jellyfin and Immich. See `catalog/v1-qualified-apps.json` and the task ledger. Other offerings remain previews unless their own matching evidence supports a stronger claim.

An actionable page, HTTP 200, image pull or parsed deployment definition is not meaningful-task proof. AI apps may still need a model/API key. Accounts, storage choices, hardware and upstream onboarding are explicit dependencies. Local loopback publication does not make an unauthenticated app safe against other local processes.

## Reuse the existing harness

1. Cheap offline preflight validates immutable image/source identities, normalized plan semantics, supported platforms, required inputs and storage/resource constraints. Reject unsupported semantics rather than dropping them.
2. Real qualification uses `src/qualification.rs` and an explicitly verified owned engine. Give the run its own namespace, app root and bounded resources. On a shared engine, run one qualification at a time.
3. Check every long-running service and required successful one-shot job after install, restart and reinstall. Capture measured startup/memory/storage/image costs and enforce reviewed limits; measurements are not universal hardware minimums.
4. Execute a versioned app-specific task probe with an exact independent state assertion. Reuse task families and API fixtures where possible; login, private data and required credentials remain scoped to the fixture.
5. Restart and reinstall over retained data, then repeat the exact task assertion and verify credential/data continuity. A keep-data reinstall is not an app-version upgrade proof.
6. Remove only owned fixture resources and check bystanders. Cleanup failure is a failed run, not a pass with a warning.
7. Bind receipts to current source, importer/plan, image, engine, resource review and probe identities. Promotion is a separate reviewed manifest/ledger decision, never automatic from a repository URL.

## Freshness and separate claims

`python scripts/check-v1-qualified.py --release-gate` checks the ten-app task gate. Run it after an explicit resume; changed or stale inputs must be requalified. Deleted rootfs, private fixtures and installer files cannot be recovered from their recorded hashes.

Agent-content capability has separate provider/test receipts and expiry, checked with `python scripts/check-agent-provider-evidence.py`. Engine setup, native windows, clean-PC delivery and current installer acceptance remain E01–E04/F04/R04–R05. Passing app tasks does not close those gates.

Report discovery entries, importable definitions, reviewed offerings, zero-input/setup-assisted installations, task proofs, content reads and content writes separately. Seven launch installs historically required no installer answers; three needed setup answers. App login/onboarding may still be needed afterward.

## Future expansion

The frozen 100-member roster is a research/replacement pool, not a requirement to test all 100 for V1. Reuse parameterized fixture adapters and early screening before costly runs. Independent runtime workers require separate verified owned environments and resource budgets before concurrency.

A deterministic model-provider stub could test a future AI-app integration, but would prove only that stub path. Do not market it as a real provider/model result. Cross-platform, long-running workloads, app-version migrations and external side effects require their own evidence.
