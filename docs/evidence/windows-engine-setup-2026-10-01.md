# Windows engine setup and explicit selection

Date: October 1, 2026. Scope: owner-controlled source builds and the existing
managed WSL engine. This is not a clean-machine certification or a distributable
engine approval.

`src/engine_setup.rs` adds the native owner service used by launcher/CLI actions.
Its preview exposes prerequisite, payload, available disk and ownership facts;
its action API accepts only a typed action and three explicit consent values.
Native resource resolution supplies the fixed `engine/rootfs.tar` path. The
browser cannot choose a file, payload digest, WSL distribution or data directory.
Viewed onboarding steps continue to carry no authorization.

The rootfs identity is the October 1 session-package development build: 572,798,976
bytes, SHA-256
`ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32`.
Missing or mismatched payloads refuse import. The engine preview identifies this
as a verified development payload with `payload_release_approved: false`.
Its conservative disk admission budget is 2,255,267,840 bytes, calculated as
three observed archive lengths plus 512 MiB of setup headroom. Unknown free space
and lower measurements refuse setup. This budget is not a measured fresh-Windows
minimum and does not reserve storage for application images or app data.

Install, clean retry, verification resume, explicit repair and managed-engine
selection reuse the bounded bootstrap coordinator and the shared cross-process
`managed-engine` operation lock. Uncertain ownership requires manual review.
No action invokes global WSL shutdown, sets a default distro, changes a global
Docker context or unregisters another product's distribution.

An explicit development reuse action reads only the fixed native source-build
`.cache/engine/real-wsl-proof/state` location. It requires a Verified journal, its
data disk at the corresponding canonical sibling `data` path, and matching
external and in-distro ownership tokens. It copies ownership metadata into the
native local state root. The existing virtual disk, apps and original proof
record remain in place. A conflicting destination journal refuses replacement;
a retry can complete only a missing token from the freshly proven original.

New installs require an explicit saved choice of the owned Local Store engine.
There is no automatic Docker Desktop endpoint fallback. Existing per-app engine
bindings retain their previous endpoints. The saved selection is bounded and
validated; its durable marker prevents a missing selection file after an
interrupted Windows replacement from restoring ambient routing. A selected
managed engine rechecks ownership and daemon readiness before a new install.

The [actual selection and Doctor receipt](windows-self-engine-selection-2026-10-01.json)
records the owned WSL engine selected with explicit owner consent and responsive
Docker 29.8.0 / Compose 5.5.1 while Docker Desktop stayed stopped. The
[native preview receipt](windows-self-engine-preview-2026-10-01.json) verifies
the staged development payload and reports sufficient measured disk space.
Its payload remains `payload_release_approved: false`. Native CLI preview
exposed a prior 1 MiB stack buffer overflow; checksum streaming now uses a
64 KiB heap buffer, with a small-worker-stack regression check.

## Targeted checks

All four owner-service tests passed in a release library test run:
specific consent and unknown fields; exact disk threshold/unknown-space refusal;
and saved-selection corruption, oversize, remote endpoint and missing-file
refusal. The Windows-only regression exercises owned metadata reuse,
retrying a missing token, conflicting destination identity, and source-token
tampering. It asserts no import, unregister or global shutdown command is issued.
Command: `cargo test --locked --lib --release --features tauri/custom-protocol
engine_setup::tests:: -- --nocapture`. Result: 4 passed, 0 failed.

## Remaining proof boundary

This work does not enable Windows optional features or grant administrator
rights automatically. Prerequisite failure tells the owner that WSL setup may
require elevation and a restart. WSL/rootfs bootstrap on a clean supported
Windows machine without Docker Desktop, measured installation disk minimum,
supported-host matrix, sleep/wake and engine-removal/data-separation proof still
remain. Ubuntu archive signature/provenance and complete rootfs redistribution
obligations remain E01 review items. E02/E03/E04 must not be marked DONE based
only on these service tests.
