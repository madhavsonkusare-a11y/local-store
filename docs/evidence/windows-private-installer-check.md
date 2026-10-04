# Actual private Windows installer inspection

Reviewed October 4, 2026. The private NSIS installer stays unsigned, unexecuted
and unpublished. This checks the bytes actually stored in the finished archive;
it does not install the application or run its uninstaller.

The [actual receipt](windows-private-installer-extraction-2026-10-04-051651.json)
records 50 extracted files, 41 configured resources and 39 exact notice files.
Every configured resource matches its repository source, including the exact
Cargo.lock-derived notice inventory and five MPL source archives. The connector
matches its retained binary. The launcher differs by exactly Tauri's three-byte
bundle-type marker; the entire remaining binary matches, rather than merely a
version string or selected PE sections.

## Reproduction

Use the existing verified owned WSL engine, without importing/replacing it.
Obtain the official [7-Zip 26.03 Linux x64 portable archive](https://github.com/ip7z/7zip/releases/download/26.03/7z2603-linux-x64.tar.xz),
linked by the [upstream download page](https://www.7-zip.org/download.html).
Keep it at `.cache/installer-extraction-2026-10-04/7z2603-linux-x64.tar.xz`
and extract only its regular `7zzs` member beside it. The archive SHA-256 is
`dc99eff5008f1ab79bd7084c68513701547a808a89502bf4133683535ab3c695`;
the reader SHA-256 is
`eab4c8d7f193e3d6d3237370bbcaa879a160a3f1dc82202207e27baeab79b6ac`.
The package digest agrees with official GitHub release asset metadata. This
is an HTTPS/content identity check, not an upstream signature claim. No tool is
installed on Windows or into the engine's package filesystem.

Run `python scripts/check-private-installer.py --run` from the repository.
The script requires exact tool and retained installer hashes, validates native
ownership, lists archive names/types/sizes before extraction, refuses unsafe
paths/aliases or excessive counts/sizes, and creates a unique ignored extraction
directory. It verifies actual notice/resource/entrypoint bytes and unchanged
native engine identity. It retains review files, never recursively deletes user
paths, and creates a new receipt without overwriting prior results.

The bundler is `tauri-cli 2.11.5`. Its official
[patch implementation](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.5/crates/tauri-bundler/src/bundle.rs)
changes the unique `__TAURI_BUNDLE_TYPE_VAR_UNK` marker to
`__TAURI_BUNDLE_TYPE_VAR_NSS` for NSIS. The checker reproduces only that exact
equal-length transformation, then compares every extracted launcher byte.
The retained build launcher hash is `9c86749e7e0b243011d9f2b4c640fee2465ce6768c2a0b832ebc734a027eeb0c`;
the packaged launcher hash is `ff791afd0f93030930efec2a52897dc56479e572e1a6626e9229763cb382bb18`.
The older build receipt's staged source/mapping checks did not establish literal
launcher equality inside the installer; this receipt adds that distinction.

## Limits retained

7-Zip reconstructs NSIS helper/stub material. The generated uninstaller has no
listed size and is separately bounded after extraction. `StartMenu.dll` lists
13,316 bytes but extracts to 7,680 bytes; that discrepancy is retained in the
receipt. It does not exempt a product resource or entrypoint from exact equality.
Installed plugin/uninstaller behavior requires actual Windows acceptance; it is
not certified by this inspection.

No installer destination, shortcuts, registry, prerequisite download, first
launch or uninstall behavior was exercised. The archive contains the launcher
and connector, excluding the engine payload/source companions. It was not
downloaded from a public release. R05 remains PARTIAL and R04 remains BLOCKED
until those later delivery and clean-host checks have evidence.
