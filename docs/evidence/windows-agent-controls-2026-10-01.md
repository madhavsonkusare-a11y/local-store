# Windows owner controls for agent connections — October 1, 2026

The production launcher can inspect enrolled clients, exact app grants and the
latest 100 redacted gateway audit events; explicitly enroll or rotate a client;
export its MCP configuration once; revoke a connection; and grant/revoke status
or start/stop permissions for an exact installed app. No owner command is exposed
as an MCP tool. Every IPC entry checks the trusted launcher boundary.

Implementation: `src/agent_owner.rs`, owner snapshot accessors in
`src/agent_gateway.rs` and `src/agent_policy/store.rs`; the launcher command
registration and capability entries are in `src/main.rs`, `build.rs` and
`tauri.conf.json`.

## Security and recovery behavior

- Enrollment requires explicit consent and an installed MCP connector alongside
  the launcher. It grants no app access by default.
- The preview contains no credential. A separate explicit export returns a
  sensitive MCP configuration for manual placement in the chosen client's
  settings; it does not edit client configuration, detect/connect other clients
  or automatically copy secrets to the clipboard.
- Exports expire after ten minutes, are one-use and are checked against the
  current credential before disclosure. Pending secrets are memory-only and
  disappear on launcher restart; persisted connections remain visible and can
  be explicitly rotated or revoked.
- Rotation removes old grants before re-enrollment. A canceled stale preview
  cannot revoke a newer credential created by another owner process.
- Existing gateway checks enforce enrolled identities, installed app membership,
  status expiry of 1–720 hours and lifecycle expiry of 1–24 hours. Grant changes
  require owner consent. Expired scopes and scopes for removed apps remain
  visibly marked in the owner snapshot.
- Audit/snapshot data contain no bearer credential. Configuration export is a
  sensitive owner-only response and must not be logged or persisted by the UI.
- The same-Windows-user limitation is included in the snapshot for display.

## Targeted verification

`cargo test --locked --release --lib agent_owner::tests -- --test-threads=1`
passed **5/5** tests on October 1, 2026 (42.80-second incremental compilation,
0.27-second execution). Tests cover consent and redaction, single-use export,
expiry/revocation/restart/cancel, missing connector with no enrollment side
effects, stale cancellation preserving a rotated token, rotation dropping old
grants and tokens, and honest expired/inaccessible app snapshots.

## Remaining checklist boundaries

This first owner-control slice initially offered status/discovery/start/stop.
The subsequent [mutation queue implementation](windows-agent-mutations-2026-10-01.md)
adds exact owner approvals for reviewed recipe installs and keep-data uninstall,
with pending IDs included in the production connection snapshot. The generic
grant controls still expose only status/lifecycle; mutation approval is one-use
and is not a remembered broad install permission. Subsequent real Memos content
and stdio mutation receipts now satisfy A02/A03; targeted owner consent,
revocation and accessibility checks satisfy A07. These do not prove universal
content access. Provider coverage remains the separate A04–A06 gates, and native
Windows interaction acceptance remains F04.
