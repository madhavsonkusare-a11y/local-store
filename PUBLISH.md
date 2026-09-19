# Release checklist

The GitHub repository and release automation are already connected. A pushed
version tag triggers the Windows CI build and publishes its installer to the
matching GitHub Release. Builds depend on both quality and
the minimum-Rust job; a failed MSRV check blocks publication.

## Build targets and integrity metadata

| Active runner | Rust target | Intended artifact architecture |
| --- | --- | --- |
| `windows-2022` | `x86_64-pc-windows-msvc` | Windows x64 |

Runner architectures follow [GitHub's runner image list](https://github.com/actions/runner-images#available-images).
macOS and Linux staging support remains in the repository for later scope, but
V1 does not build, certify or publish those artifacts. The Linux release job is
only a platform-neutral coordinator for Windows provenance and publication.
Tests and Tauri builds use the explicit Windows target. Tauri CLI is pinned to
2.11.4. Confirm the Windows job on GitHub before claiming coverage.

`scripts/prepare-release.py` stages the platform installers and a standalone CLI
named `local-store-<target>` (plus `.exe` on Windows). It refuses a missing CLI,
missing installer, duplicate filenames, input paths outside the build root or
nonempty staging directory. Debug symbols and `.d` files are not release assets.

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
code signing and **not** an updater signature: Windows will still warn that the
publisher is unknown, and there is no update channel. Task 30 remains
outstanding and needs owner-generated keys — see below.

On Linux/macOS, download the matching checksum list and all its listed files
into one directory, then run `sha256sum --check SHA256SUMS-<target>.txt` (or
`shasum -a 256 --check ...` on macOS). On Windows, use `Get-FileHash -Algorithm
SHA256` to compare a downloaded artifact with its listed digest.

## Code signing and updates (required for V1)

**Required by the owner on September 12, 2026.** This supersedes the earlier
signing/updater deferral. No signing or updater implementation is claimed yet.
S01–S04 in [V1_TASKS.md](docs/V1_TASKS.md) define the release acceptance gates.
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

**Windows code signing** is separate and needs a certificate from a CA. Without
it, SmartScreen warns on every install; build provenance does not remove that
warning, because it answers a different question.

**Before any of this ships**, the update endpoint has to exist and be served
over HTTPS, and the update UI has to let somebody decline. An updater that
cannot be refused is a worse defect than no updater.

## Before tagging

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

## Publish and verify

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
