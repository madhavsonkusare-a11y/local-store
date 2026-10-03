# Independent retained engine review

Reviewed October 2, 2026. This completes an independent provenance and obligation
inventory check for the retained development payload. It does not approve engine
binary distribution or close the complete E01 release gate.

The selected archive is 572,798,976 bytes with SHA-256
`ea9358fb64636e1d60da85df2ae5fd87090db2246c591996196dbb29089e5b32`.
The [machine-readable review](engine-independent-provenance-2026-10-02.json)
anchors it to the committed October 1 payload receipt and current build-input
hashes. No Docker or WSL command was used for this review. Payload files were read
in memory, with bounded metadata reads, without extracting a filesystem onto
the Windows host.

## Measured provenance

| Retained material | Independent result |
| --- | --- |
| Rootfs inventory | All 138 package/version/architecture rows agree between the current lock, exported inventory and rootfs dpkg database |
| Ubuntu metadata | Eight InRelease signatures verified; 30 package indexes matched signed uncompressed SHA-256 values and lengths; the 39-member index inventory also includes its empty lock file |
| Ubuntu archives | All 44 `.deb` hashes and sizes matched verified signed package records; 42 match installed versions and two are unused newer bootstrap OpenSSL packages |
| Docker components | All five locked archive hashes and lengths matched the captured current official signed index |
| Ubuntu base | Official pinned manifest, config and layer digests verified; 91 installed packages have unchanged identities in its 92-package base inventory |

The Ubuntu chain follows the documented InRelease signature, package-index hash,
then binary-package hash sequence. Historical versions are checked against the
captured snapshot metadata. Source verification is a separate chain through
signed release metadata, a Sources index and source artifacts; this review does
not claim to have performed it. [Ubuntu archive verification](https://documentation.ubuntu.com/security/software-integrity/archive-verification/).

The reviewed Ubuntu primary anchors are
`F6ECB3762474EDA9D21B7022871920D1991BC93C` and
`790BC7277767219C42C86F933B4FE6ACC0B21F32`; every retained release used the former.
They are published by Ubuntu. [Ubuntu security FAQ](https://wiki.ubuntu.com/Security/FAQ).
Docker's key was obtained from its official package repository endpoint and its
primary fingerprint pinned to `9DC858229FC7DD38854AE2D88D81803C0EBFCD88`.
The captured October 2 signed Docker index corroborates retained package bytes;
it is not a build-time signed Docker receipt. [Docker Ubuntu setup](https://docs.docker.com/engine/install/ubuntu/).

The base image is pinned to
`sha256:a61567bd31828687156d735ea8eb01ba4e37636e225dd6a48ba94136a70d9d61`.
This verifies the official HTTPS origin and content graph, not an OCI signature
or upstream attestation. The three inherited packages absent at their exact
versions from the captured Ubuntu binary indexes are `base-files`, `libc-bin`
and `libc6`; their identities are checked against the pinned base layer instead.
No claim is made that every output filesystem byte reproduces that base or its
package archives. The provenance inventory accounts for package identities; a
complete reproducibility claim requires additional build/file comparison.

Three [offline boundary checks](engine-provenance-boundary-2026-10-02.json)
passed: genuine retained signed metadata was accepted, modifying its signed
cleartext was refused, and the genuine signature was refused when only a foreign
primary key was allowed. The checks reuse installed GnuPG and maintained LZ4 and
Zstandard decoding libraries; no custom cryptographic or compression parser was
introduced.

## Notice and corresponding-source gaps

Copyright files resolve for **134 of 138 installed packages**, including valid
Debian documentation symlinks. Their paths and hashes are recorded individually.
Four packages lack packaged copyright files: `docker-ce`, `docker-ce-cli`,
`docker-buildx-plugin` and `docker-compose-plugin`. Their retained documentation
only has changelogs. A package-level copyright file's presence is not proof that
all licenses or bundled dependency notices have been satisfied.

The rootfs includes 14 full common license texts, including GPL, LGPL, Apache and
MPL versions. The separate retained notices tar only includes `/usr/share/doc`,
so it is not a self-contained license bundle. The inventory has **93 provisional
source/version pairs**, derived from dpkg source fields and a binary-name fallback
where no Source field exists. Docker's fallback identities still need mapping to
the exact upstream and bundled dependency sources. The automated `License:`
hints are review aids, not comprehensive SPDX expressions or release decisions.

Before publishing this rootfs as a binary asset, E01 still needs:

1. Collect exact-version upstream licenses, attribution and applicable dependency
   notices for all five Docker components, then produce a complete readable
   notices bundle including referenced common license texts. Exact Buildx and
   Compose releases have Apache-2.0 root licenses, but those alone do not establish
   complete dependency notice coverage. [Buildx 0.37.1 license](https://raw.githubusercontent.com/docker/buildx/v0.37.1/LICENSE),
   [Compose 5.5.1 license](https://raw.githubusercontent.com/docker/compose/v5.5.1/LICENSE).
2. Review each package's actual license and exceptions, resolve the provisional
   source identities and implement the applicable source delivery obligations.
   Not every source identity requires source distribution. For GPLv3 software
   conveyed through a download, section 6 requires an applicable corresponding
   source route; equivalent network access must remain available with clear
   directions. Merely listing package names or generic upstream repositories
   does not demonstrate fulfillment. [GPLv3 section 6](https://gcc.gnu.org/onlinedocs/libstdc%2B%2B/manual/appendix_gpl.html).
3. Retain and verify the exact Ubuntu source artifacts and packaging/build
   material needed for the applicable obligations; preserve source access beside
   the selected payload. The current binary indexes and 44 `.deb` archives do not
   contain corresponding source. No source archive bundle or fulfillment receipt
   has been produced by this review.
4. Review the finished notices/source bundle against the exact payload hash and
   document retention and security-refresh ownership before distribution approval.
   Apache-2.0 redistribution includes providing the license and preserving
   applicable notices; upstream NOTICE requirements apply when such a file is
   supplied. [Apache-2.0 section 4](https://www.apache.org/licenses/LICENSE-2.0.txt).

These are rootfs binary redistribution gates. They do not require signing keys
for the project's source-only GitHub milestone. None is silently marked complete
by a successful package hash or signature check.

## Separate runtime evidence

The [fresh retained-payload import proof](windows-fresh-payload-2026-10-02.json)
actually booted this exact archive in an isolated WSL fixture on the existing
Windows host. Docker 29.8.0 and Compose 5.5.1 became ready in **28.84 seconds**.
The VHD file length was **683,671,552 bytes**. The fixture was unregistered after
exact ownership verification and native selected-engine state was unchanged.

This is direct import on a host with existing WSL prerequisites. It does not
prove clean Windows setup, the product's fixed-name transaction, WebView startup,
prerequisite failure recovery, a supported host floor or physical disk allocation.
Those acceptance limits remain separate from the provenance result.

## Reuse

```text
python scripts/verify-engine-provenance.py --fetch-docker --fetch-base
python scripts/verify-engine-provenance.py
python scripts/test_engine_provenance.py
```

The first command captures public signed metadata and pinned base objects into
the ignored review cache. Later runs repeat offline against those same captures.
Use installed `gpg`/`gpgv`, `python-lz4` and `zstandard`; on this host Git's GnuPG
was reused and `lz4==4.4.4` was installed only into the ignored review vendor
directory. No runtime dependency, rootfs byte, component lock or package lock was
changed. The review always leaves `distribution_approved` false.
