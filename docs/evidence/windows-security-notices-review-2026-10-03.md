# Local Windows security, notices and support review

Reviewed October 3, 2026 against integration baseline `2f2c102` and the current
local source. This completes the locally achievable document/notice review
portion of R05. R05 remains partial because current downloaded Windows artifact
and clean-host acceptance depend on R04. This review neither distributes a
Windows installer nor changes the completed source-only GitHub milestone.

## Implemented boundaries reviewed

| Surface | Current code and evidence | Review result / limit |
| --- | --- | --- |
| Launcher/app windows | `tauri.conf.json`, `src/windowing.rs`, existing native privilege smoke | Launcher-only commands, validated URLs, origin separation and launcher CSP retained. Remote app pages have their own network behavior. Complete current native V2 flow proof remains separate. |
| Protocol/shortcut | `src/activation.rs`, `src/platform/shortcut.rs` | Existing app-ID resolution, strict single URL/arguments, no query/fragment/credentials/traversal; shortcuts are protocol data. Installed handler behavior on a clean PC remains R04. |
| Setup and stored secrets | `src/runtime/mod.rs`, template/recipe metadata, `src/agent_content/protected.rs` | Required setup and sensitive fields remain app-specific. Generated Compose secrets are local plaintext; broker app credentials are DPAPI protected. Kanboard's known initial administrator password/plugin caution stays explicit. No promise that DPAPI encrypts all app data. |
| Engine ownership/consent | `src/runtime/engine/wsl/`, `src/engine_setup.rs`, owned-engine receipts | Exact local development payload, consent, selection, token/journal ownership and bounded retry/repair. Refuse uncertain ownership; no global WSL shutdown or ambient context changes. Prerequisite failure/clean-PC/product engine-removal acceptance remains open. |
| Client identity/grants | `src/agent_gateway/`, `src/agent_policy/`, `src/agent_requests.rs`, owner/content receipts | Enrollment, connection, exact expiring client/app/action grants and one-use exact write/mutation approvals are separate. Raw bearer configuration is private. Revocation/replacement/reconnect invalidate older scopes; metadata audit excludes credentials and content. Same-user unrestricted processes remain outside the broker boundary. |
| Untrusted content/providers | `src/agent_content/`, ten-app provider receipts, content-boundary review | Fixed bounded loopback operations, read-only file subset and isolated browser. Malicious app text is data; it cannot issue permission commands. Exact actions/setup are documented in the launch-app matrix. Optional pinned browser runtime distribution review remains separate. |
| Backup/recovery | `src/backup.rs`, `src/backup/protection.rs`, real Memos/Gitea restore receipt | Quiesced owned storage/credentials, DPAPI, link/path/type/occupied-target refusal, removed plaintext staging. Same-account local recovery; shared folders, remote effects, account-loss portability and automatic version undo are not proven. |
| Support/diagnostics | `docs/support.md`, diagnostic redaction code, verbatim app logs | No response guarantee; ordinary bugs and private security reporting separated. Sharing instructions exclude tokens/configuration/backups and require manual review of logs/screenshots. Current target/preview/roster limits are explicit. |

Current source has **53 offerings**, with the same selected **10** launch apps.
Linkding has separate candidate app-task proof and unverified content access;
other offerings do not inherit launch/content proof. This review did not alter
manifests, providers, fixture/probe hashes, runtime code or qualification evidence.

## App, asset and dependency material

- Catalog adaptations preserve upstream licenses, attribution, source revisions
  and transformations. App image license labels remain separate from definition
  licenses; registry pulls do not establish permission to mirror every image.
  The source milestone includes no engine/browser/app image binary asset.
