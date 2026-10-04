# Exact retained engine source and notices companion

Reviewed October 3, 2026. This extends the
[October 2 provenance review](engine-provenance-review-2026-10-02.md) with actual
source and notice capture for retained rootfs SHA-256
`ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32`.
Distribution approval remains false. No payload, component/package lock,
Dockerfile, provider receipt or running engine was modified.

| Material | Measured completion |
| --- | --- |
| Ubuntu sources | All 89 exact source/version identities for 133 Ubuntu packages; 282 artifacts, 338,509,497 bytes |
| Ubuntu verification | Six signed InRelease captures and twelve Sources indexes across September 13 and September 7 snapshots; signed compressed and uncompressed hashes, then exact artifact SHA-256/length |
| Docker upstream | Nine commit-addressed source archives, 128,500,909 bytes; five components plus runc, tini and two observed Go runtime versions |
| Module source supplements | 120 exact ZIPs, 86,161,513 bytes; 115 linked Compose dependencies plus five additional module versions |
| Module verification | ZIP `h1` content hashes match exact upstream checksum inventories; Compose also matches retained binary module/version/hash metadata |
| Notices | 1,423 preserved files, 134 packaged copyright resolutions and all 14 common license texts included; standalone tar and readable full text |
| Binary source mapping | Ten retained executables inspected without execution using maintained pyelftools 0.32 |
| Boundary checks | Eight passed: altered digest/length, missing offline capture, file-size limit, aggregate byte budget, changed module hash, unsafe/duplicate ZIP paths and official empty Hash1 format |

The source policy conservatively collects every Ubuntu source, including material
whose permissive license may not require source distribution. It does not turn
license hints into SPDX/legal decisions. Source descriptions, upstream originals,
Ubuntu/Debian packaging changes and build material listed in signed Sources are
retained. Exact obsolete inherited base identities are resolved against the
September 7 snapshot rather than guessed from newer package versions. The Ubuntu
chain uses installed `gpgv`, reviewed Ubuntu trust anchors and Python's maintained
XZ decoder. It follows [Ubuntu archive verification](https://documentation.ubuntu.com/security/software-integrity/archive-verification/).

Upstream sources use official HTTPS GitHub repositories and commit-addressed
archives, with retained raw SHA-256/length and tag-to-commit captures. Observed
Docker/runc binary revisions agree with selected commits. This is not a signed
upstream-artifact attestation. Compose's exact upstream release lacks a vendor
tree, so the linked binary's 115 dependency hashes are checked against its exact
`go.sum` and source ZIPs are retained through the public Go proxy. Go's documented
[Hash1 algorithm](https://github.com/golang/mod/blob/master/sumdb/dirhash/hash.go)
uses SHA-256 supplied by Python's standard library. No Go checksum-database
signature is claimed. Missing notices in vendor trees are supplemented from
exact-version source ZIPs, with the specific exception below.

The [source manifest](engine-source-companion-2026-10-03.json),
[upstream notices](engine-upstream-notices-2026-10-03.json),
[module notices](engine-module-notices-2026-10-03.json),
[binary mappings](engine-binary-source-mapping-2026-10-03.json), and
[final companion receipt](engine-companion-bundle-2026-10-03.json) record origins,
hashes, lengths, identity mappings and unresolved material. Large artifacts remain
in ignored `.cache/engine-source-companion/`. The source companion is approximately
594 MiB and includes originals, signed source indexes/keyring, manifests, readable
notices, notice tar, source delivery instructions and verification scripts. The
standalone notices tar is 11,243,520 bytes; readable notices are 10,522,494 bytes.
Final receipt hashes are authoritative after regeneration.

Three distribution gates remain concrete:

1. The exact Moby extensions dependency
   `v0.0.0-20260828112943-dd7ba219688d` has no LICENSE/NOTICE/COPYING file or
   copyright/SPDX/license declaration in its checksum-verified source ZIP.
   Retained `dockerd` embeds this exact linked module identity. The absence is
   recorded; no Apache grant is inferred from Moby's separate root license.
   Authoritative upstream licensing for this source or a separately qualified
   engine selection is required. Locally prepared archives containing this source
   also must not be published while its grant is unresolved.
2. The static `docker-init` identifies tini 0.19.0 `git.de40ad0` and Ubuntu GCC
   13.3.0, but its exact embedded C-library source revision and relinking/build
   inputs are unestablished. Library source from a nearby Ubuntu version and
   historical packaging recipes are not an exact build receipt. Clarify the
   upstream build inputs before claiming its applicable source delivery is met.
3. Public equivalent access to the finished notices/applicable corresponding
   source must accompany the selected binary, with authenticated release metadata
   and continuing retention ownership. Local cache retention prepares that route
   but does not publish it. The package command and receipts preserve these gates
   and never mark distribution approved.

[Source delivery and maintenance instructions](../../engine/SOURCE_DELIVERY.md)
provide capture/offline verification commands, source use/build directions,
retention ownership and security-refresh requirements. Checks completed entirely
without WSL, Docker or binary execution. A clean Windows machine, supported-host
matrix, installer delivery and signing remain separate external release work.
