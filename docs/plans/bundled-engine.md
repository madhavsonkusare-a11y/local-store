# Managed container engine for V1

Updated September 13, 2026. Required by the owner; task status belongs to
E01–E04 in [V1_TASKS.md](../V1_TASKS.md).

## Selected development architecture

Build a Linux/amd64 Ubuntu 24.04 rootfs containing Docker Engine, CLI and the
maintained Compose plugin, then import it into a Local Store-owned WSL2 distro.
Reuse upstream packages, Compose semantics and our bounded process runner.
The [development payload](../../engine/README.md) now builds and exports with
five checksum-pinned Docker packages, a full installed-package inventory and
retained notices. That page records exact versions, provenance and build limits.

WSL2 still requires virtualization and Windows prerequisites, potentially with
administrator consent and a restart. The managed-engine promise removes a
separate Docker Desktop installation; it does not remove platform prerequisites.
Keep the existing Docker Desktop path available without silently migrating apps.

Rancher Desktop's rootfs construction and checksum verification are useful
references. Its complete Alpine/OpenRC/Kubernetes service stack is not our
payload. containerd/nerdctl and Podman remain possible future backends, but they
would need independent Compose and storage conformance proof. Driving the engine
API directly would require recreating dependency ordering and recovery behavior
that Compose already supplies.

## E01 verification checkpoint — September 13, 2026

A development rootfs was built and exported; its binaries report the locked
versions. [Evidence](../evidence/engine-payload-development-2026-09-13.json) is
explicitly unsigned and not WSL-boot-tested. The 128-package inventory is exact
for that build, but Ubuntu transitive dependencies still resolve at build time.
Finish the full dependency lock, signed-index provenance and package source/
notice obligations before approving a distributable payload.

Microsoft documents custom distro imports and systemd support. Docker recommends
maintained packages over static binaries for production updates. Moby/Compose
project licenses alone do not cover the complete distro's distribution duties.

