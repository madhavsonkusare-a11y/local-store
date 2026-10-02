# Managed Windows launcher recovery check

The opt-in test [managed_launcher_recovery.rs](../../tests/managed_launcher_recovery.rs)
calls the same locked owner recovery functions used by launcher IPC. The actual
October 1 run passed in 28.18 seconds with the Windows release configuration;
[its receipt](windows-launcher-recovery-2026-10-01.json) records the real steps.

Run only after the serial app qualification batch finishes and the native Local
Store engine has been explicitly selected. The test uses the previously verified
`.cache/engine/real-wsl-proof/state` ownership journal and the selected native
engine. It does not start Docker Desktop, import another distro or move its disk.

```powershell
$env:LOCAL_STORE_RUN_MANAGED_QUALIFICATION = '1'
cargo test --locked --release --features tauri/custom-protocol,mcp-sidecar --test managed_launcher_recovery -- --ignored --nocapture
```

The test acquires the same cross-process `qualification.lock` as the managed app
qualification harness and refuses to run while that slot is busy. It uses a
private APPDATA configuration root, restores the original environment, and
refuses installation if the engine already contains the normal Memos project,
its reserved container name or project volumes/networks. Do not install another
Memos app on the same daemon while this opt-in check runs.

The check exercises these real outcomes:

- Install the reviewed Memos recipe on the owned engine, create its first admin,
  write a private memo and read that exact content.
- Preserve viewed onboarding steps, then move only the isolated fixture's live
  and previous registry files aside to simulate registry loss.
- Inspect real retained setup ownership and project the resulting launcher
  recovery state; adopt through the existing owner action and read the exact
  memo again at the unchanged address.
- Uninstall while preserving data; verify the real no-container recovery state,
  fingerprint the stopped data files, then discard with `delete_data: false`.
  Check that every data file, Compose setup and engine binding remains intact.
- Re-adopt the retained setup and read the same private memo again. Remove only
  the verified fixture, then compare all other containers' IDs and states with
  the initial snapshot.

Keep-data discard deliberately preserves the setup files as well as data; it
does not remove the recovery candidate or claim to delete metadata. Cleanup
reuses exact Compose-path ownership verification instead of a broad label-only
resource deletion.

Execution writes
`docs/evidence/windows-launcher-recovery-2026-10-01.json`, including actual step
results and test/content-probe hashes. Failed cleanup or a changed bystander
container makes the result fail. No password, bearer token or private memo
content is copied into that receipt.

This is backend owner-action and launcher-projection parity on the development
host. It does not certify a native Windows WebView flow, fresh-machine engine
setup, sleep/wake behavior or all supported Windows versions.
