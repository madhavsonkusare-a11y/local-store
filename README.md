# Local Store

Discover self-hosted software, install reviewed local recipes, connect the
instances you already run, and open them in dedicated desktop windows.

Point Local Store at any local web app — Penpot, your homelab dashboard, a
self-hosted tool — and it opens in a real native window with that app's name
and icon. Links outside the connected app's origin open in your **default
browser**. Local, LAN and hosted HTTPS instances are supported.

## Why

Local Store brings app discovery, reviewed Docker Compose recipes, and native
desktop windows into one workspace. It builds on Tauri and the operating
system's webview, with an open-source, MIT-licensed launcher.

The interface is dark-only: a warm, visual app shelf, clear capability labels,
and native dialogs with keyboard and reduced-motion support.
See the [refined brand deck](branding/brand-deck.html) and
[identity guidelines](branding/README.md).

The [V1 GitHub Release](https://github.com/madhavsonkusare-a11y/local-store/releases/tag/source-v1.0.0)
is **source-only**.
It has no Windows installer or automatic updater. The Windows product is still
in development: ten selected apps have managed-engine lifecycle and
meaningful-task proof, while the bundled-engine setup, complete V2 interface
and agent access remain unfinished. See the [V1 task ledger](docs/V1_TASKS.md)
for current evidence and release status.

## How it works

- **Discover** searches the embedded project catalog in bounded pages. A project
  website is presented as a source link and is never treated as your instance.
  Browse **1,678 catalog entries**, four interest collections, combined filters and
  source provenance. Every entry has a bundled icon, including monogram fallbacks.
- **Install** supports **52 offerings** (three recipes and 49 reviewed templates) with digest-pinned images,
  a Docker/Compose preflight check, persistent local data, health verification,
  and rollback when setup fails.
  Qualification evidence is recorded in `docs/evidence/`. Application-specific
  end-to-end use is not proven for every offering; see the
  [qualification plan](docs/plans/qualification.md).
- **Connect an app** saves its name and reachable HTTP(S) address. Local Store
  does not seed an example or imply that catalog projects are already installed.
- **My Apps** opens connections and starts, stops, inspects, or uninstalls apps
  managed by Local Store. Uninstall preserves app data unless deletion is
  explicitly selected and confirmed.
- Apps use a versioned registry at `%APPDATA%/local-store/registry-v2.json` on
  Windows (or the platform config directory elsewhere). Existing v1 and legacy
  registries are imported once with a backup.
- The launcher lists them; clicking **Open** spawns a native window to that URL.
- A tiny injected script intercepts `window.open` and external `<a>` clicks,
  rewriting the navigation to a `localhost` marker URL. Rust catches that in
  `on_navigation` and launches the OS default browser.
- Apps with a `compose` file boot their Docker stack and wait for a health
  check before opening.

## Build

On Windows x64, install Rust, Visual Studio C++ Build Tools and the WebView2
runtime. The repository selects its Rust version through `rust-toolchain.toml`.
Run the source with its committed interface embedded:

```powershell
cargo run --locked --release --features tauri/custom-protocol --bin local-store
```

App installation uses Local Store's explicitly selected, owned WSL 2 engine;
Docker Desktop is not required. Open Settings → Local Store engine to review
setup, disk use and consent. Windows may need WSL 2 enabled and a restart.
The engine uses Docker and Compose internally. Fresh-PC payload delivery remains
in development; the source-only release does not contain an engine archive.
For an optional local installer with the MCP sidecar, follow
[the local preview build procedure](PUBLISH.md#unsigned-local-windows-preview).
The V1 source release does not include that installer.

Add your own connection through the launcher or CLI:

```bash
local-store doctor
local-store doctor --json
local-store catalog --capability preview_install --json
local-store catalog --collection media --limit 24
local-store install memos
local-store install n8n
local-store install uptime-kuma
local-store add penpot --url http://localhost:9001
local-store list
local-store status memos
local-store logs memos
local-store stop memos
local-store start memos
local-store open memos --browser
local-store open memos             # dedicated native window
local-store shortcut memos
local-store remove penpot
local-store uninstall memos
local-store --version
```

See [CLI search, paging and diagnostics](docs/cli.md) for filters, JSON fields
and exit codes.

Compatibility (one release): previous registry locations and launch links are imported/recognized.

## Roadmap

### v0.2 (shipped ✅)
- [x] `local-store add <name> --url <u> --icon <i>` Rust CLI (replaces `cli.js`)
- [x] Docker Compose boot: `docker compose up -d` + health check before open
- [x] Start Menu shortcut generation with the app's icon
- [x] `localstore://open/<name>` protocol handler
- [x] Per-app icon on the native window title bar

### v0.3 (shipped ✅)
- [x] **Cross-platform registry path** — `apps.json` now uses `PathBuf` (was hardcoded `\`, broken on Linux/macOS)
- [x] **Unit tests** for the registry (path, dedup, preset lookup)
- [x] **`local-store --version`** / `local-store version` subcommand
- [x] **GUI parity with CLI** — icon, compose, and health inputs; per-row Remove button; app icon + 🐳 compose badge in the launcher
- [x] **macOS `localstore://`** registered via bundle `Info.plist` (`CFBundleURLTypes`)

### v0.4 (shipped ✅ — previous binary release)
- [x] **Embedded app catalog** — 1,257 self-hosted app entries bundled into the binary
- [x] **Catalog-backed setup wizard** — browse and configure catalog apps from the launcher
- [x] **Reference recipe data** — 12 curated entries document Compose and health-check metadata for future integration
- [x] **Broad icon coverage** — verified icon sources plus favicon fallback for entries without one

### v0.5 (in progress)
- [x] Versioned v2 registry with one-time legacy migration and recovery
- [x] Three reviewed recipes with pinned images and persistent data
- [x] Docker doctor, transactional install, health verification, and rollback
- [x] Managed app status, start, stop, logs, and data-preserving uninstall
- [x] Validate every reviewed recipe with Docker Compose in CI
- [x] Dark-only visual workspace, refined identity, and accessible motion
- [x] Approved top-aligned logo and mathematically verified corner spacing
- [x] Four reproducible source imports, stable catalog IDs and offline artwork
- [x] Combined discovery filters, collections, source details and Settings/Doctor
- [x] Protocol shortcuts and native CLI opening with stable window IDs
- [ ] Run clean-machine installer and live-container smoke tests

See the [V1 release task list](docs/V1_TASKS.md), [documentation index](docs/README.md),
and [catalog contribution guide](docs/catalog.md).

## Security and privacy

Browsing the bundled catalog is entirely offline. Local Store has no account,
telemetry or analytics. Explicit installs pull images, app windows load their
pages, and opt-in GitHub inspection contacts GitHub's public API.
[Security and privacy](docs/security-and-privacy.md) covers
what is stored, the boundaries the app enforces, what it does **not** protect
you from, and how to report a vulnerability.

## License

Original launcher code: MIT — see [LICENSE](LICENSE). Imported catalog material,
fonts and icons retain their licenses; see [third-party notices](THIRD_PARTY_NOTICES.md).
