# Third-party runtime notices

`seccomp_profile.upstream.json` is copied without modification from Microsoft Playwright v1.62.1 at `utils/docker/seccomp_profile.json`, SHA-256 `cc3e61cabda6bbc1e53e54d27ba4d55a9d3be829b6dd1a596f4a7b31b1cc7849`. Playwright is licensed under Apache-2.0. Its sandbox profile is based on Docker's default seccomp profile, also Apache-2.0.

Local Store modifies the runtime `seccomp_profile.json` by adding one unconditional `chroot` syscall rule. SHA-256: `8dff2d2308d7af9b30c020f3df33d374405604bc1b300fd45f739f8f2c8f5ead`. Chromium needs this syscall inside its child user namespace, while the upstream conditional rule is removed when Docker drops all initial capabilities. The kernel still denies chroot to the capability-free initial container namespace. A real blank-page smoke verified both that denial and Chromium sandbox startup under non-root UID/GID 1001, no network, no-new-privileges and all capabilities dropped. This keeps Chromium's sandbox enabled and grants no container capability.

The development payload builder stages the official `playwright-core` 1.62.1 npm distribution, preserving its LICENSE, NOTICE and ThirdPartyNotices files. Chromium and Ubuntu container dependencies retain their upstream licenses and notices inside the pinned official image. Distribution of that large runtime remains a separate packaging/provenance review; this adapter does not close the bundled-engine redistribution gate.

PrivateBin app resources are fetched from the exact installed app at operation time. The app's own Zlib license and image review remain attached to its offering. This adapter does not copy the upstream app resources into the Git repository.
