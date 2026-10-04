> Research snapshot, not an approved implementation or pricing policy.

> Historical research, indexed October 4, 2026. The project is paused. This study is not current release scope; use [V1_TASKS.md](../V1_TASKS.md). Recheck upstream facts before adopting its proposals.
> External product, protocol, licensing, eligibility and pricing claims require
> fresh primary-source verification before implementation or commercial decisions.
> Current V1 scope and task status live in [V1_TASKS.md](../V1_TASKS.md).

# Agent platform and product differentiation

Proposal recorded September 12, 2026. **Nothing here is agreed product
direction and no code exists for any of it.** It answers two questions: how AI
agents could be given controlled access to the apps Local Store installs, and
which further features would make the product distinct and sellable.

Statements about other products, protocol revisions and libraries were checked
on September 11–12, 2026 and are sourced at the end. Statements about this
repository name the file they came from. Where something was not checked, it
says so.

## Where the product stands

| Product | Shape | Agent support | Notes |
| --- | --- | --- | --- |
| umbrelOS 2.0 | Home-server OS on separate hardware | MCP server for Claude Code, Codex, Cursor and OpenClaw: install apps, organize files, troubleshoot. Each agent gets its own revocable token | Full release September 22, 2026. Its new Mac and iPhone apps are clients for that server, not local runtimes |
| Cloudron | Server platform, 120+ apps | None published | Free for 2 apps, then $15/month. The pitch is per-app backups, automatic updates and single sign-on |
| Pinokio v8 | Desktop launcher for Windows, macOS and Linux | Runs agent applications; no platform MCP found | Runs install scripts on the host: no container isolation and no verification. Has LAN access, a per-app RAM/CPU/VRAM monitor and a community app network |
| CasaOS, Runtipi | Web dashboards on a server | None found | Not examined in depth |

Two conclusions follow. **Platform-level agent control is about to be table
stakes, not a differentiator** — Umbrel ships it ten days from this note. And
the position none of them occupy is the one this product already holds:
reviewed, containerised self-hosted apps on the computer somebody already
owns, in native windows, with recorded evidence behind each claim.

Five assets follow from that position, and no general-purpose agent tool has
them:

1. The registry knows every installed app's address, Compose project and data
   directory (`src/model.rs`).
2. Installation generates and retains each app's credentials (`src/setup.rs`,
   `local-store-secrets.json`).
3. The product owns the data directories and already confines deletion to them
   (`runtime::confined_to_managed_root`).
4. Qualification records evidence per app and withholds apps with a stated
   reason (`src/qualification.rs`, `src/templates/*.json`).
5. The catalog already holds both ends of the loop: of 49 approved templates,
   fifteen are AI front-ends or agent builders, so the agent and the tools it
   would act on can come from the same reviewed source.

---

# Part one — agents as a platform feature

The shape is an MCP gateway inside Local Store. An agent makes one connection
to it; behind it sit per-app adapters and a policy layer the person
configures.

## Architecture

```
 Claude Desktop / Claude Code / Cursor / Codex / Open WebUI / n8n agent
              │ MCP: stdio (`local-store mcp`) or loopback HTTP + per-agent token
              ▼
 ┌────────── Local Store agent gateway (Rust, rmcp) ──────────┐
 │ who is calling → allowed? → needs approval? → record it    │
 │ snapshot before writes · inject credentials · rate limits  │
 └──────┬───────────────────┬────────────────────┬────────────┘
        ▼                   ▼                    ▼
  Platform tools       App adapters          Raw access
  (existing runtime    own REST adapters     data-folder files,
   and catalog         (Memos, Kanboard…)    SQL, docker exec,
   functions)          or proxied official   headless-browser
                       MCP servers (Grafana, sidecar
                       n8n, Metabase)
```

Design choices, and why:

- **One gateway rather than a set of separate MCP servers.** Each app's MCP
  server has its own permission model or none. Routing through one gateway
  gives the same grants, approvals, audit trail and credential handling
  everywhere, and lets the tool list follow what is installed.
