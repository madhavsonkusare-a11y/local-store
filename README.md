# Local Store

A Windows desktop workspace for discovering self-hosted apps, installing reviewed deployments, opening apps in native windows, and giving agents controlled access to them.

**Project paused as of October 4, 2026.** Source remains available; development and support are on hold. The installed development app, managed engine, private setup files and local build/test caches were removed. There is no current installer to download from the source release.

The [V1 source release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0) is published. The Windows product is unfinished: **23 of 31 product tasks were accepted (74.2%) at the pause**. This is a task count, not a readiness or delivery-time estimate. See the [release ledger](docs/V1_TASKS.md) for acceptance boundaries and [handoff](docs/agent-handoff.md) before resuming work.

## What is in the source

| Area | Implemented scope |
| --- | --- |
| Discovery | 1,678 embedded project entries with local icons or monogram fallbacks, search, filters, collections and source provenance |
| Installations | 53 offerings: three recipes and 50 approved templates, using reviewed image digests, typed setup, persistent storage, health checks and rollback |
| Windows engine | An owned WSL 2 engine using Docker Engine and Compose internally; Docker Desktop is not required for the product runtime |
| App windows | Connect a reachable HTTP(S) app and open it in a dedicated native window; external links open in the default browser |
| Lifecycle and recovery | Start, stop, inspect, uninstall with data preservation, recover/adopt retained setups and protected backup/restore within tested scopes |
| Agent access | Authenticated local MCP bridge, per-client/app grants, exact-action write approval, redacted audit and bounded app-content providers |
| Interface | Dark-only approved V2 design, bundled fonts/artwork, keyboard interactions and reduced-motion support; full native acceptance remains open |

Catalog presence is not installation approval. An offering is not proof of every app feature. The ten-app launch cohort has historical managed-engine task and lifecycle receipts: **Memos, Flatnotes, Kanboard, PrivateBin, Uptime Kuma, n8n, Gitea, WordPress, Jellyfin and Immich**. Each also has a bounded agent access path; read and write coverage differ by app. Receipts must be checked for expiry and source/image changes before new verification claims.

An arbitrary GitHub URL can be inspected and matched to a reviewed app or returned as a candidate. Local Store does not execute arbitrary repository commands or automatically approve every project for installation.

## What remains before a Windows product release

Managed-engine provenance, source/license delivery and packaging; complete native setup/lifecycle flows; the strict V2 capture gate; and a clean Windows installation run remain unfinished. Existing tests ran on a development laptop and do not certify a fresh PC. WSL 2 may require virtualization, administrator consent and a restart. Signing and automatic updates are deferred Windows distribution work, not requirements for the published source release.

Windows x64 is the product target. macOS/Linux code is retained for later scope. No universal app-upgrade, arbitrary source-build or unrestricted agent-automation promise is made.

## Build from source

For Windows x64, install Rust (the version is pinned in `rust-toolchain.toml`), Visual Studio C++ Build Tools and WebView2. From the repository root:

```powershell
cargo run --locked --release --features tauri/custom-protocol --bin local-store
```

This runs the launcher with its committed interface. Installing managed apps additionally needs a verified owned-engine payload and explicit setup consent; the source archive does not bundle a shipping rootfs. See the [engine reference](engine/README.md). Do not automatically restore that environment while the project is paused.

For the optional browser-only UI preview:

```powershell
npm ci
npm run preview
```

The preview does not provide native backend commands. The npm lockfile is used by CI; a matching pnpm lockfile is also retained. Choose one package manager for a working installation.

To connect an existing app from a built CLI:

```powershell
local-store add penpot --url http://localhost:9001
local-store list
local-store open penpot
```

See the [CLI reference](docs/cli.md), [contributing guide](CONTRIBUTING.md) and [local preview packaging procedure](PUBLISH.md#unsigned-local-windows-preview). A locally built unsigned installer remains a development preview, separate from the public source release.

## Documentation and design

Start with the [documentation index](docs/README.md). The [master ledger](docs/V1_TASKS.md) owns scope and task status; dated evidence is historical, and design approval alone does not certify implementation.

The approved [V2 handoff](docs/design/v2/HANDOFF.md), [brand deck](branding/brand-deck.html) and [identity guidelines](branding/README.md) remain in the repository. No V3 handoff is required.

## Security and license

Catalog browsing is offline. Local Store has no account, telemetry or analytics. Explicit installs, app pages and opt-in GitHub inspection use the network. Agent permissions are application boundaries; an unrestricted agent running under the same Windows user is not isolated by this gateway. Read [security/privacy](docs/security-and-privacy.md) and [support boundaries](docs/support.md).

Original launcher code is MIT: [LICENSE](LICENSE). Imported app definitions, icons, fonts and dependencies retain their licenses: [third-party notices](THIRD_PARTY_NOTICES.md).
