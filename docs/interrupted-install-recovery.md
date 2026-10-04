# Recover an interrupted installation

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

An interrupted install may leave an owned Docker project and retained files
without a saved app. An occupied port alone does not establish ownership.

## Inspect

```text
local-store recovery --json
local-store recovery --json --docker
```

The first command reads registry/files only; the second adds bounded Docker
label checks. Candidates can include an intentional keep-data uninstall.
Ownership is `not_checked`, `no_containers`, `mismatch` or `verified`.
A snapshot does not authorize deletion, and a corrupt registry fails inspection.

## Resume or clear

```text
local-store adopt <app-id>
local-store recover <app-id>
local-store recover <app-id> --delete-data
```

Adopt re-derives a supported retained setup under the app's operation lock,
verifies ownership, starts it, and registers it only after readiness. Recover
re-checks current registry/files/Docker ownership under the lock and clears only
the owned project. Data is kept unless `--delete-data` is explicitly selected.
Installed apps, busy operations, mismatched ownership and incomplete cleanup
are refused rather than guessed away.

The launcher currently exposes inspection; action wiring belongs to V1 task
F02. Do not replace these guarded CLI operations with manual global Docker
cleanup. If ownership is refused, inspect the exact retained project and error.

## Evidence and limitations

`tests/install_interruption.rs`, `tests/install_interruption_stages.rs`,
`tests/recovery_adopt.rs` and `tests/recovery_discard.rs` cover healthy pre-commit,
pull/startup interruption, ownership checks and adoption/refusal behavior.
Dated real evidence remains in `evidence/recovery-action-memos-windows-2026-09-08.json`.

```powershell
$env:LOCAL_STORE_RUN_DOCKER_TEST = '1'
cargo test --locked --test install_interruption -- --ignored --nocapture
```

Use one isolated qualification at a time and inspect retained test resources on
failure. These tests do not prove arbitrary database repair, all daemon-crash
modes or the future bundled-engine implementation. That work is in
[V1_TASKS.md](V1_TASKS.md).
