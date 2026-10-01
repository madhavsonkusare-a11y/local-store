# V1 source release audit — October 1, 2026

Scope: the tracked source snapshot for `source-v1.0.0`, not a Windows installer
or a security audit of the installed applications. The owner authorized
completion and publication of the source release on October 1. The separate
Windows-product roadmap is unchanged by this audit.

## Source review

- Reviewed README, contributor instructions, publishing guidance, privacy and
  third-party notices against current code and the master ledger. Removed stale
  V3/100-app/signing requirements and the obsolete claim that explicit GitHub
  inspection makes no network request.
- Corrected the source-run command to enable `tauri/custom-protocol`, which
  embeds the committed frontend. Verified
  `cargo run --locked --release --features tauri/custom-protocol --bin local-store -- --version`:
  build succeeds and reports `local-store 0.5.0-1`. This is a build/CLI check;
  it is not clean-machine engine or complete WebView flow proof.
- Preserved CapRover's original Apache-2.0 license directly from the archive
  checked against `catalog/import-audit-sources.json`. Added its attribution.
  Existing catalog, font, icon and Rust dependency notices remain intact.
- Replaced all eight `NOASSERTION` gallery icons using the existing deterministic
  monogram generator. Icon refresh and offline validation now refuse undeclared
  artwork licenses. Coverage remains 1,678/1,678. The historical source lock and
  notice are retained, with no artwork from that source in the active manifest.
- Kept the owner's uncommitted `CHANGELOG.md` edit out of the release changes.

## Secret scan

Used [Gitleaks v8.30.1](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1)
for a Git archive of the staged source. The Windows x64 scanner ZIP was verified
against GitHub's published SHA-256 digest:
`d29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e`.
Scans enable nested-archive inspection, retain all default rules and redact
reports. Reports and downloaded audit tools are outside the source repository.

The initial 88 findings were reviewed: 85 public pinned commit hashes, two
instances of the `CAP_N8N_DIAGNOSTICS_ENABLED` field name, and one documented
shared upstream default credential. Four literal upstream-default examples
were redacted from the historical study. The checked-in `.gitleaks.toml` allows
only the exact public hashes/metadata fields at their specific source paths;
it does not disable a detection rule or skip a source directory.

The candidate source scan passes with zero unexcluded findings. An inert
GitHub-token fixture at the same candidate metadata path remains detected,
proving that the exceptions do not exclude the whole file. The exact final
tagged snapshot is scanned again before publication. This scanner result is
bounded detection evidence, not a guarantee that no possible secret exists.

## Targeted validation

| Check | Result |
| --- | --- |
| `python scripts/test_catalog.py` | 12 tests passed, including undeclared-license refusal |
| `python scripts/cache-catalog-icons.py --check` | All 1,678 local icons validated; 100% coverage |
| `python scripts/catalog_pipeline.py --check` | Generated catalog current; 1,678 entries and cached icons |
| `python scripts/check-licenses.py` | 512 locked Rust dependency crates accepted; five unmodified MPL-2.0 crates reported |
| `python scripts/check-v1-qualified.py --release-gate` | 10/10 selected task-qualified apps; zero remaining |
| Corrected source-run command with `--version` | Build and launcher identity passed |
| Changed Markdown local links and `git diff --cached --check` | Passed |

The source-only tag is outside the existing `v*` installer workflow trigger.
Publication uses the saved [release notes](../releases/source-v1.0.0.md).
After publication, the final verification record will identify the actual tag,
commit, generated ZIP/tar.gz inventory and release asset list. The post-release
completion update belongs on `main`; the published source tag stays immutable.
