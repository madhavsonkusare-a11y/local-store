# Reusing app providers for the launch roster

> Historical research, indexed October 4, 2026. The project is paused. This study is not current release scope; use [V1_TASKS.md](../V1_TASKS.md). Recheck upstream facts before adopting its proposals.

Checked October 1, 2026. This document records implementation choices and
remaining provider work. It does not complete A04–A06.

The first production provider reuses Memos 0.30.0's documented authenticated
memo API. The gateway adds exact client/app scopes, protected credentials,
bounded replies, no redirects/proxies, and an owner-approved private-memo
write. It never exposes authenticated arbitrary HTTP. The real fixture proof
is [recorded separately](../evidence/memos-agent-content-2026-10-01.json).
The API source defines the private visibility and memo routes; its authentication
API supplies `/api/v1/auth/me` for checking the pasted token's account.
[Memo API](https://github.com/usememos/memos/blob/v0.30.0/proto/api/v1/memo_service.proto),
[authentication API](https://github.com/usememos/memos/blob/v0.30.0/proto/api/v1/auth_service.proto).

The fastest next official provider is the MCP server already shipped inside
the pinned n8n 2.37.10 deployment. It avoids packaging a third-party n8n MCP
server. Review the native tool schemas against that exact release and proxy
only compiled operations. Upstream workflow visibility is broader than our
per-client app grant, so connecting a token is not permission for arbitrary
workflow execution. The compiled proxy's first write instead creates a named
data table with one fixed text column, avoiding workflow code entirely.
Its shared approval binds the project and name. The current workflow creation
tool is `create_workflow_from_code`, requiring validated SDK code; creation,
execution, publication and deletion need separate review. The proxy remains
unverified until a real native-MCP task and denial/revocation proof pass.
[Official MCP connection guide](https://github.com/n8n-io/n8n-docs/blob/main/docs/connect/connect-to-n8n-mcp-server.md),
[tool reference](https://github.com/n8n-io/n8n-docs/blob/main/docs/connect/connect-to-n8n-mcp-server/mcp-server-tools-reference.md).

For the browser path, reuse the repository's locked Playwright 1.62.1 and a
matching pinned browser image. The official image alone does not establish
isolation: its default root user disables Chromium's sandbox. Use a separate
browser user and reviewed seccomp policy. Scope the container to a private
app-only network, with no Docker socket, host IPC, host gateway or arbitrary
host folders. A matching private profile volume needs explicit owner consent.
Intercept requests and navigation against the compiled app origin and block
downloads. Resource, timeout and output ceilings belong to the broker, rather
than trusting page instructions. No production WebView remote debugging is
needed. The existing PrivateBin browser proof is useful task logic, but its
host-run qualification browser is not the production isolated provider.
[Playwright container guidance](https://playwright.dev/docs/docker).

Expand typed providers by moving only the shared transport, protected token,
scoped policy and exact-argument approval machinery into a common kernel.
Keep authentication and response projections app-specific. Existing meaningful
task probes already provide reviewed request/response examples for Memos,
Flatnotes, Gitea, Kanboard, WordPress, Jellyfin and Immich. Uptime Kuma's
Socket.IO interface and PrivateBin's browser-held encryption key need their
actual protocol providers; routing them through generic REST does not solve
their access model. Prove one real task per declared provider and retain the
current unsupported state until that proof exists.
