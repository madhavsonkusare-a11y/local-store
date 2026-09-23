# Candidate queue baseline

Generated offline by `python scripts/build-candidate-queue.py` using actual Rust adapters.

```json
{
  "caprover": {
    "definitions": 356,
    "expressible": 124
  },
  "runtipi": {
    "definitions": 250,
    "expressible": 123
  },
  "resolved_project_groups": 233,
  "unresolved_definitions": 365,
  "confirmed_unique_verified_apps": null
}
```

Repository URLs are source-declared identities, not independently verified matches.
Unknown identities remain source-qualified; equal image repositories are possible matches,
never automatic app merges. Consequently no unique verified-app total is claimed.

Four reviewed candidates lead the queue; remaining rows use structural screening:
expressible, fewer required inputs, services and fields. First use remains unverified.
No candidate is promoted. Field values/defaults and generated credentials are not exported.

## First 20 screening candidates

| Source | App | Services | Required inputs | Identity |
| --- | --- | --- | --- | --- |
| runtipi | privatebin | 1 | 0 | reviewed_repository_match |
| runtipi | nodered | 1 | 0 | reviewed_repository_match |
| caprover | linkding | 1 | 0 | reviewed_repository_match |
| runtipi | joplin | 2 | 0 | reviewed_repository_match |
| runtipi | planning-poker | 1 | 0 | source_declared_repository |
| runtipi | homer | 1 | 0 | source_declared_repository |
| runtipi | whoogle | 1 | 0 | source_declared_repository |
| runtipi | ntfy | 1 | 0 | source_declared_repository |
| runtipi | cheshire-cat-ai | 1 | 0 | source_declared_repository |
| runtipi | changedetection | 1 | 0 | source_declared_repository |
| runtipi | glance | 1 | 0 | source_declared_repository |
| runtipi | grafana | 1 | 0 | source_declared_repository |
| runtipi | inspircd | 1 | 0 | source_declared_repository |
| runtipi | jellyfin-vue | 1 | 0 | source_declared_repository |
| runtipi | kiwix-serve | 1 | 0 | source_declared_repository |
| runtipi | libretranslate | 1 | 0 | source_declared_repository |
| runtipi | dashy | 1 | 0 | source_declared_repository |
| runtipi | nginx | 1 | 0 | source_declared_repository |
| runtipi | nextgba | 1 | 0 | source_declared_repository |
| runtipi | ollama-cpu | 1 | 0 | source_declared_repository |

## Reviewed shortlist

The first four rows use the explicit maintenance/first-use screening order in
`catalog/candidate-reviews.json`; remaining rows retain structural ordering.
See `docs/candidate-screening.md` for the decision and qualification gates.
Reviewed repository matches merge identity only, never deployment definitions or approval.
