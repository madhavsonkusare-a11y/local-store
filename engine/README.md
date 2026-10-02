# Development engine payload

E01 implementation checkpoint, updated October 1, 2026. This is a buildable Linux
rootfs for the future managed WSL2 engine, not a shipping installer.

```text
python scripts/build-engine-payload.py
python scripts/test_engine_payload.py
python scripts/build-engine-payload.py --inspect-inventory
python scripts/build-engine-payload.py --build
python scripts/build-engine-payload.py --build --managed-engine
```

The validation and test commands are offline. `--inspect-inventory` uses Docker
to build only the unverified package stage and writes `packages.tsv` plus a
package-level `inventory-drift.json` under `.cache/engine/build-<unique-id>/`.
It never exports a rootfs. The final `--build` uses an existing Docker daemon to
build an isolated Linux/amd64 image, verify installed component versions, retain
the package inventory and notices, then export `rootfs.tar` with SHA-256 evidence
under `.cache/engine/build-<unique-id>/`. It does not register a WSL distro,
start an engine daemon, mount a host socket or run privileged containers.
Only the exact export container is removed. Build images/cache remain available.

`--managed-engine` builds through Local Store's already verified development
WSL engine instead of an ambient host Docker CLI. It checks both ownership
records, uses the fixed distro and local socket, and passes paths with spaces
as separate arguments. It does not require or start Docker Desktop. On a first
build, use an existing Linux container builder; the bootstrap-builder dependency
is separate from the Windows product's runtime requirements.

The [October 1 development export](../docs/evidence/engine-session-payload-development-2026-10-01.json)
locks 138 packages and measures 572,798,976 bytes. Adding `libpam-systemd` and
`dbus-user-session` resolves a measured root-session startup delay in the owned
development engine. The original 128 package versions are unchanged. The
[local repair](../docs/evidence/engine-session-local-repair-2026-10-01.json)
used a newer `libexpat1` patch (.6); the rebuilt payload uses snapshot patch .4.
These are distinct artifacts. The local repair does not prove a fresh boot of
the new archive, and the export remains unapproved for distribution.

## Package choice

Ubuntu 24.04 plus Docker's maintained packages gives us a conventional package
inventory and systemd units without implementing Compose or daemon supervision
from scratch. The base image is pinned to the Linux/amd64 manifest digest.
All five Docker package URLs, versions, lengths and SHA-256 values are locked in
[components.lock.json](components.lock.json); downloads must match before use.
Hashes were selected from Docker's HTTPS package index. They pin observed bytes;
this first version does not independently verify Docker's signed index. Ubuntu
dependencies are installed through apt's signed repository metadata.
The candidate Ubuntu archive snapshot is pinned to `20260913T120000Z` in the
component lock and passed to APT during the build. This uses Ubuntu 24.04's
official snapshot mechanism instead of whichever `noble-updates` happens to
be current. A September 30 clean build reproduced all 128 locked package
versions. A follow-up clean build retained 34 Ubuntu `.deb` archive hashes and
39 APT index hashes in a [provenance manifest](../docs/evidence/engine-ubuntu-provenance-2026-09-30.json);
the larger archive/index tars remain in the ignored local build cache. Signed
indexes and archive bytes still need independent verification against the
reviewed transitive package lock for release provenance.
The minimal Ubuntu base has no CA bundle, so the build first installs the
exact locked `ca-certificates` version from Ubuntu's signed current index,
then switches APT to the snapshot. Both index updates fail on fetch errors;
the final full inventory lock rejects any bootstrap package drift. The final
install explicitly downgrades the bootstrap OpenSSL pair to the snapshot-locked
versions, since the initial current index may supply newer versions.
The current 138-package result is frozen in [packages.lock.tsv](packages.lock.tsv).
The Dockerfile and exporter refuse a build if the installed inventory differs
byte-for-byte from this lock. This prevents silent dependency drift, but old
package versions may disappear from Ubuntu's moving repositories. The diagnostic
stage exists so a failed rebuild can identify every changed package without
loosening the export gate. A [September 28 diagnostic](../docs/evidence/engine-inventory-drift-2026-09-28.json)
found one changed Ubuntu dependency (`libapparmor1` `.7` to `.8`); the normal
post-lock build refused export. This is a measured blocker, not a release build.
The [September 30 snapshot build](../docs/evidence/engine-snapshot-development-2026-09-30.json)
resolved this drift and passed the exact inventory lock. It remains a
development build, not release approval. See [Ubuntu's snapshot
service](https://ubuntu.com/server/docs/how-to/software/snapshot-service/).

We inspected Rancher Desktop at `515877cd5f92af42089c9128fc3c7402c0fbbb6c`:
its WSL downloader verifies an expected rootfs checksum. Its separate distro
recipe builds an Alpine/OpenRC rootfs with Moby and Kubernetes/network helpers.
Reuse the build-and-verify pattern; Ubuntu's supported Docker packages avoid
adopting the complete Rancher-specific service stack. No Rancher source was copied.

- [Docker's Ubuntu packaging](https://docs.docker.com/engine/install/ubuntu/)
- [Rancher WSL downloader](https://github.com/rancher-sandbox/rancher-desktop/blob/515877cd5f92af42089c9128fc3c7402c0fbbb6c/scripts/dependencies/wsl.ts)
- [Rancher rootfs build](https://github.com/rancher-sandbox/rancher-desktop-wsl-distro/blob/main/files/build.sh)

## Recorded build and remaining release gates

[Build evidence](../docs/evidence/engine-payload-development-2026-09-13.json)
records the versions, rootfs hash and measured size (464,494,592 bytes,
uncompressed). The [128-package inventory](../docs/evidence/engine-packages-development-2026-09-13.tsv)
records the exact installed versions. These are observed development artifacts.

The complete installed-package inventory is now locked using the exact bytes
measured in the original development build. A clean post-lock export succeeded
on September 30 with the pinned Ubuntu snapshot. Cache or mirror every
transitive package with signed index provenance before claiming a reproducible
release build. The export preserves license files
inside the rootfs and a separate notices tar, but distributing a rootfs also
requires reviewing source obligations for the complete package set.

V1's proposed host floor is Windows 11 x64 with supported WSL2; the tested local
host has Windows build 26200.9445, WSL 2.6.3.0 and kernel 6.6.87.2. Those numbers
are a development host observation, not a clean-machine support matrix. Microsoft
documents systemd support from WSL 0.67.6; use a maintained WSL release and verify
the final supported floor during E03. [WSL systemd](https://learn.microsoft.com/en-us/windows/wsl/systemd).

`wsl.conf` enables systemd and excludes Windows PATH injection. Root execution is
explicit for the engine broker, not a claim of isolation from same-user shell
access. The daemon uses its local socket, bounded logs and live restore. No TCP
API is configured. Systemd alone does **not** keep WSL alive: E04 must provide
and prove background supervision and sleep/wake behavior. No global WSL shutdown
or unregister operation belongs in ordinary app lifecycle code.

Local Store maintainers own engine security updates. Before shipping: monitor
upstream advisories, review and rebuild the locked payload, run the stronger
qualification suite, sign the payload manifest, and deliver it through the updater.
Keep app data separate from replaceable engine binaries; prove failed-update
recovery before offering automatic engine upgrades. Signing keys, authenticated
delivery, WSL bootstrap, data-disk layout and rollback remain E03/E04 work.