- **A compact tool mode is required, not optional.** Forty-nine approved apps
  at 20–40 tools each would bury an agent's context. Offer meta-tools — list
  apps, describe one app's tools, call one tool — alongside a flat mode.
- **Credentials never reach the agent.** The gateway adds each app's token to
  the request. Revoking an agent is then instant and does not mean rotating
  app passwords, and secrets stay out of the model provider's context.
- **Approvals belong in the launcher, not in the agent's chat.** The current
  MCP revision (2026-07-28) can ask a client for input mid-call, which is fine
  for low-risk prompts, but a client can be configured to auto-accept.
  Irreversible actions need a click on a surface this product controls.
- **Reuse before writing.** `rmcp` is the official Rust SDK: Apache-2.0, and
  its minimum Rust version is 1.88 — the same as this project's, so it clears
  the MSRV and licence gates. It needs tokio, which Tauri already pulls in.
  Proxy first-party MCP servers where they exist. Write small native adapters
  for simple REST apps. Admit community MCP servers only through the same
  pin-and-review process recipes go through.
- **Docker's MCP Gateway was considered and set aside.** It is open source and
  good at running MCP servers in containers with injected secrets and
  interceptors, but it knows nothing about this product's apps, credentials or
  data directories, which is the part worth building.

## Control levels

Maximum capability, with the person choosing what is granted. Local Store
itself:

| Level | The agent can | Built on |
| --- | --- | --- |
| Discover | Search the catalog, read recipe details, run the Docker check | `catalog`, `runtime::doctor` |
| Observe | List apps, status, readiness, logs | `commands::list_apps`, `runtime::logs` |
| Operate | Start, stop, restart, open a window for the person, make shortcuts | `runtime::start`/`stop`, `windowing` |
| Install | Install approved templates, connect addresses, answer setup fields | `install_template`, `add_app` |
| Configure | Change setup fields and ports, share a folder from this computer | `plan`, `setup`, `folders` |
| Remove | Uninstall and keep data | `uninstall(false)` |
| Destroy | Uninstall and delete data | `uninstall(true)` |

Inside one app, using Kanboard as the example:

| Level | The agent can |
| --- | --- |
| Observe | See health, version and counts |
| Read | Read projects, tasks and comments |
| Write | Create and move tasks, add comments |
| Delete | Delete tasks and projects |
| Admin | Manage users, API tokens and plugins |
| Files | Read and write that app's data directory, and nothing outside it |
| Database | Run SQL, read-only or read-write |
| Exec | Run commands inside the app's container |
| UI | Drive the web interface through a headless browser in its own container |

Every grant also carries an approval mode — automatic, ask once per session,
or ask every time — and an optional expiry of an hour, a day, or never.
Presets: **Look only**, **Assistant**, **Operator**, **Full control**.

**The recommended floor.** Full control should still mean the agent can do
everything; four actions would keep one human click even there:

- deleting an app's data,
- sharing a folder from this computer (`src/plan.rs` already refuses to carry
  a host mount nobody consented to),
- revealing a raw secret,
- anything touching agent grants or the activity record, so an agent can never
  raise its own access.

That removes no capability. It adds a confirmation to the irreversible
actions, and whether even this is configurable is the owner's decision.

## Features

1. **One-click connect.** Detect Claude Desktop, Claude Code, Cursor, VS Code
   and Codex, and write the MCP entry into their configuration with a backup.
   One revocable token per agent.
2. **Platform control and diagnosis.** Every current CLI operation as a tool,
   plus a composite `diagnose_app` (status, readiness, recent logs, Docker
   check) and an event feed for went-down and install-finished.
3. **Work inside apps** through the adapters surveyed below.
4. **Agent-ready installs.** Installation already generates secrets; extend
   templates to mint a scoped API token after first start — a Grafana service
   account, for instance — so an app is usable by an agent the moment it
   opens. The same step can complete first-run owner setup, which helps people
   who never touch agents.
