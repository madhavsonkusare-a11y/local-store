# Command errors and operation events

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

All launcher commands return their existing success payload or reject with
`{ "code": "...", "message": "..." }`. The frontend IPC adapter exposes this
as `CommandError`. Messages are readable, redacted diagnostics; use codes for
branching, never message matching. No error implies that retrying is safe.

| Codes | Failure boundary |
| --- | --- |
| `invalid_input`, `forbidden` | Input validation or non-launcher caller |
| `not_found`, `already_exists` | Missing app/recipe or duplicate registry entry |
| `operation_busy`, `port_in_use` | Per-app cross-process lock or install port preflight |
| `prerequisite_unavailable` | Install requires a ready Docker and Compose |
| `process_unavailable`, `process_failed` | Missing process, spawn/wait failure, nonzero exit |
| `rollback_failed` | Setup/registry commit failed and cleanup also failed; inspect files and containers before retrying |
| `timed_out`, `cancelled` | Process deadline/cancellation or readiness wait |
| `storage_io`, `storage_corrupt`, `migration_refused` | Filesystem, registry parsing/version, migration refusal |
| `unsupported_operation`, `unsafe_path` | Connected-app lifecycle or unsafe managed-data path |
| `browser_open_failed`, `window_open_failed`, `shortcut_failed` | Native integration failures |
| `internal` | Task join, clock or poisoned command mutex failure |

`list_apps` retains `status: "error"` and includes optional `status_error` when
status probing fails. Doctor keeps its optional per-check `error` object.
Successful app logs remain raw output, not diagnostic-redacted text.

CLI lifecycle failures still print readable stderr and exit 1. Invalid CLI
arguments exit 2. Doctor uses 0/1 readiness and 2 usage codes. Named codes do
not introduce new shell exit values in this compatibility batch.

## Lifecycle event transport

Subscribe with `listenOperations(callback)` from `src/js/api.js` before invoking
install, start, stop, uninstall or native open. Keep and call the returned
unlisten function when the consumer is disposed. No desktop event adapter
returns a no-op unsubscribe; malformed event envelopes are ignored.

The `local-store://operation` channel is targeted exclusively to the launcher
webview, with the same launcher guard as commands. Each accepted invocation
emits `started`, followed by `succeeded`, `failed`, or `cancelled` after its result.
Install operations also emit `progress` with an optional `stage` field:
`checking_system`, `preparing_files`, `validating_recipe`, `starting_containers`,
`waiting_for_health`, `saving_app`, or `rolling_back`. Each stage announces entry
into that work, not its completion. Progress retains the same operation ID.

```json
{
  "operation_id": "1234-1",
  "app_id": "memos",
  "kind": "start",
  "state": "failed",
  "error": { "code": "timed_out", "message": "docker timed out" }
}
```

Operation IDs are unique within the running process. `error` is omitted on
start/success. Notification delivery is best effort: IPC completion remains
authoritative, and a failed notification never reverses a successful mutation.
Process termination can prevent a terminal event; refresh app status on reload.
These are operation and install-stage boundaries, never percentages. There is
no replay queue and no automatic retry. Install events carry `cancel_id`, the
runtime operation a cancel request must name; it is absent on operations that
cannot be cancelled.
Connect/remove/shortcut remain short request-response commands without events.

## Consumer checkpoint (tasks 24/25)

The launcher now consumes this transport for per-app busy state and persistent
diagnostics; IPC completion is authoritative. Readiness/health state remains
separate work. Install-stage events are now connected to the review dialog.
Preserve existing inputs and keyboard focus on failure, and avoid retrying
partial installs blindly. Native remote-window isolation still needs the release
security gate.

## Cancellation

`cancel_app_setup { id, operationId } -> bool` asks a running install to stop.
It requires the launcher window, and returns whether a matching operation was
still running — `false` is a normal outcome, not an error, and means the
operation had already finished. IPC completion of the install itself remains
authoritative: a cancel request never settles the install, it only asks.

The launcher shows a **Stop setup** control inside the install progress notice
while `cancel_id` is known and the reported stage is before `saving_app`. It
disappears at the commit cutoff and on any terminal event. If focus is on the
control when it disappears, focus moves to the live progress region — the
confirm button is disabled mid-install and cannot hold focus, and the dialog's
close controls would risk dismissing the dialog on a keypress.

- **One token spans the transaction.** `begin_install` takes the token from the
  app's operation lock and threads it through the preflight checkpoints, Compose
  `config` and `up`, and the health wait.
