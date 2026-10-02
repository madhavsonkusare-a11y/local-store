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
- Stop/revocation releases only the worker's exact WSL stdin helper. No daemon,
  container, other distribution or global Docker context is altered.

## Verification status

The focused owned-WSL run passed 30 tests, including the five supervisor/lease
checks. These exercise real file-lock contention and status liveness, bounded
status parsing, stale/future status refusal, missing-state refusal, fresh
ownership before lease replacement and rejection of immediately exited helpers.
The fixed CLI grammar refuses caller-supplied supervisor paths or extra options.

The real launcher-exit and long-idle proof is pending. Windows sleep/wake,
unplanned machine restart, full owned-engine removal and fresh-host support
remain E04 acceptance work. Process-local lease tests and passing app lifecycle
proofs do not establish these guarantees.
