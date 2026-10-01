# Release checklist

The [GitHub repository](https://github.com/madhavsonkusare-a11y/local-store)
is already public. The next V1 milestone is a **source-only GitHub Release**;
it does not publish an installer and needs no Windows code-signing certificate,
updater key or HTTPS update channel. The Windows-product and signed-update
checklists below remain for later distribution work.

## Current V1: source-only GitHub Release

The existing workflow matches `v*` tags and publishes a Windows installer.
Use a reviewed source-only tag such as `source-v1.0.0`, which does not match
that trigger. Do not push a `v1.0.0` tag for this milestone without changing
the workflow first.

Before tagging, review the exact commit for accidental secrets, the launcher
license and third-party notices, current build instructions and honest feature
limitations. Prepare release notes in a saved Markdown file. The master
[source-release gate](docs/V1_TASKS.md#source-only-v1-github-release-gate)
records the remaining checks. Publication needs owner authorization; the
October 1 request to complete this source release supplies it. Keep the exact
source commit, saved notes and tag name reviewable before publishing.

When the gate is complete and publication is authorized, the source-only
procedure is:

```bash
git tag -a source-v1.0.0 -m "Local Store V1 source release" <reviewed-commit>
git push origin source-v1.0.0
gh release create source-v1.0.0 --verify-tag --title "Local Store V1 source release" --notes-file <release-notes.md>
gh release view source-v1.0.0
```

Verify the release tag points to the reviewed commit, inspect the generated
source archives and notes, and confirm that no installer asset is attached.
Record the result and G04 completion in a follow-up commit on `main`, preserving
the already published tag. The [source audit](docs/evidence/source-release-audit-2026-10-01.md)
and [saved notes](docs/releases/source-v1.0.0.md) provide the preparation evidence.
This publishes source, not a claim that a fresh Windows computer can already
install and run every planned V1 feature.

## Later Windows binary distribution

A pushed `v*` version tag currently triggers the Windows CI build and
publishes its installer to the matching GitHub Release. Builds depend on both
quality and the minimum-Rust job; a failed MSRV check blocks publication.

## Build targets and integrity metadata

| Active runner | Rust target | Intended artifact architecture |
| --- | --- | --- |
| `windows-2022` | `x86_64-pc-windows-msvc` | Windows x64 |

Runner architectures follow [GitHub's runner image list](https://github.com/actions/runner-images#available-images).
macOS and Linux staging support remains in the repository for later scope, but
the later Windows distribution does not certify those platforms. The Linux release job is
only a platform-neutral coordinator for Windows provenance and publication.
Tests and Tauri builds use the explicit Windows target. Tauri CLI is pinned to
2.11.4. Confirm the Windows job on GitHub before claiming coverage.

`scripts/prepare-release.py` stages the platform installers and a standalone CLI
named `local-store-<target>` (plus `.exe` on Windows). The Windows job also
builds the read-only `local-store-mcp` sidecar into the installer and stages a
standalone target-named copy. Staging refuses a missing CLI, MCP executable or
installer, duplicate filenames, input paths outside the build root or a
nonempty output directory. Debug symbols and `.d` files are not release assets.

Each target also publishes `SHA256SUMS-<target>.txt` and `build-<target>.json`.
The JSON records version, source commit, target, workflow URL and file hashes;
the checksum list also covers the JSON. This is unsigned build metadata on its
own: it says what was built, not who built it.

The release workflow additionally attests every published file with
`actions/attest-build-provenance`, signed by the workflow's own OIDC identity.
Anyone can check an artifact against it:

```
gh attestation verify "Local Store_<version>_x64-setup.exe" --repo <owner>/<repo>
```

That proves which workflow, at which commit, produced the file. It is **not**
code signing and **not** an updater signature: Windows can warn that the
publisher is unknown, and there is no update channel. S02/S03 are deferred
to later Windows distribution and need owner-controlled keys then.

On Linux/macOS, download the matching checksum list and all its listed files
into one directory, then run `sha256sum --check SHA256SUMS-<target>.txt` (or
`shasum -a 256 --check ...` on macOS). On Windows, use `Get-FileHash -Algorithm
SHA256` to compare a downloaded artifact with its listed digest.

## Unsigned local Windows preview

The owner can build and use a local Windows preview without signing or updater
keys. It is separate from the source-only V1 GitHub Release.
Build the sidecar explicitly, then build the NSIS installer:

```powershell
cargo build --locked --release --target x86_64-pc-windows-msvc --features mcp-sidecar --bin local-store-mcp
New-Item -ItemType Directory -Force binaries | Out-Null
Copy-Item -LiteralPath "target/x86_64-pc-windows-msvc/release/local-store-mcp.exe" -Destination "binaries/local-store-mcp-x86_64-pc-windows-msvc.exe"
cargo tauri build --target x86_64-pc-windows-msvc --ci --config tauri.mcp-release.conf.json --bundles nsis
python scripts/check-local-windows-build.py --build-dir target/x86_64-pc-windows-msvc/release
```

The installer is under `target/x86_64-pc-windows-msvc/release/bundle/nsis/`.
The September 29 personal copy and its checksum are in the ignored
`dist/local-preview/` folder; rebuilding or cleaning `target/` does not remove
that copy.
The local preview currently needs Docker Desktop or an already configured
Local Store WSL engine; the reproducible bundled-engine payload, setup consent
and clean-machine bootstrap are still open work. Do not present this preview as
one-click setup on a fresh PC. Windows may warn because it is unsigned. The
post-build check runs the actual launcher with `--version` and refuses the
wrong Cargo binary, a regression found on September 29.

## Code signing and updates (later distribution)

The owner changed V1 to a source-only GitHub release on September 30, 2026.
S01–S04 in [V1_TASKS.md](docs/V1_TASKS.md) are deferred to a later signed,
automatic-update distribution. No signing or updater implementation is claimed.
Code signing, updater payload signatures and build provenance are distinct.
A signed artifact is not a guarantee that reputation-based warnings disappear.

This needs credentials nobody but the project owner can create, so it is
written down rather than done. An agent must not generate these keys: whoever
holds the private key is the release identity, and that has to be a person.

**Updater signing key.** Generate once, and never commit the private half:

```
cargo tauri signer generate -w ~/.tauri/local-store.key
```

Then add the public half to `tauri.conf.json` under `plugins.updater.pubkey`,
set `bundle.createUpdaterArtifacts` to `true`, and give the workflow
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as
repository secrets. Until `pubkey` is set, the updater plugin must stay out of
the build: a placeholder key would produce update artifacts nobody can verify,
which is worse than having none.

**Windows code signing** is separate and needs a certificate from a CA.
The launcher, bundled MCP sidecar and installer must all be verified as
signed. Without trusted signing, browser downloads can trigger SmartScreen warnings;
even a newly signed app may need time to build reputation. Build provenance
answers a different question.

**Before automatic updates ship**, the update endpoint has to exist and be served
over HTTPS, and the update UI has to let somebody decline. An updater that
cannot be refused is a worse defect than no updater.

## Later owner setup for Windows signing and updates

These are two different signatures. **Windows code signing** puts a verified
publisher on the EXE/installer. **Tauri updater signing** lets an installed
copy reject a tampered update. The HTTPS update address tells the installed
app where to check; it is not a signing service.

For the current Tauri NSIS/MSI build distributed through GitHub Releases:

1. Choose the legal publisher name that should appear in Windows. Apply for
   a publicly trusted Windows code-signing certificate from a certificate
   authority, preferably with a managed HSM/signing service that GitHub Actions
   can call. Microsoft currently limits Azure Artifact Signing public trust to
   organizations in the US/Canada/EU/UK and individuals in the US/Canada; if
   those rules exclude you, compare OV certificate providers such as DigiCert,
   Sectigo or GlobalSign. Verify eligibility and current price with the
   provider before paying. A self-signed development certificate will not
   satisfy the public-release gate. Do not send the certificate private key,
   HSM credentials or account recovery codes in chat.
2. For the update address, the simplest existing-hosting choice is this
   repository's public GitHub Releases. Each release can carry a Tauri
   `latest.json` plus the Windows installer and its `.sig` file. Tauri can
   check the HTTPS `releases/latest/download/latest.json` URL. We will
   implement and test the manifest generation, consent UI, invalid-signature
   rejection, failed-download behavior and staged updates before enabling it
   in a public build. No separate domain or update server is required for
   this choice.
3. When release plumbing is ready, generate the Tauri updater key pair once
   on a machine you control, back up its private half offline, and provide
   **only the public half** for `tauri.conf.json`. Put the private half and
   passphrase into restricted CI secrets. Losing the private half prevents
   future updates to existing installs.

The owner need only decide the publisher identity/certificate route and
whether GitHub Releases is acceptable as the update host. The project can
prepare the rest without receiving private keys. Tauri's
[updater documentation](https://v2.tauri.app/plugin/updater/),
[Windows signing guide](https://v2.tauri.app/distribute/sign/windows/),
[Microsoft signing comparison](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)
and [GitHub Releases guide](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases)
are the source for these choices.

## Before a later Windows binary tag

1. Update the version in both `Cargo.toml` and `tauri.conf.json`.
2. Confirm the working tree contains only the intended release changes:
   ```bash
   git status --short
   ```
3. Run the verification suite in [`docs/agent-handoff.md`](docs/agent-handoff.md)
   and review the required gates in [V1_TASKS.md](docs/V1_TASKS.md).
   Installer/native/container evidence is separate from unit-test success.
4. Commit the release changes to `main` when publication is authorized. The tag
   must be exactly `v` followed by the configured product version; staging fails
   if a version tag disagrees with `tauri.conf.json`.

## Publish and verify a later Windows binary release

```bash
TAG=vX.Y.Z
git tag "$TAG"
git push origin main
git push origin "$TAG"

# Watch the matching tag workflow, then confirm the release and installers.
RUN_ID=$(gh run list --workflow build.yml --branch "$TAG" --event push --limit 1 \
  --json databaseId --jq '.[0].databaseId')
test -n "$RUN_ID"
gh run watch "$RUN_ID" --exit-status
gh release view "$TAG"
```

Do not claim a release is shipped until the tag workflow succeeds and
`gh release view vX.Y.Z` shows the release assets.
