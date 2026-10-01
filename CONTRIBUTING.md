# Contributing

Local Store is small by design. Contributions welcome.

## Dev setup

Windows x64 development needs Rust (the repository pins the toolchain in
`rust-toolchain.toml`), Visual Studio C++ Build Tools and the WebView2 runtime.
Docker Desktop or an explicitly configured Local Store managed engine is
needed to install apps; it is not needed just to compile the launcher.

```powershell
# Compile and launch with the committed interface embedded.
cargo run --locked --release --features tauri/custom-protocol --bin local-store

# Optional browser-only UI preview; backend commands are unavailable here.
npm run preview
```

For an optional local Windows installer with its MCP sidecar, install the
same CLI version as CI (`cargo install tauri-cli --version "=2.11.4" --locked`)
and follow [the local preview build procedure](PUBLISH.md#unsigned-local-windows-preview).
The V1 GitHub Release contains source archives only.

## Conventions

- Keep it dependency-light. The current footprint is Tauri, serde, serde_json,
  and narrowly scoped platform APIs; add dependencies only with a clear need.
- External-link routing lives in `src/windowing.rs`. Preserve origin checks and
  the launcher-only Tauri capability boundary.
- App registry is `%APPDATA%/local-store/registry-v2.json` on Windows, under the
  platform config directory elsewhere. Preserve migration/recovery semantics.
- GUI-subsystem binary on Windows: keep `#![cfg_attr(not(debug_assertions),
  windows_subsystem = "windows")]` — no console window on launch.

## Before publishing changes

- `cargo fmt --all -- --check`, clippy with warnings denied, and `cargo test --locked`
- `npm ci` and `npm test` (Windows visual baselines; Linux CI checks interactions/axe)
- Offline catalog/icon/brand checks described in [the catalog guide](docs/catalog.md)
- Update the current [V1 task ledger](docs/V1_TASKS.md) with actual evidence

For the next bounded development batch, start with the
[agent handoff](docs/agent-handoff.md). It contains scope, acceptance criteria,
validation commands and current limitations without requiring chat history.

## Current focus

The baseline has 52 offerings and 1,678 discovery entries. The source-only V1
GitHub milestone requires audited source, accurate docs and a verified tag;
signing and updates belong to later Windows distribution. The Windows product
targets ten task-verified apps, a managed container engine, the approved V2
interface and controlled agent access. The ten-app proof count passes, while
complete engine setup, interface flows and agent content access remain active
work. Use the master ledger rather than historical phase notes to choose a
task. Reuse the existing runtime and adapters; do not promote imported
definitions automatically.

See the [documentation index](docs/README.md). Keep detailed plans under
`docs/plans`, unapproved studies under `docs/research`, and the handoff short.
