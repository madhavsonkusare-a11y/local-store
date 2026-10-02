# Untrusted content and recovery boundaries

The broker treats app content as data. It does not run a language-model prompt,
execute returned instructions, infer consent or expose an owner permission tool
through MCP. Typed tool dispatch checks the authenticated client, exact installed
app, action and expiry. Each write binds to one exact owner-approved request;
changing arguments, replay, another client or an old connection cannot reuse it.

Actual owned-engine evidence includes:

- WordPress's private synthetic post requests another app's credential, a new
  permission, writes and an external URL. Its bounded text is returned exactly;
  cross-app reads, extra URL/method arguments, write grants and the purported
  owner-grant MCP tool are refused. The audit contains no private post/token.
- Memos's malicious-looking note content requires exact explicit approval and
  cannot turn a read scope into a write. Cross-client and approval replay fail.
- PrivateBin's untrusted paste is encrypted/decrypted by a non-root sandboxed
  browser with no network. Only the app broker can perform its fixed HTTP
  operation after approval; arbitrary browser, historical paste and destructive
  actions are refused. Fragment keys are not returned to the agent.
- n8n's official MCP output is projected to reviewed metadata/notes. Workflow
  parameters, credentials, arbitrary tools and execution are not forwarded.
- Live revocation, replacement and disconnect/reconnect deny earlier scopes.
  Each actual receipt binds to current provider and test source bytes.

These are application enforcement tests. They do not prove that every external
model will ignore malicious text. An agent with unrestricted access to the same
Windows account can bypass the broker using local files or the engine directly.
Settings displays that limitation next to connection and content permissions.

Recovery is bounded separately. The real protected-backup proof restores exact
Memos and Gitea private content and credentials into distinct fresh projects,
including owned named volumes. Sources are quiesced, resumed and left unchanged;
shared folders and occupied targets are refused. This proves consistent local
backup primitives for those storage cases, not automatic undo of every action,
version rollback, email, external service changes or other remote side effects.
Settings displays these limits before app write requests.

Evidence: `wordpress-agent-api-content-2026-10-02.json`,
`memos-agent-content-2026-10-01.json`, `privatebin-agent-content-2026-10-02.json`,
`n8n-agent-content-2026-10-02.json` and
`windows-backup-restore-2026-10-01.json`. The strict provider checker verifies
current-source receipt hashes; the native Windows and clean-host gates remain
separate.