- [WSL import](https://learn.microsoft.com/en-us/windows/wsl/use-custom-distro)
- [WSL systemd](https://learn.microsoft.com/en-us/windows/wsl/systemd)
- [Docker package installation](https://docs.docker.com/engine/install/ubuntu/)
- [Static binary update limits](https://docs.docker.com/engine/install/binaries/)
- [Moby license](https://github.com/moby/moby/blob/master/LICENSE)
- [Compose license](https://github.com/docker/compose/blob/main/LICENSE)

## E02 — One selected engine for every operation

New production installs now capture the effective local Docker endpoint into
`local-store-engine.json` beside `compose.yaml`. A Compose header marks bound
projects so deleting the binding file cannot silently restore ambient behavior.
Corrupt, unsupported and unreadable binding files refuse operations.

`src/runtime/engine.rs` routes commands with an explicit `--host` and removes
Docker context/host/TLS environment overrides in child processes. Install,
lifecycle, rollback and recovery reuse the saved endpoint. Qualification captures
one binding before starting and uses it for pulls, install/reinstall, service
inspection, bystander checks and cleanup. Recovery retains the selection in
memory across delete-data so its final check uses the same engine.

Fake alternate-executable tests and a [real Memos regression](../evidence/engine-binding-memos-2026-09-13.json)
cover routing and refusal. The real test invalidates its process's ambient
Docker context after installation; restart, reinstall and cleanup still pass.
It proves routing on an existing local daemon, not a managed WSL bootstrap.

E02 remains partial. Unmarked legacy projects retain their previous behavior;
explicit adoption is now available with `local-store bind-engine <id-or-name>`.
The current binding supports local
Unix sockets and Windows named pipes. Remote TCP/SSH and the future WSL broker
are not implemented. No existing app is automatically migrated to another engine.
Adoption reloads the registered app under its operation lock and verifies existing
containers on the selected local endpoint against the exact Compose file,
working directory and project labels. Multi-service projects are supported;
empty, duplicate-ID, truncated, foreign or one-off inventories refuse binding.
It runs only container listing/inspection, never retained Compose commands.
The saved binding and Compose marker are preserved on retry; a previously saved
endpoint wins over a new ambient default. A marker-write failure reports the
saved state and can be retried. This is endpoint adoption, not data migration or
a claim of immutable daemon identity. No existing user app was adopted during
development. A [real legacy-shaped Memos fixture](../evidence/engine-adoption-memos-2026-09-14.json) passed adoption, restart, reinstall, all-service checks and cleanup after its ambient context was invalidated. Multi-service adoption has fake-runner coverage; this does not prove WSL or cross-engine migration.

A WSL backend still needs persisted selection, working-directory and Compose/
bind-path projection, plus real conformance tests. Keep discovery separate from
migration; preserving a socket endpoint does not prove the daemon's immutable
identity or its future image/probe freshness (Q01).

### WSL transport contract — September 14, 2026

`src/runtime/engine/wsl.rs` adds a diagnostic-only transport through the existing
bounded `ProcessRunner`. It names `local-store-engine-v1`, root, `/`, the Docker
binary and Unix socket explicitly; `--exec` avoids a shell. A clean Linux
environment and removal of Windows `WSLENV` prevent ambient Docker context
overrides. Only the existing doctor's two commands are admitted. It is not wired
to discovery or installation and has not launched any distro. Bootstrap must
prove ownership before using it; a matching distro name alone is not ownership.

The lexical drive-path translator handles drive letters, spaces, Unicode and
extended Windows drive paths. It refuses UNC/device/relative/traversal paths.
It assumes the owned distro's verified `/mnt/<drive>` layout; it does not prove
mount existence, permissions or confinement. Sources checked September 14:
[Microsoft WSL commands](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)
and [automount configuration](https://learn.microsoft.com/en-us/windows/wsl/wsl-config).

Three focused tests and strict all-target/all-feature Clippy passed. This is a
preparatory seam, not an operational WSL backend. Next project the existing typed
plan into a separate Linux Compose artifact: translate host bind sources and
seed-file paths, preserve container targets and named volumes, and retain both
Windows and Linux path identities for recovery label checks. Then add versioned
WSL binding persistence and route lifecycle/qualification through that artifact.
Do not simply rewrite `-f`: Windows paths embedded inside YAML also need mapping.

## E03 — Bootstrap and payload integrity

The first E03 implementation slice is a durable ownership journal in
`runtime/engine/wsl/bootstrap.rs`. It binds the fixed distro name, absolute local
data directory, exact lowercase rootfs SHA-256 and a caller-generated ownership
token. Reservation uses exclusive file creation; existing, corrupt, oversized or
conflicting state is never overwritten. Only reserved → imported → verified is
accepted. This is transaction state only and performs no WSL operation.

The next slice must decode `wsl.exe` inventory output explicitly (UTF-16LE was
observed on the development host), verify both distro-name and data-directory
availability, then reserve before import. Recovery may unregister only when the
journal identity and imported distro identity both match; absence of either must
refuse deletion.

That inventory slice is now implemented. It parses bounded UTF-8 and the
NUL-separated ASCII shape produced when this project's current text runner
captures WSL's UTF-16LE output. Lossy replacement characters, malformed NUL
placement, control characters, oversized inventories and invalid names refuse.
The read-only preflight removes `WSLENV`, runs exactly `wsl.exe --list --quiet`,
checks the reserved name case-insensitively and refuses any pre-existing target
directory before invoking WSL. Tests prove collision paths create and remove
nothing. Non-ASCII distro output currently refuses because the lossy text runner
cannot prove its original bytes; a future raw-byte process result can broaden
this without weakening collision detection for the fixed ASCII name.

### E02 integration follow-up — September 14, 2026

The diagnostic-only checkpoint above is now extended by experimental schema-2
bindings (`wsl.exe`, `wsl://local-store-engine-v1`). Default discovery still picks
the existing local engine. `InstallSource` retains the resolved typed plan; a WSL
install requires that it exactly reproduces the original Compose source before
any file write or WSL call. The shared renderer projects bind sources into
`compose.wsl.yaml`, keeping named volumes, container paths, service configuration
and seed contents. Bind mounts use long syntax with `create_host_path: false`.
Quoted dollar signs are separately escaped for Compose interpolation.

Lifecycle maps the original Compose command to this required artifact; missing
or escaping artifacts refuse execution. Reinstall permits the companion file;
failed-install rollback restores both Compose versions. Recovery matches Linux
project/config labels and keeps its captured engine after deleting project files.
Windows fake-runner tests exercise install, lifecycle, reinstall, rollback and
post-deletion recovery. A real Docker Compose parser test checks a two-service
projection including named volume, host share, seed bind, Unicode and literal
dollars. This does not prove mounts, WSL boot, background operation or app tasks.

The initial parser assertion expected a single dollar in JSON; upstream
`docker/compose` `cmd/compose/config.go` deliberately re-escapes dollars when
exporting interpolated config. The regression now checks that exported form.
See [Compose volumes](https://docs.docker.com/reference/compose-file/services/#volumes)
and [interpolation](https://docs.docker.com/reference/compose-file/interpolation/).

Do not expose WSL selection until bootstrap verifies ownership. A distro name is
not ownership proof. An E03 journal must bind the reserved name, absolute data
directory, payload digest and registration identity, refuse collisions, and
retain enough information to recover an interrupted import. Add managed-engine
selection to qualification only with this verified bootstrap state. No WSL
distro was launched or imported for the integration tests.

Define an owned distro/data-disk layout and explicit Windows/WSL support matrix.
Verify available disk, virtualization and WSL before downloading. Verify an
authenticated manifest and payload digest before import; checksum equality alone
does not authenticate a remotely supplied manifest. Downloads and imports need
bounded execution, progress, cancellation and resumable failures.

An existing WSL installation is not a clean-machine test. Prove bootstrap on a
Windows x64 host without Docker Desktop, including consent/restart cases. Check
loopback forwarding, bind permissions, all Compose dependency conditions and
coexistence with an existing Docker Desktop installation.

## E04 — Background apps, repair and updates

Systemd services alone do not keep a WSL instance alive. Design and test an owned
supervisor that preserves background apps after the launcher closes. Exercise
sleep/wake, crash, restart and failed engine upgrade. Never use global WSL shutdown
or unregister another product's distro. Removing the engine and deleting app data
must remain separate, explicit actions.

Local Store maintainers own engine security updates: review upstream advisories,
refresh locks, rebuild and requalify, then sign and deliver the payload through
the updater. Keep replaceable engine binaries separate from durable app data.
Live restore is configured in the development daemon but is not evidence of a
working update/rollback system. Disk compaction needs an explicit offline
maintenance design and must target only the owned disk.

## Proof boundary

Requalify current offerings on the managed engine after E04 and Q01/Q02. Existing
Docker Desktop evidence cannot establish compatibility with the new daemon,
filesystem or network. All-service health checks now exist in qualification;
full engine identity, resource measurements and useful app tasks remain required.
V3 integration, signing credentials and release publication are separate gates.
