# Engine notices and source delivery

The October 3, 2026 companion is tied to rootfs SHA-256
`ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32`.
It is prepared locally; engine binary distribution remains unapproved.

## What is retained

All 89 exact Ubuntu source/version identities for the 133 Ubuntu binary packages
are resolved through Ubuntu-signed InRelease metadata and Sources indexes from
the September 13 and September 7 snapshots. The companion retains all 282 source
artifacts (338,509,497 bytes): upstream originals, Debian/Ubuntu packaging changes,
`.dsc` descriptions, and additional artifacts listed by signed Sources. Collecting
every Ubuntu source conservatively covers permissive packages too; no automated
license hint decides whether a package requires corresponding source.

Nine commit-addressed upstream archives cover the five Docker components, runc
1.5.1, tini 0.19.0 and the observed Go runtimes 1.26.8/1.26.6. Docker executable
revision metadata agrees with the selected exact upstream commits. Their root and
vendored license, copyright, NOTICE, COPYING and PATENTS files are preserved.
Compose's release does not vendor dependencies: 115 exact binary-linked module
sources are retained, plus five additional module/version sources needed to
supplement missing vendor notices. Every ZIP's Go `h1` content hash matches the
exact upstream checksum inventory; Compose hashes also match its retained binary.
These HTTPS/content checks do not claim upstream artifact signatures or a Go
checksum-database signature.

The standalone notices companion includes the original 134 package copyright
resolutions, all 14 full common license texts, all collected upstream notices and
the exact module notices. The readable `NOTICES.txt` preserves each text with its
source path. Package documentation symlink resolutions remain recorded in the
independent provenance inventory; the notices bundle uses their canonical targets.
A notice inventory preserves supplied material, and is not a legal certification.

## Recreate and verify locally

Use Python 3.13, installed GnuPG `gpgv`/`gpg`, and pyelftools 0.32. Git for Windows'
existing GnuPG is supported. Install pyelftools into the ignored review cache:

```text
python -m pip install --no-deps --target .cache/engine-source-companion/vendor pyelftools==0.32
python scripts/build-engine-source-companion.py --fetch-indexes --plan-only
python scripts/build-engine-source-companion.py --fetch-sources
python scripts/collect-engine-upstream-notices.py --fetch
python scripts/collect-engine-module-notices.py --fetch
python scripts/inspect-engine-source-mapping.py
python scripts/package-engine-source-companion.py
python scripts/test_engine_source_companion.py
```

First capture of Ubuntu indexes requires `--fetch-indexes`. All fetch commands use
public upstream services and bounded reads. The Ubuntu source plan is checked
before downloading; module archives have a concurrent aggregate 550 MiB bound,
and upstream archives have a 100 MiB per-file bound. No Docker, WSL, paid service,
installer signing, or binary execution is needed. Captured objects and companions
are stored under `.cache/engine-source-companion/`, which is ignored by Git.
Public receipts in `docs/evidence/engine-*-2026-10-03.json` include exact origins,
lengths, digests, source identities and the payload anchor.

To repeat verification offline, omit all `--fetch*` flags and omit `--plan-only`,
then run the same inspection, packaging and boundary checks. Missing or changed
retained source artifacts fail. A genuine cached ZIP is checked by its Go content
hash rather than by its compressed-file encoding. Public tar receipts also record
raw SHA-256 and lengths. The final package command refuses missing Ubuntu sources
or a mismatched input artifact. It preserves explicitly recorded upstream gaps.

## Using the source artifacts

The `engine-source-companion.tar` contains `MANIFEST.json` with exact downloadable
URLs and hashes, source archives, signed source-index metadata and verification
scripts. It also contains `SOURCE_DELIVERY.md`, readable `NOTICES.txt`, and
`engine-notices.tar`. The bundled scripts document the repository workflow and expect its retained
rootfs/cache layout; they are not a standalone source-only rebuild service.
Sources are preserved as supplied by upstream; the tools do
not unpack a Linux filesystem onto Windows. To work with an Ubuntu source package,
use an isolated Linux development directory and run `dpkg-source -x <package.dsc>`
with its sibling originals and packaging tar/diff files present. The `.dsc`
provides build dependencies; `debian/rules` and packaging patches form part of the
retained source. Docker/Go archives include their upstream Makefiles, Dockerfiles,
module checksum inventories and build documentation where supplied. These files
are source delivery material, not a claim that the rootfs has been reproduced
byte for byte or that Docker's package build environment has been recovered.

## Remaining distribution blockers

1. Exact `github.com/moby/extensions` version
   `v0.0.0-20260828112943-dd7ba219688d` is linked into retained `dockerd` and has no
   license/NOTICE/COPYING file or copyright/SPDX/license declaration in its exact
   source archive. Moby's Apache root license does not establish this dependency's
   grant. Obtain authoritative upstream licensing for that exact source or select
   and qualify an engine release with a verified grant. The current payload is
   preserved unchanged; no licensing claim is invented.
2. `docker-init` is statically linked, identifies tini 0.19.0 git.de40ad0 and an
   Ubuntu GCC 13.3.0 compiler, and does not identify its exact embedded C library
   source revision. Exact library/build and relinking material must be established
   before declaring its applicable source obligations fulfilled. A nearby Ubuntu
   libc source or historical packaging commit is not proof of its build inputs.
3. Before conveying any engine binary, publish the finished notices and verified
   applicable source material beside that exact binary, with clear equivalent
   download directions. Local cache files and generic upstream links are not a
   public source delivery route. The release owner must retain the companion and
   verify continuing access for the applicable license period; match both payload
   and companion digests in authenticated release metadata. No publication has
   occurred in this task.

Local Store maintainers own notice/source retention, upstream license clarification
and security refreshes. For each refreshed payload, recapture exact source
identities and notices, qualify the engine and regenerate the companion. Do not
reuse this payload's receipt for another binary. Keep old companions accessible
for conveyed versions while applicable obligations persist. Runtime qualification,
a supported Windows host matrix and clean Windows acceptance remain separate.

Ubuntu's signed source chain follows its [archive verification documentation](https://documentation.ubuntu.com/security/software-integrity/archive-verification/).
Apache redistribution obligations are defined in [Apache-2.0 section 4](https://www.apache.org/licenses/LICENSE-2.0.txt),
GPL source delivery in [GPLv3 section 6](https://www.gnu.org/licenses/gpl-3.0.html),
and library relinking requirements in [LGPL-2.1 section 6](https://www.gnu.org/licenses/old-licenses/lgpl-2.1.html).
These references describe the review route; they do not approve this binary.