5. **Snapshot and undo.** Before an agent's first write to an app in a
   session, copy that app's data directory; offer "undo this session". This
   product owns the bind mounts, so it can do this where a generic agent tool
   cannot.
6. **Approvals and an activity record.** Pending approvals appear in the
   launcher, optionally with an ntfy push. Every call is recorded with
   redacted arguments, outcome and a link to its snapshot. This supplies the
   durable history `docs/design/v2/CAPABILITY-MATRIX.md` currently marks
   `concept` for the Activity surface.
7. **Review then apply.** The agent submits a batch — twelve tasks, three
   monitors — the person reviews once, the gateway executes.
8. **Cross-app tools.** One search across every readable app, plus prompts for
   playbooks: Memos `#todo` into Kanboard, Uptime Kuma reports down then
   diagnose, restart and notify, a Homer tile for each installed app.
9. **Injection guard.** Label each app by whether it holds secrets, ingests
   untrusted content, can send data out, or can execute code; warn when one
   grant combines private data, untrusted content and an outbound path — the
   "lethal trifecta". Later, taint tracking within a session.
10. **A closed local loop.** The approved catalog already contains fifteen AI
    front-ends, several of which are MCP clients in their own right —
    LibreChat's own description names agents, tools and MCP. So this product
    can install the agent, install the tools, and mediate between them, with
    no cloud account anywhere in the path. That is a stronger version of the
    same idea Umbrel reaches by shipping OpenClaw in its store, and it is
    available as soon as the gateway exists rather than being a late phase.

## What the apps offer

As of September 12, 2026 there are 57 templates, 49 of them approved, plus the
three original recipes (Memos, n8n, Uptime Kuma). The table below covers the
fifteen whose interfaces were actually surveyed. Nothing in it has been
tested against a running container by this project, and the rest of the
catalog is inventoried after it rather than guessed at.

| App | Interface | Existing MCP server | What an agent could do | Note |
| --- | --- | --- | --- | --- |
| Memos | REST plus access tokens | Community only | Search and write notes | Good first adapter |
| Kanboard | JSON-RPC plus application token | Community only | Tasks and projects | Good first adapter |
| Flatnotes | REST; notes are Markdown files | None found | Read and write notes | The files adapter works even without the API |
| Homer | `config.yml`, no API | None | Edit the dashboard | Files adapter |
| ntfy | HTTP publish plus tokens | Not checked | Send messages to a phone | This is an outbound path |
| Grafana | HTTP API with service accounts | **Official** `grafana/mcp-grafana`, Docker image, streamable HTTP, caller token supported | Dashboards, queries, alerts | Run as a sidecar and proxy it |
| Metabase | Agent API plus API keys | **Official, built in**; enabled under Admin → AI; keys scoped to a group | Query data, build questions and dashboards | No extra container |
| n8n | Instance-level MCP plus API key | **Official, built in** since April 2026; choose which workflows are exposed | Build and run workflows | Write access is code execution with network access |
| Node-RED | Admin HTTP API (`/flows`) | Not checked | Read and deploy flows | Write access is code execution |
| Uptime Kuma | Socket.IO; API keys in v2 | Community only | Monitors and status | Pairs with platform diagnosis |
| Navidrome | Subsonic API | Not checked | Search, playlists | Low risk |
| Beszel | PocketBase REST | None found | Read metrics | Mostly observe |
| Vaultwarden | Admin token; the vault is end-to-end encrypted | Bitwarden's official server wraps the `bw` CLI and needs the master password; community servers target Vaultwarden | Administration only, by default | Vault access should stay off unless explicitly enabled |
| PrivateBin | Encrypted in the browser | None | Little | |
| Adminer | Web interface only | None | Nothing | Use the Database level instead |

Memos, n8n and Uptime Kuma are recipes rather than templates; Gotify was in
the first survey and is withheld.

