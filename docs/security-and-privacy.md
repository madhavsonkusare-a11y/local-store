# Security and privacy

What Local Store sends, what it stores, what it enforces, and — just as
importantly — what it does not protect you from. Statements here describe the
code in this repository; the limits section is not boilerplate.

## What leaves this computer

**The bundled launcher has no analytics, telemetry, crash reporting or account
system.** Its Rust health probe (`src/runtime/mod.rs`) connects to configured
app addresses to check whether they answer a plain HTTP request. While My Apps
is visible, eligible displayed apps are checked again 15 seconds after the
previous round completes, with at most four probes in flight. Checks pause
when the launcher is hidden or another section is selected, and skip apps
with an operation in progress. HTTPS readiness is reported as unknown.

The launcher interface cannot reach the network either: its Content Security
Policy restricts connections to the local IPC channel, and a UI test asserts
that browsing the catalog issues no remote requests.

These actions also involve the network:

| Action | What happens |
| --- | --- |
| Installing a reviewed recipe | The selected Docker engine pulls pinned images from their registries. The development managed engine imports a reviewed local rootfs; a complete fresh-PC setup flow remains unfinished. |
| Opt-in GitHub source inspection | The Rust backend contacts GitHub's public API to inspect the requested repository and pin metadata. This does not approve installation or run repository code. |
| Opening a link that leaves an app | The URL is handed to your default browser, which then does whatever it normally does. |
| Checking an address | A single TCP connection and one plain HTTP request to the address you typed. |
| Opening an app window | The embedded browser loads that app's page and resources. Third-party app code may contact additional services according to its own behavior and privacy policy. The launcher's network restriction does not apply to those pages. |

Browsing the bundled catalog is entirely offline. The integration baseline
contains 1,678 entries, each with a local icon or generated monogram.

## What is stored, and where

- **App registry** — `%APPDATA%/local-store/registry-v2.json` on Windows, the
  platform configuration directory elsewhere. Names, addresses, icons and
  runtime details of apps you added. Written atomically, with the previous
  version retained alongside it.
- **Managed app data** — `<config>/local-store/apps/<app id>/`. Compose files
  and each app's own data directories.
- **Generated secrets** — `local-store-secrets.json` in that same app
  directory, for apps installed from a deployment plan. A credential is kept
  beside the data it unlocks rather than in the registry, which lists every
  app. Uninstalling while keeping data keeps this file, so a reinstall reuses
  the same credential instead of locking the app out of its own database;
  deleting the data removes it with everything else. The values also appear in
  that app's Compose file, because the containers need them — the same as any
  Docker Compose deployment.
- **Browser data** — the platform WebView may retain cookies, caches and web
  storage in its browser profile. Removing a connection deletes its registry
  entry; it does not clear that browser profile or the remote server's data.
- **Agent policy** — `agent-policy-v1.json` under the Local Store configuration
  directory contains owner-enrolled credential hashes, scoped grants and bounded
  redacted audit. Raw bearer credentials are returned at enrollment and must be
  kept by the owner/client; the policy store does not retain their plaintext.
- **OS integration** — protocol registrations and requested desktop shortcuts
  are stored by the operating system. Docker also maintains its own images,
  containers and volumes outside the registry directory.

## What the application enforces

Each of these is a deliberate boundary with a test behind it.

- **App windows cannot drive the launcher.** Only the window labelled
  `launcher` is granted any command. A test ties every registered command to its
  build declaration and capability grant, so a new command cannot be shipped
  ungranted — or granted without being declared. The Windows native smoke test
  additionally confirms a remote page is denied four sensitive commands.
- **Content Security Policy.** `default-src 'self'`, no inline or evaluated
  script, `object-src`/`frame-src` `none`. Asserted by test.
- **Every URL that can leave the app is validated.** Absolute `http`/`https`
  only, at most 2048 bytes, no embedded credentials, no control characters.
  `javascript:`, `file:` and `data:` URLs are refused before anything opens.