- All local artwork is covered by the existing icon manifest and upstream notice
  records; undeclared gallery artwork stays replaced by existing monograms.
  Instrument Sans and IBM Plex OFL notices were omitted from the old installer
  resource list. Both actual font OFL files and the root MIT license are now
  configured while preserving existing resources. OFL condition 2 requires
  retaining copyright/license with redistributed fonts. [Official OFL](https://openfontlicense.org/open-font-license-official-text/).
- Offline exact-version Rust collection covers **512** locked all-target packages.
  Notice texts for **483** packages are retained, including exact-revision upstream
  supplements for the **12** Windows-target packages whose crate archives omit
  standalone notices. The collector checks supplement hashes and recorded archive
  VCS revisions. **Zero Windows-target packages** lack a collected notice file.
  The remaining 29 missing standalone files belong to other target graphs and
  remain visible limits; no non-Windows binary approval is implied.
- The inventory is a conservative target/development superset, not an exact
  compiled-binary SBOM or a complete vendored-subcomponent audit. SPDX expression
  acceptance does not replace attribution, NOTICE or source-delivery review.
- Five exact unmodified MPL source `.crate` archives are retained with checksums
  matching `Cargo.lock`, clear local directory directions and exact upstream
  routes. They are included by the existing catalog notice resource glob. This
  provides a concrete local source route for the reviewed unmodified MPL files.
  [MPL sections 3.1–3.2](https://www.mozilla.org/en-US/MPL/2.0/).
- Managed-engine notices/source delivery remain owned by E01; PrivateBin's optional
  Chromium/Playwright image needs its separate finished distribution review. This
  launcher review does not override either runtime distribution boundary.

## Local checks and retained artifact inspection

`python scripts/collect-rust-notices.py --check` reproduces the committed notice
inventory/text from existing exact source cache material; it performs no build,
lock update or package download. `python scripts/check-packaged-notices.py`
passes with **39** configured notice/source files; its [receipt](windows-notice-configuration-2026-10-03.json)
records exact hashes and all five MPL lock matches.

The retained `dist/local-preview/Local Store_0.5.0-1_x64-setup.exe` is
**16,017,533 bytes**, SHA-256
`5e1a2aa847f1b97845582e8aa433a563edbbfe1e8b7ea865c8f4648226be2c75`,
matching its existing local checksum file. Native signature inspection reports
`NotSigned`; unsigned private testing is allowed. It was not installed, launched
or treated as a current product candidate.

The retained `target/x86_64-pc-windows-msvc/release` resource tree has only the
old Inter OFL in `src/fonts`. Comparing it with current notice resources correctly
fails: it predates the current notices/source bundle, CapRover/root/font additions
and updated notice document. [Exact differences](windows-retained-notice-inspection-2026-10-03.json)
are preserved. This staged-tree inspection is not extraction or verification of
the NSIS archive. Current configuration fixes will require a new local rebuild
and installed-resource verification when the current native candidate is ready.

## Current private bundle follow-up (4 October 2026)

After production sources stabilized, the current launcher and MCP sidecar were
built together with the locked release/custom-protocol feature set. The MCP-only
bundle overlay produced a private unsigned NSIS installer through Tauri's
bundle-only command. The installer is **18,307,828 bytes**, SHA-256
`5aa36857e5480d53a55826b8a98165341a2a1048da2e318a187de8318d227972`;
native signature inspection reports `NotSigned`. No installer was executed,
installed or published.

The launcher returns version `0.5.0-1` and differs from the actual MCP sidecar.
All **39 current notice/source resources** pass exact-byte staged inspection.
The generated NSIS script maps every one to its correct source/destination and
maps the launcher and current sidecar separately. None of the managed-engine
rootfs or source/notice companion archives is included. This is staged-resource
and generated-script inspection, **not extraction of the NSIS archive or
verification of an installed application**.

The [bundle receipt](windows-private-bundle-2026-10-04.json) binds binary, installer
and NSIS-script checksums to a canonical inventory of **2,119 current worktree
build/resource inputs**, which remained unchanged during build. The [notice
receipt](windows-private-bundle-notices-2026-10-04.json) records all exact resource
hashes. The recorded base commit is contextual: the build includes the current
uncommitted implementation and is not represented as a build of the old base
commit. Owner-modified changelog and pnpm lock are excluded from build identity.

A source-export check subsequently identified a line-ending-dependent Cargo.lock
notice digest. The inventory now explicitly hashes UTF-8 text with LF line endings;
exact package versions and source archive checksums remain unchanged. A single
bundle-only refresh included that corrected resource without rebuilding Rust.
The retained staged inventory was synchronized from its exact NSIS source because
bundle-only does not refresh Cargo's older staged resource copies. All 39 staged
files and generated NSIS mappings passed again. The initial installer and receipts
are preserved privately; the receipt distinguishes initial compiled-source
identity from refreshed resource identity.

## Exact remaining acceptance gates

1. Current private launcher/sidecar build, unsigned bundle, staged notice bytes
   and generated NSIS mappings now pass as described above. Archive extraction,
   installed resources and native application acceptance remain unverified; this
   local packaging receipt does not certify the older retained installer.
2. R04 needs a supported clean Windows host for install/first launch, prerequisite
   refusal/failure recovery, engine bootstrap, native windows, protocol/shortcut,
   agent flow and uninstall/data preservation. The owner has no additional clean
   PC available; no fresh-host claim is made from this existing host's receipts.
3. For an eventual public Windows artifact, download that exact published artifact,
   verify source/workflow/identity/checksums and applicable provenance, inspect
   notices/source access, and repeat its native acceptance. No public Windows
   binary exists for the completed source-only V1 milestone. Signing/updater
   requirements remain deferred and do not block private unsigned testing.

These are unfinished R05/R04 product gates, separately recorded from the completed
local review. No signing keys, updater endpoint, public installer publication,
new host setup or additional runtime qualification was requested by this review.
