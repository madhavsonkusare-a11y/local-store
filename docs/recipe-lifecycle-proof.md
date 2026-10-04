# Real recipe lifecycle verification

> Historical record, indexed October 4, 2026. This preserves its original test/selection scope. Current ten-app launch acceptance belongs to [V1_TASKS.md](V1_TASKS.md); local runtime artifacts were removed during shelving.

## September 7, 2026 result

All three recipes passed all 13 recorded checkpoints on Windows with Docker
Desktop 4.68.0 / Linux-amd64 engine 29.3.1:

| Recipe | Version | Result |
| --- | --- | --- |
| Memos | 0.30.0 | Passed, including bind-mounted data preservation/deletion |
| n8n | 2.37.10 | Passed, including named-volume preservation/deletion |
| Uptime Kuma | 2.5.3 | Passed, including bind-mounted data preservation/deletion |

[Machine-readable evidence](evidence/recipe-lifecycle-windows-2026-09-07.json)
records the run from 16:57:46 to 17:04:20 UTC. No test-owned containers, networks,
volumes or app directories remain. Image layers remain cached. No release
installer was tested by this run.

## Reproduce

Run the opt-in harness with a freshly built Local Store CLI and a running Docker
engine:

```text
cargo build --locked
python scripts/smoke-recipes.py --binary target/debug/local-store.exe
```

On Linux/macOS omit `.exe`. Add `--recipe memos`, `--recipe n8n`, or
`--recipe uptime-kuma` to run one app. This is not part of offline CI: it pulls
images, starts services and explicitly deletes its own test data.

The harness refuses existing recipe container names, Compose project resources,
reserved volume/network names, and occupied published ports. It creates a unique
configuration root below `.cache/recipe-smoke-*`, passed through the platform
configuration environment variables. Do not run two copies for the same recipe,
or install that recipe in the launcher while the harness is running.

Each recipe goes through the actual CLI install and registry commit, an HTTP
health check, container-side persistence marker creation, logs, stop/status,
start/health, keep-data uninstall, reinstall/health, marker verification, and
explicit deletion. The final assertions check registry removal, managed directory
removal and absence of the project's containers, networks and volumes.

Every run writes `report.json` with the binary hash, engine details, exact image
reference and local image ID, passed checkpoints and any failure. Failed runs
retain evidence and any remaining resources for diagnosis; there is no global
Docker prune. Review the private Compose path before any manual cleanup.

This establishes runtime lifecycle behavior and persistence of a file in the
app's mounted storage. It does **not** establish application-level document or
workflow migration, an upgrade between two image versions, clean installer
behavior, or support on an untested host platform. Current offering status comes from the reviewed manifests; this dated report
does not certify a different engine or V1 release candidate.

The first Windows run on September 7 exposed a localhost health defect: only
the first resolved address was attempted, so IPv6-first localhost could not
reach the recipes' IPv4-only published ports. Memos started successfully but
installation timed out and rolled back. The probe now tries up to four resolved
addresses, with bounded connection/read/write timeouts. A real IPv4 listener
regression test covers this route in `tests/health_loopback.rs`.
