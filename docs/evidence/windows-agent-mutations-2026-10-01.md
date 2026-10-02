# Exact agent mutation approvals — October 1, 2026

Implementation: `src/agent_requests.rs`, `src/agent_gateway.rs`,
`src/agent_gateway/auth.rs`, `src/agent_mcp.rs`, `src/agent_policy*`, and the
`begin_install_on_engine` runtime entry point. Native owner commands are
`agent_mutation_requests` and `agent_mutation_decide`; their capability and
handler registrations remain restricted to the launcher.

## Actual supported contract

Authenticated agent clients can request an install of one of the three compiled,
reviewed zero-input recipes (Memos, n8n, Uptime Kuma), or uninstall an installed
managed app while retaining its data. Imported apps requiring setup use the
existing owner installer instead; arbitrary GitHub definitions, shell commands,
setup answers, raw credentials and permanent data deletion are not accepted.
Requesting an uninstall additionally requires a live status grant for that app.

Requests bind the authenticated client and its current credential generation,
the exact recipe or installed-record/Compose fingerprint, the saved engine
binding, the daemon ID and (for the owned WSL engine) its ownership generation.
Only an explicit trusted owner decision can approve a pending request. Approval
expires after ten minutes and execution consumes it once. Fresh identity checks
and the existing app operation lock precede dispatch. Rotation, another client,
expired approval, recipe/installed identity drift and engine replacement cannot
reuse an approval. Owners can deny requests even after their client is revoked.

The versioned queue uses an exclusive cross-process sidecar lock and atomic
replacement. It is bounded to 256 retained records, 32 active requests and eight
active requests per client; file size is limited to 1 MiB. Pending requests
expire after 24 hours. Records contain approved identities, hashes and bounded
status metadata, not bearer values, prompts, setup answers or private content.
The credential-generation hash is internal and absent from agent/owner responses.

Approved execution returns a running request immediately, leaving stdio
responsive for polling and cancellation. Installs reuse the existing cancellable
runtime and rollback transaction, with an exact approved engine rather than
ambient discovery. A monitor checks cancellation, credential revocation and a
15-minute execution deadline; the existing registry commit remains the
cancellation cutoff. A late cancellation cannot falsely mark a registered app
as canceled. Keep-data uninstall retains the operation lock through registry
removal, never requests volume deletion and never removes app data folders.
Started uninstall is short and cannot be canceled; its pending request can.

MCP tools: `local_store_request_install`, `local_store_request_uninstall`,
`local_store_execute_request`, `local_store_request_status` and
`local_store_cancel_request`. Each accepts exactly one `app_id` or `request_id`.
No client identity, approval boolean, grant, engine path or command argument is
accepted. Durable audit metadata precede mutation dispatch.

## Targeted verification and remaining proof

`cargo test --locked --release --lib agent_ -- --test-threads=1` passed
**38/38** targeted agent tests (49.03-second incremental compilation,
10.52-second execution). Five new request tests cover absent approval,
cross-client and rotated-generation denial, replay, expiry, recipe/engine drift,
owner denial, cancellation and the commit cutoff, queue locking/version/limits,
secret-free persistence, and real enrolled credential rechecks. The new MCP
handler test checks initialization, exact tool arguments and rejection of a
client-supplied approval. Prior owner/policy/auth tests also remain passing.

The opt-in actual Windows stdio/owned-engine run now **passes** (48.71 seconds),
recorded in [the mutation receipt](memos-agent-mutations-2026-10-01.json).
It installs the reviewed Memos recipe after owner approval, refuses consumed
approval replay, writes and reads an exact private memo, uninstalls with data
retained, reinstalls and reads that memo again, cancels a running request,
refuses revoked credentials in the existing session and cleans up only the
private fixture. Native ownership files are unchanged. Cancellation in this
run was observed before a reported container stage; this does not prove
cancellation during every individual install stage. The focused owner-control
browser checks use mocked IPC; native Windows interaction is a separate gate.

Status polling tolerates bounded queue contention, reauthenticating each retry.
The connector marks only known pre-dispatch queue/credential lock refusals as
safe to retry. Mutations are never retried on ambiguous transport failures.
Audit failure while claiming execution is saved as terminal failure under the
same held queue lock; no worker is started from a failed audit.

Normal stdio closure cancels outstanding install workers and waits for existing
rollback before process exit. Forcibly killing the connector can leave a running
queue record and retained setup; this implementation never replays that consumed
approval automatically. Inspect app state and existing launcher recovery before
requesting a fresh action. No automatic process-crash recovery is claimed here.
As with existing policy, this gateway is not an OS sandbox against an agent with
unrestricted same-Windows-account shell/filesystem access.