### The rest of the approved catalog, unsurveyed

Grouped by what they are, taken from each template's own `category` and
`description`. **No interface, API or MCP claim is made for any app in this
list** — each still needs the same check the fifteen above received.

| Group | Apps | Why they matter here |
| --- | --- | --- |
| AI front-ends and agent builders | anythingllm, big-agi, dify, flowise, khoj, kotaemon, langflow, librechat, lobehub, open-webui, paperclip, sillytavern, sim, vane, maxun | The other end of the loop: these are the agents, not the tools. Several are MCP clients already |
| Notes, wikis, documents | docmost, trilium, joplin | Read/write content adapters; Joplin's server may sync only, so its API needs checking before anything is promised |
| Tasks and planning | vikunja, wekan, grocy, tandoor | Same adapter shape as Kanboard |
| Media and files | immich, jellyfin, tautulli, pairdrop | Large data directories: snapshot cost and backup design matter most here |
| Development and publishing | gitea, wordpress, penpot | Write access is close to code execution; treat like the automation apps |
| Money and life admin | ghostfolio, wallos, monica | `wallos` is a subscription tracker, so the "what am I replacing" total belongs there rather than in a new screen |
| Analytics and watching | umami-analytics, changedetection, glance, whoogle | Mostly read; `glance` is configured by file like Homer |

## Limits to state plainly

- **An agent can go around the gateway.** Agents run as the same Windows user,
  so a coding agent with a shell can read `local-store-secrets.json` or call
  `docker` directly. Grants constrain tool calls, including injected ones;
  they are not a sandbox against a determined process. Real enforcement needs
  a separate account holding the secrets, which is a large change. Until then,
  the agent's own deny rules close most of the gap.
- **The privacy promise changes.** Whatever an agent reads goes to that
  agent's model provider, and app logs are deliberately unredacted
  (`docs/security-and-privacy.md`). This needs a plain statement in the
  interface and a local-model option.
- **Undo cannot reach the outside world.** Sent notifications and fired
  webhooks stay sent, and restoring a snapshot also discards human edits made
  since it was taken.
- **Automation apps are code execution.** Write access to n8n or Node-RED runs
  arbitrary code with network access inside that container.
- **The gateway is a new privileged entrance.** Any app page open in a native
  window can send requests to `127.0.0.1`, so loopback HTTP needs per-agent
  bearer tokens and Origin/Host validation. For the UI level, never enable
  remote debugging on the shipped WebView2 — any local process could then
  drive signed-in app windows. Use a browser in its own container.

## Code impact

- A new `src/agent/` module in the library — policy, audit, credential broker,
  snapshots, tool router — used by both `local-store mcp` (stdio, from
  `src/cli.rs`) and the launcher (loopback HTTP, approval screens behind
  `require_launcher`).
- An `agents.json` file and an append-only activity record beside the
  registry, under the same file lock.
- An optional `agent` block in templates: adapter, token provisioning,
  snapshot method, risk labels. Template structs use `deny_unknown_fields`, so
  this needs a `schema_version` bump.
- A sidecar MCP server breaks the rule that a plan publishes exactly one port
  (`src/plan.rs`). Either allow a second loopback port or keep adapters on the
  internal network only.
- An "agent-ready" proof per app in qualification: create, read, update and
  delete against a real container, recorded like the existing lifecycle
  evidence.
- **Already in place, contrary to an earlier note in this work:** lifecycle
  operations take a per-app cross-process file lock (`runtime::lock_operation`,
  used by start, stop, install and uninstall), so a separate MCP process is
  coordinated with the launcher. `docs/security-and-privacy.md` still says
  these are serialised within a single process only; that line is stale and
  should be corrected.

## Suggested order

1. Platform MCP at Umbrel's level plus this product's extras: stdio server,
   platform levels, per-agent tokens, launcher approvals, activity record,
   one-click connect.
