# Consistent Windows app snapshots

The backend owner API is `backup::snapshot_with` and
`backup::restore_into_fresh_with`. Neither is an agent tool, general file-access
API, universal undo feature or app-version rollback promise. These primitives
use the selected Local Store engine; Docker Desktop is not required.

Capture requires explicit approval, the existing app operation lock, exact
reviewed Compose identity, the saved managed-engine binding and daemon identity,
and matching container ownership for every service. Mixed, paused or restarting
services are refused before mutation. A running app is stopped and its stopped
state rechecked before any data is read. Capture failure still attempts to resume
an app that was running; resume failure is surfaced as a recovery error.

The protected snapshot includes every reviewed bind directory, every declared
named volume, and `local-store-secrets.json` when present. Named volumes must be
the exact local Docker volumes owned by the Compose project, with default local
driver options, expected mountpoints and no running container users. Native tar
preserves database file ownership and permissions. A bounded Rust POSIX ustar
validator rejects traversal, duplicates, links, special files, extended path
headers and elevated modes before restoration. Shared host folders and templates
requiring user-provided setup answers remain unsupported; these apps are refused
instead of receiving incomplete backups.

Snapshot data is limited to 256 MiB, 4,096 entries per file/archive collection,
16 named volumes and 32 directory levels. Windows current-user DPAPI protects
64 KiB chunks, with a fresh archive nonce and each chunk's count/position bound
to its protection. No custom cipher, key file, machine-wide key or UI prompt is
used. Backups require the same Windows account and that account's DPAPI keys;
they are not portable disaster-recovery exports. The format verifies every
entry's SHA-256 and required storage coverage after decryption. See Microsoft's
[DPAPI protection contract](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
and [decryption integrity guidance](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata).

Restore requires a separate reviewed project, matching offering/plan/engine
identity, no containers, empty bind directories/volumes and no existing
credential file. It refuses overwriting existing content. Compose, registry and
engine bindings are not imported from the snapshot. Partial restore failures
leave only new target data for inspection; retry must use a fresh empty target.
The caller performs startup separately.

Named-volume tar files exist briefly in a unique staging directory under the
owned managed project and are removed after reading/encryption or extraction.
No plaintext archive is retained as the exported backup. Temporary plaintext
shares the app data's local-account threat boundary. Same-user unrestricted
shell/Docker access can still bypass these controls; these APIs are not an OS
security sandbox. Browser profiles, store-agent bearer credentials and external
effects such as sent messages or webhooks are outside this app-data snapshot.

Focused checks:

```powershell
cargo test --locked --release --lib backup:: -- --test-threads=1
```

Opt-in real proof, serial with all other engine qualifications:

```powershell
$env:LOCAL_STORE_RUN_MANAGED_QUALIFICATION = '1'
cargo test --locked --release --test managed_backup_restore -- --ignored --nocapture
```

The real proof creates private Memos content and a private Gitea repository with
an exact committed file, snapshots each while quiesced, restores into distinct
fresh projects (Gitea also uses different named volumes), checks exact generated
credential bytes, and reads the content through the existing app probes. It
also checks source resumption, occupied-target refusal and preserved bystanders.
Actual results are written to
`windows-backup-restore-2026-10-01.json`; test source or compilation alone does
not establish successful restoration.

The October 1 run passed both apps in 111.41 seconds. Memos restored 114,773
bytes of bind data/credentials; Gitea restored 61,440,085 bytes including its two
named volumes and credential file. Existing private-content probes passed
after both fresh restores. Nine focused backup tests also passed. This is
backend proof on the existing owned engine, with no fresh-Windows installer,
backup UI, browser-profile, app-upgrade or cross-account recovery claim.
