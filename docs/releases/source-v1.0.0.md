# Local Store V1 source release

This release publishes the Local Store source snapshot. It contains GitHub's
generated ZIP and tar.gz source archives, with no Windows installer, bundled
engine image, signing certificate or automatic updater. The Windows product
remains in development. This milestone tag is `source-v1.0.0`; the launcher
package version remains `0.5.0-1`.

## What is available

- Offline discovery of 1,678 catalog entries, with a bundled icon or local
  monogram for every entry.
- Installation support for 52 reviewed offerings, using existing recipes and
  templates, pinned images, health checks, persistent data and bounded runtime
  operations. Catalog presence alone does not imply installation support.
- Managed-engine task evidence for ten selected apps: Memos, Flatnotes,
  Kanboard, PrivateBin, Uptime Kuma, n8n, Gitea, WordPress, Jellyfin and Immich.
  Each has meaningful exact-state task proof through restart and keep-data
  reinstall. This evidence does not certify every machine or future app version.
- Native app windows, connections to existing local/LAN/HTTPS apps, registry
  migration, CLI management, interrupted-install recovery and data-preserving
  uninstall.
- A dark interface using the approved V2 design, including live Overview and
  My Apps surfaces. Complete first-run, installation and recovery flows remain
  active work.
- Development managed-WSL engine support and an owner-enrolled, grant-scoped
  stdio MCP sidecar for bounded app management. Complete agent content access
  across the roster remains unfinished.

## Build and run from source

Windows x64 development needs Rust, Visual Studio C++ Build Tools and WebView2.
The repository pins Rust in `rust-toolchain.toml`. From the extracted source:

```powershell
cargo run --locked --release --features tauri/custom-protocol --bin local-store
```

Docker Desktop or an explicitly configured Local Store managed engine is
needed to install apps. For setup details, optional local installer commands
and contribution checks, read the
[README](https://github.com/madhavsonkusare-a11y/local-store/blob/source-v1.0.0/README.md),
[contributor guide](https://github.com/madhavsonkusare-a11y/local-store/blob/source-v1.0.0/CONTRIBUTING.md)
and [publishing guide](https://github.com/madhavsonkusare-a11y/local-store/blob/source-v1.0.0/PUBLISH.md).

## Current limits

Fresh-PC engine bootstrap and consent, complete V2 flows, agent content access,
backup/restore and Windows product release proof remain incomplete. Resource
ceilings are proven for Flatnotes; the other nine selected apps still need
reviewed limits. Kanboard's upstream default administrator credential needs
to be changed before use with sensitive data. Default local app ports are
loopback-only; deployment beyond the local machine requires separate review.
Arbitrary GitHub repositories are inspected as metadata and are not
automatically approved or installed. App upgrades and rollback are not
established by keep-data reinstall evidence. Existing macOS/Linux code is
retained for later scope without active certification.

Original launcher code is MIT licensed. Catalog data, fonts and artwork retain
their own terms; see
[third-party notices](https://github.com/madhavsonkusare-a11y/local-store/blob/source-v1.0.0/THIRD_PARTY_NOTICES.md).
Eight gallery icons without a declared redistribution license were replaced
with existing local monograms before this release. Source review uses a
pinned secret scanner and the existing dependency-license, catalog and app
evidence checks; it is not a security audit of the apps themselves.

Follow the [master task ledger](https://github.com/madhavsonkusare-a11y/local-store/blob/main/docs/V1_TASKS.md)
for current product progress and the next implementation batch.
