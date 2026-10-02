# n8n content access through its official MCP server

The reviewed n8n 2.37.10 offering has a narrowly scoped proxy for its own MCP server. The owner enables MCP inside n8n and explicitly supplies the app-issued token in Local Store Settings. Workflow detail reads also require the owner to expose the exact workflow in n8n. Local Store never changes that policy automatically. Local Store protects the token with user-bound Windows DPAPI and grants each enrolled agent access to one exact running managed app for at most 24 hours.

The gateway exposes workflow identity/node-type summaries, project identities and table schemas. It strips upstream tool hints, workflow parameters and credential references. The sole mutation creates one table with a fixed `note` text column in an explicitly selected project. A separate exact owner approval expires after at most two minutes and is consumed before the upstream mutation; uncertain mutations are never automatically retried. Arbitrary MCP tool names, workflow execution, code, URLs, activation, deletion and credential changes are unavailable.

The target includes the installed app's identity, reviewed pinned image, retained owned-engine binding, compose hash and daemon identity. A moved or replaced target cannot inherit a protected connection or approval. App text remains untrusted data and cannot change grants or approval.

Each successful connection replacement clears the exact app's prior client scopes and pending approvals. Disconnecting also revokes those scopes before removing the credential. Connecting again needs a fresh owner grant; it cannot revive an old reader, even if an older protected connection record is absent.

`tests/managed_n8n_content.rs` is an opt-in live proof using only a synthetic n8n owner in a disposable fixture. It checks metadata reads, exact fixed-table creation, read-scope write denial, absent owner approval, cross-client denial, one-use replay denial, restart persistence, token replacement, live revocation, audit redaction and unchanged bystanders. Compilation and UI mocks alone do not prove the live integration. The JSON receipt is written only by the actual fixture run and its `passed` field determines acceptance.

The first complete Windows run on 2 October 2026 passed in 180.23 seconds; its genuine receipt is preserved as `n8n-agent-content-pre-format-2026-10-02.json`. Final acceptance uses `n8n-agent-content-2026-10-02.json` only when its provider, shared content boundary and fixture SHA-256 values match current source. n8n can report a running container before its MCP route is ready after restart; the fixture waits at most 60 seconds and retries only the table read. It never retries a consumed write approval. Earlier failed receipts remain beside it to preserve the readiness failure and its correction.

The final formatted, permission-corrected Windows run passed in 127.37 seconds. Its current acceptance receipt matches all three source hashes and additionally proves that replacement clears prior read scope, a fresh owner grant restores exact reads, and disconnect/reconnect cannot revive the old reader.

The Settings checks in `tests/ui/agent-content-controls.spec.js` cover both provider choices, separate token consent, exact operation-specific approval, private token clearing and accessibility.

This n8n proof establishes no browser provider, arbitrary workflow editing, all-app coverage, packaged-engine provenance or native Windows WebView acceptance. Other implemented provider modules require their own live acceptance receipts. Existing host-browser app qualification scripts remain test fixtures.

Upstream schema references inspected on 2 October 2026:

- [Official n8n MCP setup](https://github.com/n8n-io/n8n-docs/blob/main/docs/connect/connect-to-n8n-mcp-server.md)
- [Official tool reference](https://github.com/n8n-io/n8n-docs/blob/main/docs/connect/connect-to-n8n-mcp-server/mcp-server-tools-reference.md)

The separate PrivateBin browser module uses a reviewed pinned non-root runtime, no browser network, a temporary isolated profile and fixed DOM actions. Its narrow HTTP broker operates only against the exact owned app. Its image, runner and real read/write/revocation proof are separately documented; this n8n receipt cannot prove them.
