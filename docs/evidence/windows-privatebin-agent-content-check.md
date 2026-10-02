# PrivateBin through a networkless isolated browser

The reviewed PrivateBin 2.0.6 offering uses fixed DOM actions in an isolated Chromium process. It reuses the official Playwright 1.62.1 runtime rather than exposing a general browser tool to an agent.

The owner first connects the exact installed app in Settings, with explicit consent to prepare the browser runtime. No app credential or arbitrary paste link is accepted. Agents separately need a live app-specific content grant. A write then requires a second approval of the exact text, with at most two minutes to consume it once.

Every browser container has no network, runs as `pwuser`, enables Chromium's sandbox, drops all capabilities, uses the reviewed seccomp profile and no-new-privileges, and has a read-only filesystem with bounded temporary profile/shared-memory spaces. CPU, memory, process count and lifetime are bounded. It mounts only the reviewed read-only browser code and fixed runner. There is no host browser profile, engine socket, app data mount, published port, remote debugging endpoint, launcher IPC or caller-supplied code.

The Rust broker fetches bounded resources only from the validated app's exact loopback origin, with proxies and redirects disabled. Browser requests are fulfilled from those resources inside the container; other origins, frames, sockets, downloads and popups are blocked. Creation encrypts the owner's approved text through the app's own DOM and sends only the resulting ciphertext to that exact app in a single non-retried POST.

Only non-burning pastes with one-day expiry and no discussion can be created. Their fragment keys stay in the Windows-user-protected connection store and never become an agent-facing URL. Agents can read only paste IDs created through that connection. Arbitrary historical links are excluded because reading a burn-after-reading paste can destroy it. The protected connection supports at most 32 pastes. Disconnecting removes its fragment keys; encrypted server records remain until expiry.

Reconnecting the same exact installed target preserves its protected paste keys but clears prior client permissions and pending approvals. A fresh owner grant is required to read those pastes again. Disconnecting revokes every enrolled client's scope for that app before removing its keys; a later connection cannot revive old access.

The image is pinned to manifest-list digest `sha256:dcc5531e97840b9b5e794f2814476b21571c5124a3fca2267d73041f56e7580e`. Its actual `pwuser` identity is UID/GID 1001; both the runner and temporary home use that identity. The official npm package is checked against its locked SHA-512 and the complete extracted tree against SHA-256 `6e4e424e7d4651b64e250707f41548e71ef0620e61252b39f1a91d2354e0ece0`. The preserved upstream sandbox profile SHA-256 is `cc3e61cabda6bbc1e53e54d27ba4d55a9d3be829b6dd1a596f4a7b31b1cc7849`.

The reviewed runtime profile adds only an unconditional `chroot` rule, SHA-256 `8dff2d2308d7af9b30c020f3df33d374405604bc1b300fd45f739f8f2c8f5ead`. Docker filters the upstream capability-conditional rule when all capabilities are dropped; Chromium needs chroot inside its new child user namespace to finish its sandbox. A real blank-page smoke verified zero initial effective capabilities, denial of chroot in that initial namespace, and successful sandboxed Chromium startup with all other restrictions retained. See [Chromium's namespace sandbox implementation](https://chromium.googlesource.com/chromium/src/sandbox/+/refs/heads/main/linux/services/credentials.cc) and [Docker's capability-aware seccomp behavior](https://docs.docker.com/engine/containers/run/). This smoke proves startup only; a passing app content receipt is still required.

For a source/development build, run `python scripts/build-browser-provider.py` to stage the exact package beside the release executable. A missing/modified package blocks the provider. The first explicitly consented connection downloads the pinned browser image through the selected owned engine. Packaged delivery must include that reviewed payload; a source repository alone does not imply it is already installed.

`tests/managed_privatebin_content.rs` creates only a disposable PrivateBin fixture, exercises real DOM encryption/decryption, checks read/write scope, exact consent, cross-client/replay/destructive/arbitrary-link denial, malicious text, restart persistence, reconnection, revocation, audit privacy and removal of temporary browser containers. Only its passing actual JSON receipt establishes live acceptance. Mock UI or compilation alone does not.

The actual Windows proof on 2 October 2026 passed in 58.61 seconds. `privatebin-agent-content-2026-10-02.json` records a real encrypted paste round trip and restart readback, exact permissions and approval guards, absence of returned fragment keys, protected state, no grant revival after reconnect, secret-free audit, removed browser helpers and unchanged bystanders. Its provider, common boundary, fixture, runner and runtime-profile SHA-256 values match the current source, and it records the exact pinned browser image. The initial failed attempt found unsupported top-level WSL container aliases before any paste write; fixed `docker container` aliases preserve the existing owned-engine command allowlist.

This is a reviewed single-app adapter, not arbitrary web browsing, every historical paste, all-app coverage or native WebView acceptance. Permissions constrain this gateway; an agent with unrestricted access to the owner's Windows account can bypass them. A consumed but interrupted upstream write is not retried automatically.

Reviewed upstream references:

- [Official Playwright container guidance](https://playwright.dev/docs/docker)
- [Pinned upstream sandbox profile](https://github.com/microsoft/playwright/blob/v1.62.1/utils/docker/seccomp_profile.json)
- [PrivateBin 2.0.6 browser implementation](https://github.com/PrivateBin/PrivateBin/blob/2.0.6/js/privatebin.js)
