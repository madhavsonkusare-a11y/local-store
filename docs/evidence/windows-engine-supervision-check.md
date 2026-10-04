# Windows owned-engine supervision check

The engine uses an exact owned WSL pipe helper and, for the actual Windows
launcher/agent connector, a fixed hidden `local-store.exe engine supervise`
worker. The worker is independent of the launcher lifetime. Neither a Windows
service nor login autostart is installed.

## Boundaries

- Only the native state selected through explicit engine setup may start the
  background worker. Library examples and qualification sandboxes keep their
  process-local session instead. Scoped product-binary qualification explicitly
  opts into process-only mode to avoid leaked helpers.
- Ownership is verified before acquisition and live-session reuse. The worker
  retains its original validated journal, rejects replacement identities, and
  checks the saved selection repeatedly. Missing/corrupt state stops the worker.
- A singleton file lock establishes liveness. Status is bounded to 2 KiB,
  schema checked and accepted only alongside the lock and recent verification.
  PID values are diagnostics, never authority to kill a process.
- Launcher startup is serialized. It waits for its exact child to acknowledge
  ownership. Failed Windows job breakaway, early exit and a 35-second startup
  deadline are explicit failures; no lifetime guarantee is fabricated.
- Concurrent callers now wait up to 35 seconds for the same owned worker,
  checking selection again before accepting fresh status or acquiring the lock.
  The October 3 actual restart check exposed the former transient refusal and
  passes after this bounded wait was added.
- Stop/revocation releases only the worker's exact WSL stdin helper. No daemon,
  container, other distribution or global Docker context is altered.

## October 3 local restart and crash check

`python scripts/check-engine-restart.py --run` uses a private profile containing
only copied verified ownership/selection. The actual launcher explicitly stops
its fixture worker. A worker created through the proof's own process handle is
then acknowledged and crashed; three simultaneous launcher diagnostics recover
one fresh singleton. Only the proof-created child handle is terminated; external
PIDs from diagnostic files are never used as termination authority. Final
fixture-selection revocation stops the new worker. Native ownership files and
all existing container identities/start times remain unchanged.

[Actual receipt](windows-engine-restart-2026-10-03.json) records the launcher,
proof script, subprocess helper and supervisor-source SHA-256 values. The host
had zero existing containers, so this does not add a running-app workload proof.
It supplements the earlier 95-second idle/CLI-exit proof. Native WebView close,
Windows sleep/wake and literal fixed-name clean-host bootstrap/removal remain
separate acceptance. No host sleep, reboot, feature change or production-engine
unregistration occurred.

## October 4 running-app workload

`python scripts/check-engine-restart.py --run --with-memos` adds one isolated
product-CLI installation of the existing pinned Memos recipe. It refuses an
occupied Compose project or exact container name, then creates and rereads a
private memo using the unchanged first-use probe. After worker stop/crash and
three concurrent launcher restarts, the exact memo, running container ID and
start time are unchanged. Normal ownership-safe product uninstall deletes the
fixture's data; the original container inventory and native identity files
match. Selection revocation stops the fixture worker, including when app cleanup
refuses; uncertain cleanup retains its files. No credentials/content are published.

The [final actual receipt](windows-engine-workload-restart-2026-10-04-045856.json)
fingerprints the current proof, launcher, recipe, task probe and supervisor.
The earlier October 4 receipt is history from before stronger name/failure-cleanup
guards. This adds a running-app workload proof, but does not prove native close,
Windows sleep/wake, a daemon crash, host reboot or literal fresh-host removal.

## Verification status

The focused owned-WSL run passed 30 tests, including the five supervisor/lease
checks. These exercise real file-lock contention and status liveness, bounded
status parsing, stale/future status refusal, missing-state refusal, fresh
ownership before lease replacement and rejection of immediately exited helpers.
The fixed CLI grammar refuses caller-supplied supervisor paths or extra options.

The [real launcher-exit and long-idle proof](windows-engine-supervision-2026-10-02.json) passes: the same worker survives a 95-second interval without launcher/WSL requests, fixture revocation stops it and native state is unchanged. The first failed attempt exposed an inherited output pipe; the fixed Windows worker uses `CreateProcessW` with zero inherited handles and never attaches to the launcher console. The proof harness also returns from a descendant-pipe timeout in 1.02 seconds so exact fixture recovery can execute. No native close or app task is inferred from this proof. Windows sleep/wake,
unplanned machine restart, full owned-engine removal and fresh-host support
remain E04 acceptance work. Process-local lease tests and passing app lifecycle
proofs do not establish these guarantees.
