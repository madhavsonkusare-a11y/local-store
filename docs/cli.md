# Read-only CLI commands

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

The catalog and Doctor commands can be used by people, scripts and coding
agents without reading the app registry. Catalog search is entirely offline.
Doctor queries Docker and Compose inside the selected Local Store engine. It
does not install apps or directly start containers. A query can wake the owned
WSL distribution and its existing systemd services.

## Docker diagnostics

```sh
local-store doctor
local-store doctor --json
local-store doctor --help
```

JSON output uses the same `DoctorReport` as the launcher: `ready` and a `checks`
array. Each check contains `id`, `label`, `ok` and `detail`. The IDs are `docker`
and `compose`. Selection and ownership are checked first: missing, corrupt or
unresponsive owned-engine state returns a failed engine check without falling
back to Docker Desktop. Once selected, both component checks are reported. Strings
are JSON escaped; stdout contains one JSON document and a trailing newline,
including when prerequisites are unavailable. Plain-text output remains the
default.

Exit status is **0** when ready, **1** when a prerequisite fails, and **2** for
invalid arguments. Usage errors go to stderr and leave stdout empty. Help exits
successfully without invoking Docker. Duplicate and unknown options are errors.

Doctor's two probes each run under a 30-second deadline and capture at most
256 KiB per stream. A Docker command that hangs is terminated together with the
processes it started, and the check is reported as failed rather than leaving
the CLI waiting.

Failure details are redacted before they are reported, so a Doctor report can
be pasted into a bug report. URL credentials become `scheme://***@host`, the
value of a key such as `PASSWORD`, `TOKEN`, `SECRET` or `auth` becomes `***`,
and an `Authorization` header keeps only its scheme name. Redaction runs before
the length cap, and a partial final line is dropped when the capture bound was
reached, so a credential cannot survive as a readable prefix. `logs` output is
deliberately not redacted: it is the app's own log, which `docker compose logs`
would show verbatim anyway.

## Explicit engine setup

```sh
local-store engine setup-preview
local-store engine status
local-store engine repair
local-store engine use-self-engine --consent
local-store engine supervisor-status
local-store engine stop-supervisor
```

Use Settings for normal setup and its explicit consent choices. The preview
checks the fixed sibling `engine/rootfs.tar` against the build's pinned length
and SHA-256; it does not import it. Missing or different bytes cannot be used.

### Removing an empty owned engine

```sh
local-store engine removal-preview
local-store engine remove-empty --consent --confirm local-store-engine-v1
```

This owner-only action deletes the empty engine's virtual disk and cached images.
It refuses saved managed apps, retained app folders, any container or volume,
uncertain ownership, a running supervisor and adopted development disks outside
the native product directory. A preview grants no permission; execution repeats
the checks under locks and requires both explicit consent and the exact name.
External connections and their servers are preserved. Stop only Local Store's
background supervisor first; this does not stop app containers or global WSL.
There is no force option or arbitrary distro/path argument. Read the
[removal contract](evidence/windows-engine-removal-check.md) before using it;
uncertain acknowledgments preserve ownership records for manual review.
`use-self-engine --consent` is for this source-build development host: it reuses
only the already verified engine at the compiled project's fixed proof path,
copies its ownership metadata to native local state, and selects it for new
installs. It does not move its virtual disk or change existing app bindings.
It is not a fresh-PC bootstrap command. Docker Desktop is not required.

When the Windows launcher or its agent connector uses the selected, verified
engine, it starts one hidden Local Store background worker. That worker keeps
the owned WSL session open after the launcher closes. It holds a singleton
file lock, checks selection and the original ownership journal repeatedly,
and verifies the in-distro ownership marker before replacing an exited pipe
helper. It never terminates a distribution, changes Docker contexts or stops
app containers. Windows sleep/wake and fresh-host resilience still need their
own acceptance proof; this is not a login task or Windows service.

```sh
local-store engine supervisor-status
local-store engine stop-supervisor
```

Status returns `null` when there is no live worker with recent ownership proof;
a saved PID or status file alone does not establish liveness. Stop requests
release of only Local Store's background lease. Existing containers are not
stopped, but WSL's normal idle policy can apply after the lease ends. A later
explicit app or engine operation can start the worker again. If Windows refuses
to start a worker outside an enclosing job, Local Store reports the failure;
it does not claim that the engine can survive that job's closure.

Qualification harnesses can explicitly set
`LOCAL_STORE_ENGINE_SUPERVISOR_PROCESS_ONLY=1` on their scoped child environment
to retain only a process-local lease and avoid leaving background test workers.
This does not bypass selection or ownership verification and does not change
the chosen engine. Do not set it globally for ordinary launcher use.

## Running the installed app from a shell

On Windows the installed executable is a GUI-subsystem image, so that
double-clicking the app never flashes a console window. That choice changes how
shells treat it, and the CLI behaves differently depending on how it is started.

| How it is started | Output | Exit code |
| --- | --- | --- |
| Another program (a script, an agent, CI) | Yes | Yes |
| `cmd.exe`, including `>` redirection and pipes | Yes | Yes, via `%ERRORLEVEL%` |
| `Start-Process -Wait -RedirectStandardOutput` | Yes | Yes |
| Windows PowerShell `local-store ...` directly | **No** | **No** |

Windows PowerShell starts a GUI-subsystem process without waiting for it, so it
returns before any output is produced and sets no `$LASTEXITCODE`. Nothing in
this program can change that. From PowerShell, use either form instead:

```powershell
cmd /c "local-store doctor --json"
Start-Process local-store -ArgumentList 'doctor','--json' -NoNewWindow -Wait -RedirectStandardOutput out.json
```

