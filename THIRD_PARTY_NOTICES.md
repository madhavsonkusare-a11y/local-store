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
- App artwork from the [Umbrel app gallery](https://github.com/getumbrel/umbrel-apps-gallery):
  eight icons. That repository declares no licence, so they are recorded as
  `NOASSERTION`; see [its notice](catalog/notices/umbrel-apps-gallery/NOTICE.md) before any
  broad redistribution.
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

Local Store links 495 Rust crates. Every one declares an SPDX license, and
`scripts/check-licenses.py` fails the build if any of them cannot be
redistributed under a permissive choice — an upstream bump introducing a GPL or
AGPL dependency stops the build rather than changing what may be shipped
unnoticed. The overwhelming majority are MIT and/or Apache-2.0.

Five are under the Mozilla Public License 2.0, which is file-level copyleft:
`cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext` and `selectors`.
They are used unmodified. Their source is available from crates.io at the exact
versions recorded in `Cargo.lock`; if a future change modifies any MPL-licensed
file, that file's source must be published under the MPL.

Nineteen crates carrying Unicode data are under Unicode-3.0. No dependency is
under the GPL or AGPL.

Installer resources include this document and the referenced license texts.
`Cargo.lock` and `package-lock.json` record exact versions; Node packages are
development-only and are not shipped. A generated SBOM for releases remains
planned work.

Native activation uses the official Tauri plugins `tauri-plugin-single-instance`
2.4.4 and `tauri-plugin-deep-link` 2.4.10, each licensed under Apache-2.0 OR MIT
by the Tauri Programme within The Commons Conservancy. Their exact transitive
dependencies are recorded in `Cargo.lock`; this notice does not replace the
planned full dependency-license inventory.
