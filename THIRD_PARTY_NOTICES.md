# Third-party material

Local Store's original launcher code is MIT licensed. Imported catalog text,
artwork, fonts and interface icons retain their own licenses and copyright
notices; the root MIT license does not replace those terms.

## Catalog material

Local Store adapts and combines project descriptions and metadata, normalizes
categories and identities, and adds local capability information. Source files,
source revisions and transformation scripts are included in this repository.
Every generated project includes original listing URLs and upstream revisions.

| Attribution | License and preserved notices |
| --- | --- |
| awesome-selfhosted contributors, [structured data repository](https://github.com/awesome-selfhosted/awesome-selfhosted-data) | [CC-BY-SA-3.0](catalog/notices/awesome-selfhosted/LICENSE), [authors](catalog/notices/awesome-selfhosted/AUTHORS) |
| Runtipi app-store contributors, [repository](https://github.com/runtipi/runtipi-appstore) | [GPL-3.0](catalog/notices/runtipi/LICENSE) |
| IceWhale Technology and CasaOS/ZimaOS app-store contributors, [repository](https://github.com/IceWhaleTech/CasaOS-AppStore) | [Apache-2.0](catalog/notices/casaos/LICENSE) |
| Coolify contributors, [repository](https://github.com/coollabsio/coolify) | [Apache-2.0](catalog/notices/coolify/LICENSE) |
| CapRover one-click app contributors, [repository](https://github.com/caprover/one-click-apps) | [Apache-2.0](catalog/notices/caprover/LICENSE); source revision and archive digest are recorded in `catalog/import-audit-sources.json` |

Adapted awesome-selfhosted material remains available under CC-BY-SA-3.0;
adapted Runtipi material remains available under GPL-3.0. Consult each preserved
license for its terms. `catalog/sources.lock.json` records the exact revisions.
`catalog/legacy.json` retains the previous awesome-selfhosted-derived snapshot
for compatibility and attribution. App software-license labels describe the
upstream app, independently of these data licenses.

## Visual assets

- App artwork: [Homarr dashboard-icons contributors](https://github.com/homarr-labs/dashboard-icons),
  [Apache-2.0](src/assets/apps/LICENSE), and [Coolify contributors](https://github.com/coollabsio/coolify),
  [Apache-2.0](catalog/notices/coolify/LICENSE). Original artwork is redistributed without
  visual modification; the UI supplies an outer frame. The catalog icon manifest
  records source URLs, immutable revision and checksums. Existing featured app
  identities use Homarr's icon collection. Project names and logos
  remain the property of their respective owners; inclusion implies no endorsement.
- App artwork from an app's own repository: Paperclip's logo, Paperclip AI,
  [MIT](catalog/notices/paperclip/LICENSE), pinned in `catalog/icon-sources.lock.json`.
- The [Umbrel app gallery](https://github.com/getumbrel/umbrel-apps-gallery)
  remains a recorded research source with `NOASSERTION`. Its eight formerly
  imported icons were replaced with Local Store monograms before the V1 source
  release. No artwork from that source remains in the current icon manifest.
  [The historical notice](catalog/notices/umbrel-apps-gallery/NOTICE.md) is
  retained; the icon validator and refresh exclude undeclared licenses.
- Monogram icons for apps no source covers are drawn by Local Store from glyph
  outlines of Instrument Sans, The Instrument Sans Project Authors,
  [SIL Open Font License 1.1](src/fonts/InstrumentSans-OFL.txt).
  The font is also shipped in the V2 interface at weights 400, 500 and 600.
- Interface icons: Lucide Icons and Contributors, [ISC license](src/assets/LUCIDE-LICENSE).
- IBM Plex Mono font: IBM Plex Project Authors,
  [SIL Open Font License 1.1](src/fonts/IBMPlex-OFL.txt),
  shipped in the V2 interface at weights 400 and 500.
- Inter font: The Inter Project Authors, [SIL Open Font License 1.1](src/fonts/OFL.txt),
  [upstream source](https://github.com/rsms/inter).

## Rust dependencies

The locked Rust dependency graph declares SPDX licenses, and
`scripts/check-licenses.py` fails the build if any of them cannot be
redistributed under a permissive choice — an upstream bump introducing a GPL or
AGPL dependency stops the build rather than changing what may be shipped
unnoticed. The overwhelming majority are MIT and/or Apache-2.0.

Five are under the Mozilla Public License 2.0, which is file-level copyleft:
`cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`.
They are used unmodified. The exact five crate source archives are provided in
[`catalog/notices/rust/mpl-source/`](catalog/notices/rust/mpl-source/), with
checksums verified against `Cargo.lock` in [the source receipt](catalog/notices/rust/mpl-source.json).
Installed notice resources preserve that directory. The [dependency inventory](catalog/notices/rust/inventory.json)
also gives exact-version upstream source-download URLs. These source archives
remain MPL-2.0 covered source; the launcher MIT license does not replace their
terms. Changes to MPL-covered files must be made available under the MPL, and
executable-form distribution must inform recipients how to obtain that source.
[Mozilla MPL-2.0 sections 3.1–3.2](https://www.mozilla.org/en-US/MPL/2.0/).

Nineteen crates carrying Unicode data are under Unicode-3.0. No dependency is
under the GPL or AGPL.

Current installer configuration includes this document, the root MIT license,
all catalog notices, exact MPL source archives and all three font license texts.
A historical local installer does not inherit current configuration; run
`scripts/check-packaged-notices.py --resource-dir <staged-resource-root>`
after rebuilding and inspect the actual installed/downloaded artifact separately.
`Cargo.lock` and `package-lock.json` record exact versions; Node packages are
development-only and are not shipped. A generated SBOM for releases remains
planned work.

Native activation uses the official Tauri plugins `tauri-plugin-single-instance`
2.4.4 and `tauri-plugin-deep-link` 2.4.10, each licensed under Apache-2.0 OR MIT
by the Tauri Programme within The Commons Conservancy. Their exact transitive
dependencies are recorded in `Cargo.lock`; this notice does not replace the
locked dependency notice inventory.

## Locked dependency notice inventory

[`catalog/notices/rust/NOTICES.txt`](catalog/notices/rust/NOTICES.txt) preserves
license, copyright and NOTICE files from the exact cached crate versions. The
[inventory](catalog/notices/rust/inventory.json) records file hashes, declared
expressions and exact source routes for all 512 locked packages; this is an
all-target superset, not an assertion that every package enters the Windows binary.
Twelve Windows-target packages whose crate archives lack standalone notices have
[pinned upstream supplements](catalog/notices/rust/upstream-supplements.json),
checked against each archive's recorded source revision. Selectors source files
carry MPL-2.0 headers; its supplement supplies the standard MPL text.

29 packages outside the Windows target graph still lack standalone notices in
this all-target inventory. Those platform/source records are visible review
limits, not Windows runtime omissions or non-Windows redistribution approval.
The offline collector refuses any remaining missing Windows-target notice.
This inventory does not audit every bundled vendored subcomponent, prove a
compiled-binary SBOM, or replace upstream license conditions. SPDX choice checks
are a compatibility gate; retaining applicable copyright and NOTICE material
is a separate obligation. [Apache-2.0 section 4](https://www.apache.org/licenses/LICENSE-2.0).

## App images and optional runtime material

App software licenses in catalog/recipe/template metadata describe each upstream
app independently of imported definition licenses. The launcher distributes
definitions and pinned image locators; installation pulls images from their
upstream registries. The current source milestone includes no app-image or engine
binary asset. It does not authorize mirroring every app image or replacing each
image's bundled dependency notices with a single app license label. Source-available
offerings remain identified under their actual terms.

The managed engine has its own [provenance/source/notice review](docs/evidence/engine-provenance-review-2026-10-02.md).
Its binary redistribution approval is separate. The optional PrivateBin browser
provider has [runtime notices](providers/privatebin/THIRD_PARTY_NOTICES.md),
including an explicitly changed seccomp profile; the pinned Chromium/Playwright
image and its dependency notices need their own finished distribution review
before bundling that runtime. No engine/browser distribution approval is implied
by this launcher notice inventory.
