# Catalog contribution and refresh guide

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

The integration baseline contains 1,678 discovery entries and local icons or
monograms for every entry. 52 apps are offered through reviewed recipes and
templates. Catalog presence is not installation or meaningful-task proof.
Use [V1_TASKS.md](V1_TASKS.md) for release targets, not discovery totals.

## Existing sources, adapted as data

| Upstream | Accepted source records | Material license |
| --- | ---: | --- |
| [awesome-selfhosted-data](https://github.com/awesome-selfhosted/awesome-selfhosted-data) | 1,342 | CC-BY-SA-3.0 |
| [Runtipi app store](https://github.com/runtipi/runtipi-appstore) | 270 | GPL-3.0 |
| [CasaOS / ZimaOS app store](https://github.com/IceWhaleTech/CasaOS-AppStore) | 173 | Apache-2.0 |
| [Coolify](https://github.com/coollabsio/coolify) | 367 | Apache-2.0 |

The original snapshot contributes aliases and eight projects absent from these
sources. Four CasaOS and four Coolify records were excluded for invalid or
incomplete metadata. The exact revisions, normalized input checksums, excluded
paths, merges and legacy aliases are committed under `catalog/`.

These are discovery imports. Upstream Compose YAML is parsed with PyYAML's safe
loader for metadata; it is never executed or used as a Local Store install recipe.
The recipe allowlist remains in Rust. Project software licenses in the UI are
separate from the licenses of the imported catalog material.

## Reproduce or refresh

Requires Python 3.10+, the pinned parser in `scripts/requirements-catalog.txt`,
and a full Git checkout. Generation records the last commit that changed each
first-party definition; ZIP/tar.gz source downloads and shallow clones cannot
supply that history. Source archives already include the generated catalog
and icons for building the launcher; do not regenerate them just to build.
For catalog contribution checks, clone the repository without `--depth` and
check out the desired source tag or branch.
Normal generation and validation do not use the network.

```sh
python -m pip install -r scripts/requirements-catalog.txt
python scripts/catalog_pipeline.py --check
python scripts/cache-catalog-icons.py --check
python scripts/test_catalog.py
```

To update sources, review and edit the immutable revisions and snapshot date in
`catalog/sources.lock.json`, then run:

```sh
python scripts/catalog_pipeline.py --refresh
python scripts/cache-catalog-icons.py --refresh
python scripts/catalog_pipeline.py
python scripts/catalog_pipeline.py --check
python scripts/cache-catalog-icons.py --check
```

The icon refresh reads pinned sources in `catalog/icon-sources.lock.json`;
change those pins explicitly to update artwork. Per-app choices belong in
`catalog/icon-overrides.json`. Coolify artwork follows its
existing pin in `catalog/sources.lock.json`, matched through each project's
listing provenance. Reviewed Homarr name differences belong in
`catalog/icon-aliases.json`; a full asset path can select a visually reviewed
theme variant when upstream suffixes are inconsistent. Fuzzy matching is avoided.
Existing valid artwork at the same pin is preserved. Initial discovery of the
Homarr pin is used only when creating a new manifest. Review diffs before merging.
An interrupted source refresh leaves the embedded last-good catalog untouched;
finish the refresh or restore the source-input changes before committing.

Source adapters live in `scripts/catalog_pipeline.py`. Archives are read without
extracting files, with bounded archives/manifests. Large upstream collections
use a pinned GitHub tree and bounded individual manifests. Local download caches
are disposable and excluded from Git. No refresh runs when the app starts.

## Identity and metadata

Canonical repository/homepage URLs join related records. Documentation-host
joins also require matching normalized names. Names alone never merge projects.
Reviewed wrapper, rename and URL corrections belong in `catalog/overrides.json`.
Variants retain their original source paths in provenance. They do not inflate
the project count or become installation options without recipe review.

`catalog/ids.json` freezes identity-to-ID assignments across name changes.
Do not regenerate it from scratch. A split involving an existing ID fails until
an explicit migration is supplied. `catalog/import-report.json` preserves old
display-name aliases, including explicit nulls for invalid retired entries.
New connected and managed apps store the canonical catalog ID; older names still
resolve through aliases without rewriting a user's registry on startup.

The generated schema includes source/website URLs, description, aliases, category,
tags, project licenses, container architectures, source snapshots and local icons.
The build and runtime share Rust data types; builds reject malformed schema,
duplicate IDs, missing provenance and invalid/missing bundled icons.

`tracked` means present in a pinned current input; it does not mean healthy,
actively maintained or tested by Local Store. `snapshot_only` means retained only
from the old catalog. An upstream update date is metadata, not a local test date.
Architecture filters describe deployment metadata, not desktop installers.

The three original recipes have separate [schema-3 recipe requirements](recipe-requirements.md)
with cached upstream image-platform evidence. Discovery architecture labels do
not automatically become install requirements or graduate a catalog entry.

## Browsing and artwork

`browse_first` in the overrides is an editorial introduction spanning finance,
photos, documents, feeds, dashboards and development. Remaining projects sort
by name and ID. This order does not claim popularity or install readiness.
Interest collections match category/tag terms in `src/catalog_index.rs`; they
can be combined with search, category, license, architecture and capability.
The Web apps filter includes install previews with a known web interface.

The icon importer reuses [Homarr dashboard icons](https://github.com/homarr-labs/dashboard-icons)
and artwork referenced by [Coolify templates](https://github.com/coollabsio/coolify).
Requests require HTTPS on two explicit GitHub hosts, reject redirects, time out
after 20 seconds and cap files at 512 KiB. XML validation rejects declarations,
active elements, event handlers and external references. SVGs that fail validation
can use the same upstream identity's PNG instead. Pillow verifies and decodes
PNG files, rejects animation, and bounds dimensions to 4,096 pixels per side.
SHA-256 binds each committed file to the manifest. Missing/rejected artwork uses
a monogram; the manifest records the reason. New Homarr matches prefer light
variants for the dark interface where available.
Rendering accepts local paths only, and the launcher CSP blocks remote images.

This build-time cache replaces the originally planned runtime Rust downloader:
every shipped icon works offline, with no browsing-time image requests. Custom
user-supplied icon caching is future work. All upstream material and notices are
listed in [third-party notices](../THIRD_PARTY_NOTICES.md).