2. First content adapters — Memos, Kanboard, ntfy, Homer — plus Grafana
   through its official server, the credential broker, and a snapshot before
   the first write.
3. Safety features: undo session, review-then-apply, trifecta warnings,
   expiring grants.
4. Raw levels: files, SQL, exec, browser container.
5. The event feed. The local agent runtimes need no phase of their own: they
   are already approved templates, so the loop closes as soon as step 1 ships
   and one of them is pointed at the gateway.

---

# Part two — differentiation and sellable features

## Things only a desktop product can do

- **Apps that sleep when their window closes.** Start on open — Compose apps
  already boot and wait for health — and stop a few minutes after the last
  window closes, keeping apps marked always-on (ntfy, Vaultwarden for phone
  sync) running. Pause non-essential apps on battery and cap each app's
  memory. Servers are always on; laptops are not. Sablier does start-on-demand
  for servers by putting a reverse proxy in front; no proxy is needed here
  because this product owns the window.
- **Quick capture.** A global hotkey opens a small box that sends text, a link
  or a file to Memos, Flatnotes, Kanboard or ntfy. A right-click "Send to"
  entry does the same for files: on Windows that menu is a folder of
  shortcuts, so it costs little.
- **Native notifications and a tray icon.** Show ntfy and app alerts as
  Windows notifications; a tray menu listing running apps, opening them, and
  offering "pause everything".
- **Try before installing.** "Try for an hour" installs into a throwaway
  directory, opens it, and deletes everything including data when the window
  closes or the timer ends. Install and delete-data uninstall already do the
  hard part. Not found in Umbrel, Cloudron or Pinokio.

## Visible trust

- **An app label.** One panel per app: what it stores and where, which port,
  what it can reach, how old the image is, when it was last verified, and the
  evidence. Templates already carry `risk_notes`, image dates, digests and
  `lifecycle_proof`, so this is largely surfacing existing data.
- **Show withheld apps and the reason.** "Gotify is withheld: its image has
  not been rebuilt since 2022." No competitor publishes what it refused, and
  it is the most credible trust signal this project owns.
- **An internet switch per app.** "This app never needs the internet" puts it
  on a Docker network with no outside route, with a small forwarding container
  publishing its one port. Later, a record of which domains each app
  contacted.
- **Vulnerability watch.** Scan the pinned digests with Trivy or Grype (both
  Apache-2.0) in CI, and tell people with that app installed when a known
  issue appears and whether a verified update exists.

## Growing out of the desktop

- **Backups and restore** — encrypted, scheduled, to a directory, an external
  drive or object storage. Build on restic (BSD-2-Clause) or Kopia
  (Apache-2.0) rather than writing a backup engine.
- **Safe updates**, already phase 8 of the one-click roadmap: snapshot,
  update, health check, automatic rollback. The health-check and rollback
  machinery exists.
- **Move an app to a server.** A laptop sleeps, so phone-facing apps
  eventually need an always-on machine. Export an app — its plan, data and
  secrets — and import it on another computer, or on a headless Linux build
  running on a mini PC, NAS or VPS; then manage those hosts from the same
  desktop application, since Docker can address remote hosts over SSH. Every
  competitor starts at the server, so none of them tells this story.
- **Remote access** through Tailscale rather than a relay of this project's
  own. Umbrel includes it, so it is expected rather than impressive.

## Removing the biggest barrier

**No Docker Desktop requirement.** Installing Docker Desktop is currently the
first obstacle, and it needs a paid subscription at companies above 250
employees or $10M revenue. A runtime this product manages — Docker Engine or
Podman inside WSL2, both free — removes the hardest setup step for ordinary
users and a licence cost for businesses. Rancher Desktop and Podman Desktop
show it is possible. This is the largest engineering item here and the one
that most widens who can buy.

## Distribution

- **A "Run on your PC" badge for project READMEs.** A `localstore://` link
  that opens the review screen for that project and never installs
  automatically. Cloud platforms have deploy buttons; no equivalent for
  running on one's own computer was found.
