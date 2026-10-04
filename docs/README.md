# Documentation

Updated October 4, 2026. **Local Store is shelved.** These documents preserve the source and make a future resume possible; they are not a request to restart development.

## Start here

| Question | Read |
| --- | --- |
| What is the product and current release? | [Repository README](../README.md) |
| What is complete and what remains? | [Master V1 ledger](V1_TASKS.md) — sole scope/status authority |
| What changed locally and how do we resume? | [Agent handoff](agent-handoff.md) — checkout, cleanup, blockers and bounded restart steps |
| How do I build or publish source later? | [Contributing](../CONTRIBUTING.md), [publishing](../PUBLISH.md), [published source notes](releases/source-v1.0.0.md) |
| Which interface is approved? | [Design index](design/README.md), [approved V2 handoff](design/v2/HANDOFF.md) |

## Implementation references

| Area | Reference | Interpretation |
| --- | --- | --- |
| Installation architecture | [Backend coverage](backend-app-coverage-plan.md), [command contract](command-contract.md) | Existing pipeline and typed boundaries |
| Managed engine | [Architecture](plans/bundled-engine.md), [payload](../engine/README.md), [source delivery](../engine/SOURCE_DELIVERY.md), [clean Windows host](plans/windows-clean-machine-test.md) | Development architecture and dated checkpoints; distributable payload still incomplete |
| App qualification | [Proof contract](plans/qualification.md), [scaling method](plans/qualification-at-scale.md), [recipe requirements](recipe-requirements.md) | Ten-app launch bar; future scale does not relax meaningful-task proof |
| Agent access | [Access architecture](plans/agent-access.md), [launch providers](evidence/launch-agent-coverage-2026-10-02.md), [content boundaries](evidence/agent-content-boundaries-2026-10-02.md) | Implemented bounded providers; capability, login and permission are separate |
| Operations | [CLI](cli.md), [interruption/recovery](interrupted-install-recovery.md), [support](support.md), [security/privacy](security-and-privacy.md) | Source behavior; no installed development app remains |
| Catalog maintenance | [Contribution guide](catalog.md), [lessons](lessons.md) | Reproducible source imports and known constraints |

## Historical and generated material

- `evidence/` holds dated receipts and reviews. They describe their recorded source/host, not a future rebuilt release. Ignored build files, rootfs archives and installers referenced by older receipts were deleted during shelving. The [Docker cleanup receipt](evidence/project-shelf-docker-cleanup-2026-10-04.json) records the follow-up inspection.
- [100-member planning roster](v1-app-roster.md) and [planning matrix](../catalog/v1-roster.json) are a future sourcing pool. Their pending fields are planning placeholders, not current ten-app status. Use `catalog/v1-qualified-apps.json` and current provider checks for accepted scopes; V1 targets ten apps.
- [Queue baseline](candidate-queue-baseline.md), [ranking](candidate-ranking.md), [CapRover import](caprover-import-report.md), [setup](caprover-setup-report.md), [source audit](caprover-source-audit.md) and [Runtipi import](runtipi-import-report.md) are generated snapshots. Keep their paths and regenerate through their scripts rather than editing counts by hand.
- [Candidate screening](candidate-screening.md) and [recipe lifecycle](recipe-lifecycle-proof.md) preserve earlier decisions/tests. Later managed-engine receipts supersede their coverage claims for the selected launch cohort.
- `research/` contains dated source surveys, scaling studies and commercial ideas. These are proposals; check current primary sources before adopting them. They are not extra release commitments.
- `design/v2/` retains the approved prototype, specifications, freeze and evaluation history. Prototype completion is not production or native acceptance. Earlier HTML explorations are reference material.

## Maintenance and consolidation

Keep current scope only in `V1_TASKS.md`, resume state only in `agent-handoff.md`, architecture under `plans/`, research under `research/`, and immutable proof under `evidence/`. Update code-facing references when behavior changes. Preserve manifest-linked evidence, third-party licenses and approved assets.

The obsolete `upgrade-status.md`, `plans/parallel-v1-execution.md` and `windows-manual-test.md` were removed after their useful content was consolidated. The original 33-task relationship is now in the master ledger. The old long handoff and deleted-installer instructions are recoverable from commit `a63cf2b`; no second active backlog remains.