- **Navigation isolation.** A navigation away from an app's own origin is not
  followed in that window; it is handed to your default browser instead.
- **Reviewed recipes are constrained.** Images are pinned to exact versions —
  `latest`, `stable` and `main` are refused. Ports publish to `127.0.0.1` only.
  Compose files containing `privileged`, a Docker socket mount, host networking,
  host PID or IPC, added capabilities, device passthrough or a root bind mount
  are rejected. A build-time check confirms each manifest agrees with the
  Compose file it ships, using Docker's own parser.
- **Registry changes are serialised across processes.** Every read-modify-write
  of the app registry holds an exclusive lock on a sidecar file, so the launcher
  and the command-line tool running together cannot silently discard each
  other's changes. The wait is bounded and reports a busy registry rather than
  hanging; the operating system releases the lock if a process dies.
- **Data deletion is confined.** Deleting an app's data is permitted only for
  exactly `<managed root>/<app id>`, and the path is re-checked after being
  resolved, so a link or traversal cannot redirect the delete elsewhere.
- **Credentials are redacted from diagnostics.** URL credentials, values of keys
  such as `PASSWORD`, `TOKEN`, `SECRET` and `auth`, and `Authorization` headers
  are replaced before a failure is shown or copied into a bug report. App logs
  are deliberately *not* redacted: they are your app's own output, which
  `docker compose logs` would show verbatim anyway.
- **Docker commands are bounded.** Every command has a deadline and captures at
  most 256 KiB per stream. On timeout or cancellation the process tree Local
  Store created is terminated — never your Docker daemon or unrelated
  containers, which are not in that tree.
- **Dependency licences are checked.** The build fails if any resolved Rust
  dependencies cannot be redistributed under a permissive licence.

## What this does not protect you from

- **The V1 GitHub Release contains source only.** Local Windows previews are
  unsigned and have no automatic updater. Signing and update distribution are
  deferred; this source release does not certify an installer.
- **The apps you run are not audited.** Local Store gives an app a window; it
  does not review that app's own security, authentication or update practices.
  A self-hosted app on your network is as exposed as you configure it to be.
- **Installation proof is bounded.** Recipes and approved templates carry
  lifecycle evidence, and Memos has historical upgrade evidence. Generic
  first-page checks do not prove every meaningful app task or future upgrade.
  Ten selected apps have exact-state task proof through restart and keep-data
  reinstall; the other offerings and future versions do not inherit that proof.
- **The address check speaks plain HTTP.** An `https://` address is reported as
  "cannot be checked from here" rather than verified; no certificate is
  validated, because no HTTPS request is made.
- **Lifecycle operations take per-app cross-process locks.** The launcher
  and CLI coordinate install/start/stop/uninstall through `lock_operation`.
  Registry updates separately hold their own cross-process lock. This is not
  a security boundary against another process running as the same user.
- **Native privilege review is incomplete.** The Windows release-image smoke
  test proves denial of four launcher commands from a remote page. It does not
  certify complete agent access or every approved V2 flow; macOS and Linux native proof
  remains outside the Windows release target.
- **Installer evidence is version-specific.** A clean Windows installer
  run is recorded in `evidence/windows-installer-2026-09-08.json`. The future
  Windows engine setup, complete V2 flows and agent access still need their own
  product release proof; updates are later scope.

## Reporting a vulnerability

Report privately through GitHub's security advisories on
[the repository](https://github.com/madhavsonkusare-a11y/local-store), rather
than as a public issue. Please include the version, your platform, and the
smallest set of steps that shows the problem.

This is a preview project maintained by one person: there is no guaranteed
response window, and no bounty.

## Release procedure

See [`PUBLISH.md`](../PUBLISH.md) for the release checklist, and
[V1_TASKS.md](V1_TASKS.md) for what is verified and what is
still outstanding before a release could honestly be called shippable.