- **Subscription replacement kits.** "Replace Google Keep", "Replace Trello",
  "Replace LastPass": install the app and run its own importer on the person's
  export. The savings total needs no new screen and no invented figures —
  `wallos`, already approved, is a subscription tracker, so the kit can hand
  it the subscription the person is cancelling.
- **"Verified on Local Store"** for community recipes that pass qualification.
  Pinokio already has community publishing; evidence-gated verification is the
  part this project would add.

## Two primitives carry most of the list

1. **App snapshots** power agent undo, backups, safe updates, moving to a
   server, and the cleanup behind "Try".
2. **Resource statistics** (`docker stats`, disk usage) power sleeping apps and
   battery mode, and fill in the Overview and Activity surfaces that
   `docs/design/v2/CAPABILITY-MATRIX.md` currently marks `concept` for CPU,
   memory, disk and uptime.

Building those two first is the cheapest route to five or six sellable
features.

## What could be charged for

The launcher is MIT, so any paid code feature can be forked; what costs money
to run, or what businesses need, is more defensible.

| Tier | Contents |
| --- | --- |
| Free | Everything local: catalog, installs, native windows, trust labels, vulnerability alerts, basic agent access |
| Pro, individuals | Hosted encrypted backup, several machines under one view, advanced agent policy with audit export, sleeping-app automation |
| Business | The runtime without Docker Desktop, team sharing with single sign-on, administrator policy files naming allowed apps and agent levels |

## If three things happen next

1. **Agent gateway and snapshots** — the headline, and snapshots feed half
   this document.
2. **Sleeping apps and quick capture** — together they answer "why a desktop
   application?" in one demonstration.
3. **The app label with visible withheld reasons** — nearly free, and it makes
   the trust story visible immediately.

---

## Decisions this document does not make

1. Whether platform-level agent access is worth building at all, given that
   Umbrel ships it on September 22.
2. Whether approvals live only in the launcher, or also use the agent client's
   own prompt.
3. Whether to accept the same-user bypass or invest in a separate account to
   hold secrets.
4. Whether the recommended approval floor stands, or is configurable.
5. Which apps get the first adapters.
6. Which target buyer the product is aimed at: AI power users, subscription
   refugees, or small offices. The pricing table above assumes all three,
   which no single release can serve.

## Sources

Checked September 11–12, 2026.

- umbrelOS 2.0: <https://umbrel.com/umbrelos>,
  <https://github.com/getumbrel/umbrel/releases/tag/2.0.0-beta.1>
- MCP 2026-07-28 revision:
  <https://blog.modelcontextprotocol.io/posts/2026-07-28/>
- Rust SDK: <https://crates.io/crates/rmcp> (3.3.0, Apache-2.0, Rust 1.88),
  <https://github.com/modelcontextprotocol/rust-sdk>
- Docker MCP Gateway:
  <https://www.docker.com/blog/docker-mcp-gateway-secure-infrastructure-for-agentic-ai/>
- Grafana: <https://github.com/grafana/mcp-grafana>
- n8n: <https://docs.n8n.io/advanced-ai/mcp/accessing-n8n-mcp-server/>
- Metabase: <https://www.metabase.com/docs/latest/ai/mcp>
- Bitwarden: <https://github.com/bitwarden/mcp-server>
- Uptime Kuma: <https://github.com/DavidFuchs/mcp-uptime-kuma>
- Memos: <https://usememos.com/docs/integrations/api-access>
- Kanboard: <https://docs.kanboard.org/v1/api/>
- Lethal trifecta: <https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/>
- WebView2 remote debugging warning: <https://github.com/Haprog/tauri-cdp>
- Pinokio: <https://www.therundown.ai/tools/pinokio>
- Cloudron: <https://www.capterra.com/p/190457/Cloudron/>
- Docker Desktop licensing: <https://www.docker.com/pricing/>
- Sablier: <https://github.com/sablierapp/sablier>
