# GitHub review and promotion audit — October 1, 2026

The approved V2 launcher now offers **Review GitHub link** in Discover. A user
explicitly submits a public HTTPS GitHub repository, tree or file link. The
existing launcher-only resolver verifies the canonical repository through the
GitHub API and pins its current default-branch commit before matching it against
the reviewed offering list. Repository links are not executable instructions.

An exact approved match opens the existing install review for the returned
offering ID. That review still controls setup answers, engine availability,
image pins, current proof and installation. The inspected GitHub commit is
metadata; it does not replace the reviewed deployment version. Multiple approved
matches require a choice. Unknown repositories and upstream candidates expose
remaining review work with no install action. Candidate details include source,
pinned definition, archive digest, images, service count and required answers.

## Targeted verification

- Five browser tests pass: approved identity to the existing install review
  without installing; pinned candidate provenance and blockers without an
  install action; credential-bearing URL refusal before network access and
  rate-limit retry; unknown status refusal; and stale response rejection after
  closing/reopening. Axe reports zero violations for the approved review.
- Approved and expanded candidate states were visually inspected at 1280×800.
  They retain V2 charcoal surfaces, native dialog focus and bounded scrolling.
  After a reviewed action becomes available, the lookup action is secondary.
- Six receipt tests pass. They refuse archive corruption, definition drift,
  withheld review, maintenance blockers, expired facts, changed plans, missing
  current proof, repository alias mismatch and failed task evidence even when
  a ledger digest is updated. These fixtures are not app qualification proof.
- Seventeen Rust source-boundary tests pass, including oversized links refused
  before any HTTP request, canonical identity/alias checks, default-branch
  confusion, malformed redirects and pinned candidate provenance. The opt-in
  live unit test remains ignored in that targeted run; the actual helper's
  live PrivateBin inspection below passed separately.

## Real source-to-offering audit

[PrivateBin receipt](privatebin-github-promotion-2026-10-01.json) traces an existing
reviewed Runtipi candidate to its approved offering using the actual pinned ZIP,
exact embedded definition/configuration, recorded explicit owner promotion,
current Windows managed-engine encrypted-paste/lifecycle proof, the normalized
launcher install plan, and live canonical GitHub identity.

The live inspection returned repository ID `62937181`, canonical identity
`privatebin/privatebin`, default branch `master` and commit
`10c6ba07e936a0758f242190c01f5744f18ac029`. It resolved uniquely to `privatebin`.
This is a reproducible audit of an existing promotion, not a new approval or an
increase in the ten-app launch roster. Its agent content path remains unverified.

The Rust helper uses the same offering, source resolver, normalized plan and
proof projection as the launcher. The Python receipt generator reuses the
existing qualification validator. Neither edits reviewed manifests, installs
containers or accepts shell/build hooks from a repository. Changed input must
be reviewed and qualified again before a new receipt can pass.

```powershell
cargo run --locked --release --example github_review_facts -- privatebin --inspect-github > .cache/privatebin-github-facts.json
python scripts/github-promotion-receipt.py --app privatebin --facts .cache/privatebin-github-facts.json --output docs/evidence/privatebin-github-promotion-NEW-DATE.json
```

Run the receipt within fifteen minutes of generating facts; output must be a
new file under `docs/evidence/`. The frozen candidate index may continue to
describe historical screening work; it is not an install authorization.

Browser tests use a native-command adapter. The live resolver and actual app
qualification are separate real checks that reuse the same backend paths.
This does not claim a clean-machine Windows WebView journey, arbitrary GitHub
repository installation, fresh promotion of an unknown app, or universal agent
content access. Those have separate release gates.
