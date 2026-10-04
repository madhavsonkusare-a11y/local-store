# Agent access architecture

Updated October 4, 2026. Project paused. Accepted work lives in [V1_TASKS.md](../V1_TASKS.md), A01–A08; this reference describes the implemented boundaries rather than a second queue.

## Implemented gateway

`local-store-mcp` is an authenticated stdio bridge. The owner enrolls clients and explicitly configures the bearer; client identity comes from checked credentials, not caller metadata. Store lifecycle and bounded content operations reuse the existing runtime through persisted, expiring exact-client/app grants. Writes require a separate short-lived approval bound to the exact action and arguments. Revocation, replacement and disconnect invalidate earlier scopes; dispatch records redacted audit.

The gateway does not let the agent grant itself permissions, approve its own writes, export raw credentials, execute arbitrary shell/SQL/browser operations or treat returned app text as policy. Windows protects connected app credentials for the current user. Same-user unrestricted shell/process access is outside this application's isolation model.

The launcher supplies owner enrollment, connection, grants, approval and audit controls within approved V2. Browser behavior and IPC proof do not certify all native WebView flows. Private installer extraction inspected sidecar/resources, but clean-host installation remains unproven and the private artifacts were deleted during shelving.

## Four provider kinds, ten launch apps

| Provider | Apps | Demonstrated scope |
| --- | --- | --- |
| Typed app API | Memos, Gitea, WordPress, Kanboard, Immich, Jellyfin, Uptime Kuma | Reviewed fixed operations and redacted bounded projections; Memos also supports an exact approved private-note write |
| Official instance MCP | n8n | Reviewed workflow metadata and an approved fixed note-table operation; no generic MCP proxy or arbitrary workflow execution |
| Isolated browser | PrivateBin | Approved connection-created non-burning pastes and scoped reads; no signed-in production WebView debugging or arbitrary browsing |
| App files | Flatnotes | Read/list bounded ordinary Markdown notes; no hidden files, links, traversal, databases or writes |

Exact setup, operations and remaining limits are in [launch agent coverage](../evidence/launch-agent-coverage-2026-10-02.md). Most selected apps have read-only access. Useful access does not mean every feature, every app offering or arbitrary writes.

## Capability, connection and permission

These are distinct facts:

- A provider exists with matching, unexpired source/test/fixture receipts.
- The owner connected the current installed instance and completed its required authentication.
- This client currently has an applicable app/action grant.
- A mutation also has current exact-action approval.

`src/agent_provider_evidence.rs` withholds verified claims for missing, changed, failed or expired proof. `python scripts/check-agent-provider-evidence.py` checks the strict ten-provider gate. `--allow-pending` is a development report; it is not product acceptance. The accepted source release remains usable after receipts expire.

App-content fields are data. Fixed API requests do not follow user-supplied URLs or instructions. Browser/file providers enforce their own destination, type, size and scope rules. The [content boundary review](../evidence/agent-content-boundaries-2026-10-02.md) records actual denial, revocation and untrusted-content tests.

## Extending coverage later

Reuse a reviewed official API/MCP integration first, then a narrowly scoped existing browser or file adapter where needed. Record tools, authentication, grant/approval classes, private projections, exact task/restart behavior and denial/revocation evidence. Credentials never belong in catalog metadata or prompts.

Qualification of installation and qualification of agent access are separate. Do not add a decorative access manifest, expose all databases because they are local, or silently count management/status tools as useful content access. Wider app coverage is later work beyond the ten selected launch apps.

Protected backup/restore covers recorded owned data/credentials, including scoped named volumes; it is not universal undo. Shared folders, sent messages, payments, webhooks and other external effects retain their own boundaries. Use existing backup/recovery semantics instead of raw database-file edits.

The [dated integration survey](../research/agent-platform-and-differentiation.md) and [provider reuse study](../research/agent-provider-reuse-2026-10-01.md) remain research. Verify licenses and current upstream documentation before adopting new integrations.
