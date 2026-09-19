# Raising the qualification bar

The shared template harness (`src/qualification.rs`, `qualify_template`)
records eleven steps: it pulls pinned digests, installs in one action,
answers on its address, opens to a page with a clear next step, survives a
restart and a keep-data reinstall with its data and generated credentials
intact, removes everything it created, and leaves other containers alone —
on one host and one architecture.

`scripts/standard-probe.mjs` states the limit of that in its own header:
"What it proves: the address opens something a person could act on. What it
does not prove: that they could finish a real task. An app passing this is
*tested*, not *verified*, and the two must not be conflated."

This is the ladder from tested to verified, in the order worth climbing.

## What "offered" does and does not mean

Worth saying plainly, because the store does not yet say it:

- **Broad end-to-end app coverage is unproven.** Existing per-app evidence does not show that every user can send a chat and get a reply, run
  a Langflow flow, scrape a page with Maxun or upload a file to a LobeHub
  knowledge base. The second-address probe added for Sim, Maxun and LobeHub
  checks that those addresses answer, not that uploads or scrapes work.
- **Most AI apps do nothing until a model is configured.** AnythingLLM,
  Big-AGI, SillyTavern, LibreChat, Khoj, Kotaemon, Vane and Open WebUI all
  need an API key or a local Ollama. Their risk notes say so.
- **Several are open with no login** — Langflow, Khoj, Node-RED, AnythingLLM
  until a password is set. Loopback publication reduces network exposure, but does not protect
  against other local processes or make an unauthenticated app harmless.
- **Broad app upgrades are not covered.** Memos has historical upgrade proof;
  other migration paths need their own evidence.
- **macOS, Linux hosts and arm64 are unverified**, as is anything
  long-running: backups, data growth, months of uptime.

## Current planning context

The integration baseline has 52 offerings. The old twelve-app batch is finished
in its manifests; it is not the next task. V1 now requires the bundled engine,
50 verified release apps, V3, signed updates and agent access across the
release roster. The frozen 100-app planning roster remains available for future
expansion and reviewed replacements.
Task ownership and dependencies are Q01–Q05/C03/A06 in
[V1_TASKS.md](../V1_TASKS.md). The detail below explains the proof requirements.
Historical Memos upgrade evidence exists; it does not prove template upgrades.

## 1. Assert every container is healthy, not only the main address

**Why.** Qualification never inspects container state. A crash-looping Dify
worker, a Sim `cron` that exits non-zero, or a LobeHub bucket job that failed
all pass today, because the front page still loads. Dify runs 15 containers,
12 of which the current proof cannot see.

**Change.** A step in `qualify_template` after each usable-phase check: read
`docker compose ps --format json` for the run's project and fail when any
container is `restarting`, `unhealthy`, or exited non-zero. One-shot jobs
(`plan.is_job(name)`) are the exception: they must have exited zero.

**Acceptance.** A deliberately broken definition (a service with a bad
command) fails the new step while its main page still loads.

**Risk.** This may fail apps already approved. That is the point, but expect
the first run to demote something; re-run the whole offered suite after
merging and record any demotion with its reason.

**Effort.** Small. Highest value per line, and it applies to every app
already offered.

## 2. Record what an app costs to run

**Why.** "How much memory does this need?" is the fact people most want
before installing, and the store cannot answer it. Kotaemon's 15.9 GB image
is known only because somebody looked it up by hand.

**Checkpoint.** Qualification now samples project-scoped
`docker stats --no-stream` after install, restart and reinstall. Evidence holds
time-to-first-answer, the first idle sample, peak memory per container and peak
total memory. The same samples measure bounded managed bind storage, and one
batched image inspection records virtual bytes per deduplicated immutable image
ID. Missing samples make older evidence ineligible for reuse. Named-volume
measurement and explicit limits remain. Have
`scripts/generate-first-party.py` and `scripts/generate-template.py` turn it
into a risk note ("Needs about N GB of memory with every service running").

**Acceptance.** Dify's evidence records a peak figure, and its manifest
carries the note.

