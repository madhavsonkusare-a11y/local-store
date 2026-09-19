# Documentation

Start with [V1 release tasks](V1_TASKS.md), then the [agent handoff](agent-handoff.md).
Only the master ledger owns task status and release scope. Updated September 12, 2026.

| Area | Read | Purpose |
| --- | --- | --- |
| Release | [V1 tasks](V1_TASKS.md), [Hermes traceability](upgrade-status.md), [publishing](../PUBLISH.md) | Current commitments, original 33-task mapping, release procedure |
| Next session | [Agent handoff](agent-handoff.md) | Exact checkout state, next bounded work, validation and known blockers |
| Architecture | [Backend coverage](backend-app-coverage-plan.md), [command contract](command-contract.md) | Reused installation pipeline and frontend/backend semantics |
| Detailed plans | [Managed engine](plans/bundled-engine.md), [qualification](plans/qualification.md), [agent access](plans/agent-access.md) | Implementation detail subordinate to the master ledger |
| Product design | [Design index](design/README.md), [V2 reference handoff](design/v2/HANDOFF.md) | V3 is owner-led and pending; V2 is an approved reference, not the current implementation target |
| Operations | [CLI](cli.md), [recovery](interrupted-install-recovery.md), [security/privacy](security-and-privacy.md) | Current behavior and supported actions |
| V1 roster | [100-app planning roster](v1-app-roster.md), [acceptance matrix](../catalog/v1-roster.json) | Future sourcing and replacement pool; planning is not qualification |
| Catalog | [Contribution guide](catalog.md), [recipe requirements](recipe-requirements.md) | Reproduction, sources, image and manifest requirements |
| Generated reports | [Queue baseline](candidate-queue-baseline.md), [ranking](candidate-ranking.md), [CapRover imports](caprover-import-report.md), [Runtipi imports](runtipi-import-report.md), [CapRover setup](caprover-setup-report.md), [source audit](caprover-source-audit.md) | Measurements at pinned revisions; regenerate through their scripts, never edit counts by hand |
| Evidence | [Recipe lifecycle](recipe-lifecycle-proof.md), `evidence/*.json`, [lessons](lessons.md) | Dated evidence and hard-won implementation constraints; not proof of an untested future release |
| Research | [Import sources](research/import-sources-study.md), [Umbrel architecture](research/umbrel-implementation-reference.md), [agent/product survey](research/agent-platform-and-differentiation.md), [monetization](research/monetization.md) | Dated research; verify external claims before adopting them |
| Historical shortlist | [September 9 screening](candidate-screening.md) | Provenance for the original candidate decisions; current status is in manifests |

## Maintenance rules

- Add or update release work in `V1_TASKS.md`, with acceptance and dependencies.
  Do not append a new roadmap to the handoff.
- Keep the handoff short: current state, next task, checks, blockers and
  decisions that the next agent would otherwise have to recover from chat.
- Keep detailed implementation plans under `plans/`, unapproved studies under
  `research/`, and design artifacts under `design/`.
- Generated reports retain stable paths because scripts reproduce them there.
- Preserve qualification JSON, source notices, manifest-linked evidence and
  approved design assets. They are inputs to validation, not disposable notes.
- Update statements about shipping behavior from code/evidence. A design
  approval does not make a backend capability implemented.

## Cleanup record

The old `next-phase.md`, `visual-phase.md`, `catalog-phase.md`,
`caprover-adapter-checklist.md`, `one-click-roadmap.md`, root
`00_Design_Notes.md` and the long `agent-history-2026-09-07.md` are retired.
Their completed work and still-relevant decisions are represented in the master
ledger, architecture reference, lessons and original 33-task mapping. Git
history retains their original text; no evidence JSON was removed.

The former 130 KB handoff is replaced by a short current handoff. Detailed
engine/qualification plans and import studies were moved, not discarded. The
previously local agent and monetization drafts are now explicit research docs;
their original copies remain in the named pre-consolidation stash.