- **Cancellation is honoured only at checkpoints:** before the Docker probes,
  before the first file is written, before Compose validation, before container
  start, and between health polls. It never interrupts a Compose command
  mid-flight — the runner's own deadline covers a hung child.
- **The commit is the cutoff.** `PendingInstall::commit` does not consult the
  token. Past that point the containers are running, and honouring a late cancel
  would leave them up with no registry entry: invisible to the user and
  unmanageable from the app.
- **Rollback always runs on a fresh token.** Reusing the cancelled one would
  abort the cleanup immediately. Data that predates the install is preserved,
  and a cleanup failure is reported as `rollback_failed` carrying both causes.
- **A cancel names the operation it means to stop.** `cancel_operation(app_id,
  operation_id)` cancels only when the running operation's id matches, so a
  request that arrives late cannot stop the operation that replaced its target.
  It returns whether anything was cancelled.
- **The lock spans the commit.** `PendingInstall` holds the operation lock until
  the registry write lands or rolls back, so nothing can act on the app in that
  window. The lock also holds an fs4 sidecar file lock across processes sharing
  the same configuration root. Uninstall holds the same lock through registry
  removal. Busy requests fail promptly; cancellation still belongs to the process
  that started the operation.

## Readiness

`app_readiness { id } -> "ready" | "unreachable" | "unknown"` reports whether an
app answers on its address, as distinct from whether its containers are up.

It is a separate command from `list_apps` on purpose: probing inside the listing
would make the whole list as slow as the least reachable app. The launcher asks
per app once the rows are on screen and fills them in as answers arrive, in
parallel.

`unknown` is returned rather than guessed. The probe speaks plain HTTP only, so
an `https://` address is never called unreachable on the strength of a check
that was not performed. The launcher treats `unknown`, an error and a missing
answer identically: the row keeps its container status. Only `ready` and
`unreachable` change what a row says, and readiness never overrides the busy
label of an operation in flight.

My Apps repeats readiness checks 15 seconds after a round completes, with at
most four probes active. Only displayed eligible rows are checked. Leaving My
Apps, hiding the launcher or starting a row operation pauses the relevant
checks; replies for removed, changed or paused apps are ignored. The monitor
does not poll Docker container status or replace row elements, so background
answers preserve keyboard focus. Probe errors fall back to the existing status.

## Address checks

`check_address { url } -> "ready" | "unreachable" | "unknown"` reports whether an
address answers, before it is saved as a connection. It validates the URL first
and fails with `invalid_input` for anything `validated_external_url` rejects.

It is **advisory and never blocks saving**. An app the user has simply not
started yet is a perfectly good connection to save, so `unreachable` is phrased
as information, not as a rejection. `unknown` carries the same meaning as it does
for readiness: the probe speaks plain HTTP only, so an `https://` address is not
called unreachable on the strength of a check that never ran.

## Install setup options

`recipe_details { id }` retains its existing recipe fields and now includes
`setup_review`. This explicit projection contains field labels, keys, control
types, required/sensitive flags, non-sensitive defaults, and service/generated
credential counts. It never serializes template environment values, generated
credential keys/values, or raw regex rules. Sensitive options/defaults are
omitted. Missing or blank answers use existing backend defaults; forms must
explain this for masked fields. Backend validation remains authoritative.
The current three reviewed recipes return no setup fields. This adds no imported
install permission and no arbitrary-template IPC. The existing recipe response
still includes its reviewed static Compose text; it is not a resolved template.

`install_app { recipeId, hostPort? }` accepts an optional published port.
Omitted, the recipe's pinned port is used and the call is exactly what it was
before this option existed.

A chosen port **republishes** the pinned recipe rather than mutating it:
`Recipe::with_host_port` rewrites only the host side of the Compose mapping and
carries the launch address, health address and risk notes with it, then runs the
full manifest validation on the result. A rewrite that left any address behind
is refused before Docker is invoked. The container port never moves — it belongs
to the image. Ports below 1024 are refused, and the launcher refuses anything
outside 1024-65535 before sending the command.
# Plan installation transaction

The Rust runtime exposes `begin_template_install(template, display_name,
answers, lock, progress)` and `install_template(template, display_name, answers)`.
These are backend APIs, not new IPC commands. A pending plan uses the same
`PendingInstall` as a recipe: its matching per-app lock survives resolution,
container startup, registry commit and rollback. Both entry points reject a lock
whose app ID differs from the install target. Failed commit removes newly
created containers but preserves data and credentials that predate the attempt.
The UI must select a reviewed template and acquire its lock before starting
background work; it must not accept an arbitrary caller-supplied Compose plan.
