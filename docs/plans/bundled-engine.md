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

A WSL backend still needs distro/user selection, working-directory and Compose/
bind-path translation, plus conformance tests. Keep discovery separate from
migration; preserving a socket endpoint does not prove the daemon's immutable
identity or its future image/probe freshness (Q01).

## E03 — Bootstrap and payload integrity

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