**Effort.** Small, additive to the evidence schema.

## 3. Prove account creation, not page load

**Why.** For most of these the real first task is making an account and
signing back in. Email-verification requirements, missing SMTP and broken
session cookies all pass the current probe.

**Change.** A shared probe that registers a known credential, restarts, and
signs in again, with per-app selectors in a small table. Most apps need only
a few lines; `--probe` already exists for per-app scripts.

**Acceptance.** LibreChat, Sim, Maxun, LobeHub and Dify each prove a round
trip through registration and sign-in.

**Effort.** Medium, mostly per-app selector work.

## 4. A stub model provider, so the AI apps can be driven end to end

**Why.** The reason none of the AI apps is *verified* is that a real task
needs a model, which needs a key and costs money. This is the piece that
unlocks the rest.

**Change.** A small OpenAI-compatible container started on the qualification
network — `/v1/models`, `/v1/chat/completions` (streaming and not),
`/v1/embeddings` — returning deterministic answers. A probe points the app at
`http://stub:8080/v1` with a fake key, through its settings UI or its
environment, and asserts the reply appears on screen.

**Covers.** AnythingLLM, LibreChat, Big-AGI, Open WebUI, LobeHub,
SillyTavern, Khoj, Kotaemon, Flowise, Langflow, Dify, Sim — a dozen apps from
one piece of shared machinery, with stable assertions because the answers are
fixed.

**Acceptance.** At least three apps answer a question in the UI with no real
provider key, and the evidence's `first_use` says so.

**Effort.** Medium for the stub, then small per app.

## 5. Prove the upgrade, not only the install

**Why.** What breaks in real life is moving to the next version, where
migrations run. Nothing here tests that: each manifest pins one version.

**Change.** A mode that installs the previously offered digest (git history
of the manifest has it), creates data through the probe, re-renders at the
current pin, and checks the app starts with its data intact.

**Acceptance.** One app with real migrations — Dify, LobeHub or Joplin —
passes an upgrade run, and a deliberately incompatible pair fails it.

**Effort.** Medium.

## 6. Prove backup and restore

**Why.** Keep-data reinstall is proven; copying the managed folder to another
machine is not, and that is what people need when they move or lose a disk.

**Change.** An app-consistent backup that includes managed bind data, named volumes
and generated credentials, then restores into a fresh isolated installation.
Use native database backup or quiesce services; copying live database files is
not sufficient. Browser-local state is a separate surface.

**Acceptance.** An app with a database (Monica, Joplin, Dify) restores into a
fresh install from the supported backup, including all required volumes and secrets.

**Effort.** Medium.

## 7. Re-prove on a schedule

**Why.** Tags get rebuilt and upstream breaks without anybody touching this
repository. `check-template-platforms.py` exists for exactly this and runs
in CI on pull requests and pushes to main; scheduled qualification is not implemented.

**Change.** A scheduled workflow that re-runs the offered suite and the
release watch `generate-template.py` already prints, and opens an issue when
an app falls behind or stops passing.

**Effort.** Small once 1 and 2 are in, because the failure signal is better.

## 8. Say the proof level in the product

**Why.** The probe insists tested is not verified. The store should show
which one an app has: "opens and survives restart" against "proven end to
end". It is honest, it matches what the product claims, and it turns items
1–6 into a ladder people can see apps climbing.

**Change.** A `proof_level` in the manifest, derived from which steps the
evidence contains rather than typed by hand, shown on the app's review
screen.

**Acceptance.** The guard test refuses a manifest whose claimed level is not
supported by its evidence.

**Effort.** Small, after the steps above exist to distinguish levels.

## Order and dependencies

1. Define the new engine/evidence contract; do not restart the completed twelve-app batch.
2. Items 1 and 2 — generic, small, and they apply to every app already
   offered. Re-run the whole offered suite afterwards.
3. Item 4 — the stub provider, which the AI catalogue needs.
4. Item 3, then 5, 6, 7, 8.

Items 1, 2 and 8 change what every manifest claims, so each needs a full
re-proof of the offered suite and will produce a large, mechanical diff.
