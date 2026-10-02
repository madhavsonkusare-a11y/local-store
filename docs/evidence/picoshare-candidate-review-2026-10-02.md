# PicoShare candidate review

The frozen 100-app planning roster includes PicoShare as an expansion candidate.
`catalog/promotion-proposals/picoshare-withheld.json` is a new, withheld artifact;
it is not a catalog offering and does not change the selected ten launch apps.

The draft generator verifies the checksum-pinned CapRover source archive before
mapping its one service, one named data volume and administrator passphrase.
The upstream optional passphrase is tightened to a required sensitive answer
because the official app requires `PS_SHARED_SECRET` or a secret-file setting.
No weaker requiredness override, new undeclared field or different image
repository is allowed. The old 1.4.4 tag is replaced with the exact reviewed
official v1.5.4 index digest; qualification must prove this same plan.

Primary source review on October 2, 2026:

- [Official release v1.5.4](https://github.com/mtlynch/picoshare/releases/tag/v1.5.4).
- [Pinned configuration documentation](https://github.com/mtlynch/picoshare/blob/v1.5.4/README.md): administrator secret and `/data/store.db` persistence.
- [Pinned license](https://github.com/mtlynch/picoshare/blob/v1.5.4/LICENSE): AGPL version 3 or later; a store manifest is not a relicensing of the app.
- [Official upload acceptance tests](https://github.com/mtlynch/picoshare/blob/v1.5.4/e2e/upload.spec.ts): login, upload, note, custom expiration and sharing selectors inform the isolated probe.
- [Official image source](https://github.com/mtlynch/picoshare/blob/v1.5.4/Dockerfile) uses Alpine 3.15 as its final base. [Alpine's release table](https://alpinelinux.org/releases/) gives November 1, 2023 as its normal support end and lists on-request support. The candidate's recent app release is not evidence of maintained base packages. This limitation is explicit in the withheld proposal and requires a promotion decision; no vulnerability scan or patched-image proof is claimed.

`candidate_review_facts` obtains fresh canonical GitHub metadata and checks that
the repository resolves to the exact pinned, structurally supported candidate.
It hashes the actual Rust-generated plan and proposal without exposing answers.
`candidate-promotion.py` requires a real managed Windows task, restart,
keep-data reinstall, measurements and safe cleanup. It then emits a concrete
request binding the proposal, evidence, plan, probe, source archive and live
GitHub review hashes. A separate, current review decision must name all those
hashes before an approved artifact can be created. Even that artifact does not
register an offering or install the app.

Seven small refusal tests passed, covering failed proof, changed plan or source,
stale or mismatched decision, missing image pins, changed repository commit and
unrelated resolved image IDs. Three draft boundary tests passed. These are
synthetic guard tests and do not count as a real app qualification.

The integration review declined promotion because of the unsupported final
runtime base. The compile-only fixture remains available, but no real task
receipt has been created and no actual qualification is claimed. Its test
receipt path has a historical filename; the receipt timestamp would name the
real execution date if the isolated fixture is used later. PicoShare remains
withheld and the roster membership is preserved. A maintained Linkding candidate
is being reviewed separately through the same hash-bound pipeline.
