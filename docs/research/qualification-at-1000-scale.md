# Research: qualifying a thousand local apps

> Historical research, indexed October 4, 2026. The project is paused. This study is not current release scope; use [V1_TASKS.md](../V1_TASKS.md). Recheck upstream facts before adopting its proposals.

September 23, 2026. This is a method study, not new app qualification. The current release count remains **1/50 managed-engine, meaningful-task verified** (Memos); n8n has a managed lifecycle pass without task proof. The 49/52 clean preflights are metadata checks only.

## What existing catalogs automate

| System | Reusable method | Limit of its claim |
| --- | --- | --- |
| [Winget validation](https://github.com/microsoft/winget-pkgs/blob/master/doc/Validation.md) and [Chocolatey verifier](https://docs.chocolatey.org/en-us/community-repository/moderation/package-verifier/) | Manifest/security checks, installation in disposable environments, uninstall and periodic re-verification | Package installation, not an app's meaningful user task |
| [TrueNAS apps CI](https://github.com/truenas/apps/blob/master/CONTRIBUTIONS.md) | Render template from test values, validate Compose, deploy, wait for health; common library for dependencies | Functionality still needs manual checks |
| [CasaOS AppStore CI](https://github.com/IceWhaleTech/CasaOS-AppStore/blob/main/docs/cicd/overview.md) | Per-app metadata, asset, Compose and architecture reports | Static validation |
| [Umbrel app testing skill](https://github.com/getumbrel/umbrel-apps/blob/master/.claude/skills/umbrel-test-app/SKILL.md) | Programmatic install/lifecycle on the real runtime, then browser and meaningful workflow | Image pull, Compose up, ready state or login page alone are insufficient |
| [Coolify one-click templates](https://github.com/coollabsio/coolify/blob/v4.x/templates/service-templates-latest.json) | Large reusable Compose recipe supply | An import source, not evidence it works in Local Store |

No inspected upstream system certifies 1,000 heterogeneous apps end-to-end, including a meaningful task, on **our** Windows/owned-WSL engine. Their scaling lesson is to reuse definitions, checks and infrastructure, and state the strength of each verification claim precisely.

## Strongest new acceleration candidate: browser action discovery

[Jev Ultrafast](https://github.com/browser-use/jev-ultrafast) is an MIT browser agent that exposes only currently observed DOM controls and supported actions. Jev chooses the action and element in one typed request; a separate small text model supplies text only when needed. It has no app-specific selector scripts. This is more capable than our earlier Jev triage plan suggested: **Jev can drive a browser when surrounded by a constrained executor**. Its author requires an independent outcome check and lists major MVP gaps: shadow DOM, iframes, uploads, popup tabs, canvas and unusual controls. The reported 3/3 result is three runs of one flight-search task, not broad reliability evidence. Its demo uses an existing Chrome profile; our test runner needs a fresh isolated browser profile. Tasks involving typing need a text-model API key as well as the existing TypeSafe key.

[Stagehand](https://github.com/browserbase/stagehand) (MIT) offers local-browser `act`, `observe`, `extract` and an agent, with replay of discovered actions. [Workflow Use](https://github.com/browser-use/workflow-use) can turn a successful agent run into a parameterized semantic workflow with AI fallback; it is AGPL and explicitly early-stage. [Skyvern](https://github.com/Skyvern-AI/skyvern) offers visual browser automation, also AGPL. Commercial [mabl auto-heal](https://help.mabl.com/hc/en-us/articles/19078583792404-How-auto-heal-works) adapts already-recorded browser tests; its advanced auto-heal only activates after five successful plan runs, so it does not bootstrap new-app coverage. None supplies a trustworthy pass/fail oracle for Local Store.

## Proposed verification ladder

1. **Catalog checked:** source identity, license/architecture policy, pinned image digest, valid metadata/Compose, safe port/volume/capability policy.
2. **Install tested:** one-click install on an isolated copy of our managed Windows/WSL engine; every service healthy, opening route responds, clean uninstall and resource ownership proven. No task claim.
3. **Task verified:** a unique synthetic item is created and independently read back after restart and keep-data reinstall; task traces and exact assertions are tied to manifest, engine, images and probe hashes.
4. **Curated release proof:** task-verified plus update/recovery, agent access and human UX review as required by V1 gates.

Display these statuses separately. Reaching 1,000 catalog-checked or install-tested entries can be a real scale milestone, but must not be marketed as 1,000 fully usable apps. The earlier 50-app target is superseded by the ten-app launch target; meaningful-task proof remains required.

## Implementation path that reuses this repo

1. Keep `scripts/qualification-preflight.py` and the existing importers as the fast intake gate. Deduplicate the same upstream app and image digest across Umbrel/TrueNAS/CasaOS/Coolify sources. Queue only changed inputs or evidence that aged out; never infer a pass from an upstream catalog's badge.
2. Keep `qualify_on_engine_at`, `Batch`, and `FirstUse` in `src/qualification.rs`. The harness already has lifecycle/evidence identity. **Do not parallelize it on one engine**: its foreign-resource check can confuse another concurrent qualification with leaked resources (the Grafana case is documented in code). Parallelism means one clean Windows/WSL engine per VM/worker; begin with one worker and measure disk, memory, image pulls, wall time and failure causes.
3. Batch by shared image layers and task family. Reuse cached immutable image layers and fixture images, but always give each app a fresh owned data root. Shared task drivers cover content, files, workflows, monitors, database clients and controlled external services. App-specific data should be a small reviewed descriptor, not a new probe program whenever possible.
4. For UIs lacking a usable API, **pilot** Jev Ultrafast and Stagehand as constrained task executors on isolated Chromium against local-only URLs. Record the successful trace, then convert stable steps into a replayable recipe. Keep browser-agent `DONE` out of pass/fail. An independent oracle must read the exact synthetic marker from the app/API/DOM after lifecycle transitions. Unexpected external navigation or actions fail closed.
5. Quarantine failures and unclear outcomes for review. Track pass rate, false-pass rate, time/app, model calls/app, cost/app, retry rate and number of manual interventions. Re-test changed image, manifest, engine or probe fingerprints; also schedule freshness rechecks. Avoid using AI confidence as evidence.

**Pilot gate:** Compare existing deterministic probes with Jev Ultrafast and Stagehand on eight diverse apps (Memos, Flatnotes, n8n, Uptime Kuma, Gitea, Immich, Open WebUI and Adminer), including one restart and keep-data reinstall. Memos is the positive control and Uptime Kuma tests whether semantic control selection avoids its previous brittle login selector. Some apps may need a fixture (model, SQL database, sample file) or may be unsuitable for the browser agent; record that as a result. Do not promote a browser tool to the main pipeline until it shows **zero false passes** against independent assertions and materially reduces human setup/maintenance over the scripted baseline.

At an illustrative 15 minutes of isolated runtime per app, 1,000 fresh runs consume 250 worker-hours: about 12.5 hours with 20 independent workers before pulls, retries or review. This is a capacity model, not a measured forecast. Changed-only requalification, caches and task-driver reuse lower recurring work; no method removes the need to prove unique app behavior.