macOS and Linux have no such distinction; every invocation behaves normally.

If the reader of the output goes away first — `| head`, or a shell that did not
wait — the command stops quietly and reports success rather than failing.

## Search and page the catalog

```sh
local-store catalog
local-store catalog paperless
local-store catalog home assistant --limit 10
local-store catalog --query "photo gallery" --collection media --json
local-store catalog --capability preview_install --json
local-store catalog --license MIT --hide-warnings --limit 24 --offset 24 --json
local-store catalog --json -- --literal-search-text
local-store catalog --help
```

This command uses the launcher's cached catalog index and filter semantics,
including name aliases and multi-word search. With no arguments it shows the
first 24 entries, stable IDs, names, available actions and project source URLs.
The source URL describes the project; it is not a running instance to open.
The old CLI's silent 30-row truncation is replaced by explicit paging.

| Option | Meaning |
| --- | --- |
| Search words or `--query <text>` | Match all words against IDs, names, aliases, descriptions and tags; use one form at a time |
| `--category <name>` | Exact category |
| `--license <name>` | Exact software license |
| `--architecture <name>` | Exact container architecture, not desktop installer architecture |
| `--capability <value>` | `preview_install`, `connect` or `discover`; connect also includes previews with a web UI |
| `--collection <value>` | `writing`, `automation`, `media` or `developer` |
| `--hide-warnings` | Exclude projects with source-catalog cautions |
| `--offset <number>` | Zero-based offset, default 0 |
| `--limit <number>` | Page size from 1 to 48, default 24 |
| `--json` | Machine-readable page |
| `--` | Treat remaining tokens as literal search words, including words starting with a dash |

Value options accept both `--limit 12` and `--limit=12`. Invalid ranges, missing
values, unknown or duplicate flags fail with exit status 2. Valid searches exit
0 even with no results or an offset beyond the end. Category, license and
architecture values are case-sensitive; unmatched values return no results.

JSON contains the existing `CatalogPage` fields: `entries`, `total` (filtered
matches), `catalog_total`, `offset`, `limit`, `categories`, `category_counts`,
`licenses`, `architectures`, `snapshot_date` and `source_count`. Each entry keeps
its stable ID, provenance, icon path, capability and optional recipe ID.

The additional `next_offset` is an integer when another page exists and `null`
otherwise. Repeat the same search and filters using that offset until it is
null. The human-readable output also explains which offset to request next.
Catalog growth never automatically expands the reviewed offering allowlist.

## Implementation and verification

`src/cli_queries.rs` reuses `pico-args` 0.5.0 for parsing, the existing catalog
index for search, `doctor_with` for diagnostics and serde for JSON. The parser
adds no transitive dependencies and does not change the declared Rust minimum.
Connection and lifecycle commands now also use pico-args for argument
preflight before registry access or Docker execution. Unknown/duplicate flags,
extra names, invalid URLs and unsupported deletion flags exit 2. Existing
handlers retain their runtime and data-preservation checks. Action --help
shows the command summary without executing the action. Exact two-argument
native open still dispatches before the CLI, preserving dedicated windows.

`cargo test --locked` covers injected healthy/unavailable Docker reports,
invalid options, shared search semantics and compiled CLI stdout/exit behavior.
The executable tests use an empty PATH to exercise missing Docker; they
require neither a running daemon nor access to user app data. CI runs them in
Linux quality checks and the Windows/macOS build jobs when published.

### Structured Doctor failures

Each failed check from `doctor --json` includes
`"error": {"code": "process_unavailable", "message": "..."}` alongside its
existing readable `detail`. Codes are `process_unavailable` (executable or
working directory not found), `process_failed` (spawn/wait failure or nonzero
exit), `timed_out`, and `cancelled`. Successful checks omit `error`. Readiness
exit codes remain 0 (ready), 1 (not ready), and 2 (invalid arguments). Lifecycle
commands consume shared typed errors internally and retain readable stderr with
exit 1 for compatibility; operation-wide cancellation is not exposed yet. See
[command contract](command-contract.md) for the IPC and event formats.

### Native activation routing

`open <id-or-name>` opens the launcher and focuses the selected app window.
An exact app ID wins over a matching display name. Primary protocol links use
`localstore://open/<app-id>`; legacy links still accept display names. Protocol
activation requires exactly one URL with one path segment. Malformed requests
exit 2 before registry access; missing apps exit 1. Saved launch URLs are
validated before starting managed containers. Ordinary CLI commands, including
`--version`, retain their console output and exit-code behavior.

The official single-instance plugin forwards native opens to the running
launcher. The deep-link plugin handles startup and running-instance links;
repeated opens reuse the app window. Managed startup runs off the UI thread.
Failures after forwarding appear in the launcher; a successful secondary-process
exit acknowledges delivery, not completion of container startup. Ordinary CLI
queries still run independently. Cross-platform installed-app proof remains a
separate release gate; see the current agent handoff for tested platforms.
# Recovery inventory

`local-store recovery [--json] [--docker]` lists retained Compose files for supported
recipes absent from the registry. It is read-only; Docker is invoked only with
`--docker`, which verifies Compose project/service/path labels.
Candidates include intentionally preserved data, so their presence does not
prove an interrupted installation. Ownership status is reported explicitly.
The empty result is `[]` in JSON mode. See
[interrupted installation recovery](interrupted-install-recovery.md).

## Recovery actions

`local-store recover <app-id>` clears an owned retained setup, preserving its
data. `--delete-data` explicitly requests removal of owned app data.
`local-store adopt <app-id>` resumes a supported retained setup and registers
it only after readiness. Both re-check ownership under the operation lock;
the inventory is never authority for deletion. See the recovery guide.
