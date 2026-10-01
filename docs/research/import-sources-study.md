# Where the next few hundred apps come from

## Implementation follow-up (September 8, 2026)

The original survey below is useful prioritization, not importer verification.
Its throwaway scans are not present in this checkout. The reproducible
[CapRover source-format audit](../caprover-source-audit.md) now pins an archive and
checksum in `catalog/import-audit-sources.json`; run
`python scripts/audit-caprover-source.py` offline. At that revision it measures
356 definitions, 657 services, 299 apps with validation patterns and 150 apps
declaring generated hexadecimal variables. These counts differ from the earlier
scan and do **not** establish 180 installable apps.

The [setup compatibility report](../caprover-setup-report.md) additionally runs the
actual Rust primitives: 1,377 patterns and 222 secret declarations are supported;
75 apps contain a refused primitive. Complete setup-variable mapping represents
2,057 variables and refuses 520; the report includes named refusal counts.
This does not establish deployment support. The refreshed Runtipi report now
shows 83/250 expressible plans following the owner's port-mapping changes.
Use the [adapter checklist](../V1_TASKS.md) for the next batch.

Two corrections matter for implementation:

- `cap_gen_random_hex` is not equivalent to our existing alphanumeric generator.
  Explicit lowercase hexadecimal generation is now implemented. The pinned
  [frontend utility](https://github.com/caprover/caprover-frontend/blob/c9005cc2e5ac1b6816cb983d2d3732338c546a94/src/utils/Utils.tsx#L84)
  confirms the argument counts output characters. The strict declaration parser
  preserves that length and refuses mixed expressions and lengths outside our
  16–256 policy. Existing templates remain alphanumeric. Stored values are
  validated without replacement. This primitive is not a complete importer.
- Commands, entrypoints and health checks execute code inside containers; they
  are not automatically safe just because represented as data. Preserve bounded
  execution, explicit behavior and reviewed deployment restrictions. Do not drop
  upstream constraints merely to increase a count.

### Umbrel: researched, not excluded by reputation

Inspected [umbrel-apps at ec3a323](https://github.com/getumbrel/umbrel-apps/tree/ec3a323be0f651d4191bc0e422d67a2137768fb6).
Its [README](https://github.com/getumbrel/umbrel-apps/blob/ec3a323be0f651d4191bc0e422d67a2137768fb6/README.md)
requires a usable browser-based next step after install, a useful product standard
for Local Store. Packages combine Compose and `umbrel-app.yml` metadata.
The [Memos definition](https://github.com/getumbrel/umbrel-apps/blob/ec3a323be0f651d4191bc0e422d67a2137768fb6/memos/docker-compose.yml)
pins an image digest, uses an injected `app_proxy`, a specific user, managed data,
and a stop-grace period. The
[n8n definition](https://github.com/getumbrel/umbrel-apps/blob/ec3a323be0f651d4191bc0e422d67a2137768fb6/n8n/docker-compose.yml)
also pins a digest and depends on platform hostname and proxy/auth routing.
These need explicit adaptation, not proxy removal without replacing its behavior.

GitHub repository metadata reported `license: null`, and the inspected tree had
no top-level license file. Reuse permission for the packaging and visual assets
is **not established**; each underlying app's license is a separate question.
Keep Umbrel research-only until that is clarified. No Umbrel manifests or assets
were added to Local Store's catalog. CapRover remains the better first licensed
import source, with Umbrel useful for deployment and onboarding references.

## Original survey

A survey of existing open-source app catalogues and libraries, done to answer
one question: **what is the cheapest path to a large number of apps a person
can actually install?**

Every number below was measured, not estimated from reputation. The Runtipi
column comes from our own importer. The other three come from a scan that
applies the rules `src/plan.rs` really enforces — pinned image tag, exactly one
endpoint reaching the host, mounts confined to the app's own storage, no
privileged container. The scan scripts are throwaway; the pinned archives they
read are in `.cache/catalog/`.

**A number in this document is not a promise that an app works.** It says a
definition can be *expressed* as a deployment plan. Reviewed status still means
what it has always meant: a real install, health, restart, preserved-data
reinstall and deletion run. `docs/runtipi-import-report.md` says the same thing
and it applies with equal force here.

## Correction: the CapRover estimate was too optimistic

This study originally put CapRover at ~180 apps installable today. The real
adapter, now implemented in `src/importers/caprover.rs` and measured over all
356 pinned definitions in [the import report](../caprover-import-report.md), says
**117**. The estimate stands corrected; the ranking does not, and CapRover is
still the best-fitting source by a wide margin.

The first measurement was 73, before the importer learned to hand an app its
own loopback address. Of the 217 definitions that mention the platform domain,
most only wanted to be told where they live — see below — and that is
something a loopback install can answer once the port is settled.

The gap is one thing the scan could not see. It read the *shape* of each
definition — which Compose keys appear, whether images are pinned, how many
ports are published — but never the *values*. Inside those values,
`$$cap_root_domain` appears in 217 of the 356 apps: CapRover gives every app a
routable public hostname, and apps build their own URLs, callback addresses and
cookie domains out of it. A loopback install has no such name. Substituting
`localhost` would produce URLs that resolve for nobody and links that break
silently, so the adapter refuses those apps by name rather than approximating.

That was written before the references were read closely, and reading them
split the problem in two. **145 of the 202 references are the app's own
address**, which is exactly what a loopback install provides; the importer now
rewrites those to a placeholder the installer fills with the real port. The
remaining references name a *sibling's* public address, or carry a scheme
(`wss://`) or a mail domain that loopback cannot stand in for, and those stay
refused: 46 apps rather than 101.

What is left is a genuine product boundary — a second routable endpoint needs a
hostname, a proxy and a certificate story, all well outside an importer — but
it is less than half the size it first appeared.

The lesson generalises: a structural scan sets an upper bound, not an estimate.
Treat the Coolify and CasaOS ceilings below the same way. Only the numbers
produced by a real importer over real definitions should be quoted as counts.

## The short version

CapRover's one-click catalogue is the best-fitting source anyone has published,
and it is not close. Its definitions already carry typed setup fields with
labels, defaults and validation patterns, generated-secret slots, a single
declared HTTP port, and pinned image versions — the five things our
`PlanTemplate` needs, in almost exactly the shape it needs them.

| Source | Definitions | Licence | Installable under the model as it stands | Ceiling once the listed features are modelled |
| --- | --- | --- | --- | --- |
| [CapRover one-click-apps](https://github.com/caprover/one-click-apps) | 356 | Apache-2.0 | **117** *(real adapter)* | ~204 |
| [Coolify service templates](https://github.com/coollabsio/coolify) | 371 | Apache-2.0 | 5 | 127 |
| [CasaOS AppStore](https://github.com/IceWhaleTech/CasaOS-AppStore) | 178 | Apache-2.0 | 10 | 72 |
| [Runtipi appstore](https://github.com/runtipi/runtipi-appstore) | 250 | GPL-3.0 | 60 *(real importer)* | ~119 |

Across all four, 837 distinct app ids. The overlap is real but modest — adding
CapRover to what we already have would bring 206 apps none of the pinned
sources carry.

| Added to what came before | New apps | Running total |
| --- | --- | --- |
| CapRover | 356 | 356 |
| Coolify | 259 | 615 |
| Runtipi | 138 | 753 |
| CasaOS | 84 | 837 |

## Why CapRover fits so well

A CapRover app is a Compose fragment plus a `caproverOneClickApp` block. Here
is the whole of `actual.yml`, trimmed:

```yaml
services:
    '$$cap_appname':
        image: actualbudget/actual-server:$$cap_version
        volumes:
            - '$$cap_appname-data:/data'
        caproverExtra:
            containerHttpPort: '5006'
caproverOneClickApp:
    variables:
        - id: '$$cap_version'
          label: Actual Version
          defaultValue: '23.8.1-alpine'
          validRegex: "/^([^\\s^\\/])+$/"
```

Line for line, that is our model:

| CapRover | Local Store |
| --- | --- |
| `caproverOneClickApp.variables[]` with `label`, `defaultValue`, `description` | `SetupField` |
| `validRegex` on a variable | a `FieldKind` we do not have yet, and should |
| `$$cap_gen_random_hex(32)` as a default | `SecretSpec { length }` |
| `caproverExtra.containerHttpPort` | `PublishedPort.container` |
| `$$cap_appname-data:/data` | `PlanMount::Volume` |
| `image: …:$$cap_version` with a pinned default | our pinned-tag rule, satisfied |

Measured across the catalogue:

- **547 of 566 image references resolve to a pinned tag.** Only 13 are
  genuinely `latest` or untagged. This is the best pinning discipline of any
  source surveyed, by a wide margin.
- **354 of 356 apps declare typed setup variables** — 2,577 of them, with
  labels, defaults, descriptions and validation patterns already written by
  someone who knew the app.
- **154 apps declare generated secrets** through `$$cap_gen_random_hex(n)`,
  which is exactly the contract `generate_secret` already implements.
- **Only 2 apps** use a privilege we refuse (`cap_add`).
- Compose features we do not model barely appear: `healthcheck` once,
  `entrypoint` once, `command` six times.

What actually stops the remaining apps is not exotic:

| Reason | Apps |
| --- | --- |
| No endpoint to open — databases and workers meant to be attached to another app | 81 |
| More than one endpoint reaching the host | 34 |
| Mounts a host path (usually `/var/run/docker.sock`) | 22 |
| Unpinned image | 13 |
| `hostname`, `command`, `user`, `env_file`, `healthcheck` | 24 |
| `cap_add` | 2 |

The 81 with no endpoint are correctly excluded: they are components, not apps.
The 34 with several endpoints are a real product question — our model publishes
exactly one address on purpose — not an importer bug.

## Why Coolify looks better than it measures

Coolify has the largest and most actively maintained catalogue, and its
[magic-variable grammar](https://coolify.io/docs/knowledge-base/docker/compose)
is a genuinely good design that maps onto our model:

- `SERVICE_PASSWORD_<ID>`, `SERVICE_PASSWORD_64_<ID>`,
  `SERVICE_PASSWORDWITHSYMBOLS_<ID>` → `SecretSpec` with a length
- `SERVICE_BASE64_{32,64,128}_<ID>`, `SERVICE_REALBASE64_*`,
  `SERVICE_HEX_{32,64,128}_<ID>` → `SecretSpec` with an encoding we would add
- `SERVICE_USER_<ID>` → a generated 16-character username
- `SERVICE_URL_<SERVICE>_<PORT>` / `SERVICE_FQDN_<SERVICE>_<PORT>` → the
  published endpoint, which is why only 29 of 371 templates declare `ports:`
  at all
- `${VAR:-default}` → a `SetupField` with a default

Two things hold it back.

**A third of its images are unpinned.** 269 of 813 image references use
`latest`, `main`, `stable` or no tag, and one unpinned service refuses the
whole app under our rules. That single rule accounts for 217 of the 239 refused
templates — far more than every privilege refusal combined. This is worth
attention because it is *fixable*: resolving a floating tag to a digest once, at
import time, would convert most of those 217 into candidates. It moves the
pinning decision from the upstream author to us, which is arguably where it
belongs.

**`healthcheck` blocks almost everything else.** It appears in 343 of 371
templates. It is a pure narrowing — Docker uses it to decide whether a
container is healthy, and it grants nothing — so passing it through is safe.
Modelling it alone takes Coolify from 16 to 227 structurally-expressible
templates. Then `command` (+57), `platform` (+14) and `entrypoint` (+13).

## CasaOS, and a security finding

CasaOS ships plain Compose with an `x-casaos` block carrying icons,
screenshots, categories and translated descriptions in 14 languages — the
richest presentation metadata of any source, and useful to the catalogue
independently of whether we ever install from it.

As an install source it is the weakest of the four: 61 of 178 manifests bind a
host path, 17 use a non-default `network_mode`, 13 mount devices, 9 are
privileged. Its most common gap, `deploy:` in 136 manifests, is only memory
reservations and is trivially droppable.

**37 of the 178 manifests ship a hardcoded credential in plain text**, and they
are the same on every CasaOS installation in the world:

```
SECRET_KEY_BASE=<upstream-shared-default-redacted>
POSTGRES_PASSWORD=<upstream-shared-default-redacted>
DB_PASSWORD=<upstream-shared-default-redacted>
MEILI_MASTER_KEY=<upstream-shared-default-redacted>
```

Any CasaOS importer must treat a credential-shaped literal as a `SecretSpec`
slot to generate, never as a value to copy. Importing these verbatim would ship
apps with publicly known passwords. This is the strongest argument for keeping
the importer's refusal list conservative and its output reviewed.

## Sources deliberately not used

- **[Umbrel app store](https://github.com/getumbrel/umbrel-apps)** (772 stars,
  actively maintained) — 
- **[Easypanel templates](https://github.com/easypanel-io/templates)**
- **[Lissy93/portainer-templates](https://github.com/Lissy93/portainer-templates)**
  (MIT, 684 entries) — licensed, but unusable as-is. Of its 431 self-contained
  single-container entries, **only 27 pin an image tag**; 387 use `latest`. Its
  237 Compose entries just point at external repositories, each with its own
  licence and no pinning, which would turn one clean dependency into 237
  unaudited ones.


## Reusable code

**Keep shelling out to Docker for validation.** `docker compose config
--format json` is the real parser, and `scripts/validate-recipes.py` already
uses it. No library is as authoritative, and swapping to one would weaken the
gate.

**For reading third-party Compose files, a crate is worth it.**
[`compose_spec`](https://crates.io/crates/compose_spec) (MPL-2.0, ~11k lines)
gives fully validated, typed values, and MPL-2.0 already passes
`scripts/check-licenses.py` as accepted weak copyleft. The alternative,
[`docker-compose-types`](https://github.com/stephanbuys/docker-compose-types),
is looser but simpler. Either beats hand-rolling deserialisation for Coolify
and CasaOS, both of which are arbitrary Compose rather than a narrow schema
like Runtipi's.

**Nothing else transfers.** Coolify is PHP/Laravel and CapRover is TypeScript;
their value to us is the *grammar* of their variables, which is small enough to
reimplement in an afternoon and is documented above. There is no existing
library that turns an app catalogue into a safety-checked deployment plan —
that part is genuinely ours, and it is the part that the refusal list, the
confinement rules and the secret handling live in.

## Recommended order

1. **Write the CapRover importer.** It is the highest yield per unit of work by
   a wide margin: ~180 apps expressible with the plan model exactly as it
   stands today, no new features required. It also brings `validRegex`, which
   would improve the setup fields we already ship.
2. **Model `healthcheck`, then `command`, `entrypoint`, `platform`.** All four
   are data passthrough that grant no privilege. This is what unlocks Coolify,
   and it costs four small additions to `PlanService`.
3. **Decide the floating-tag question.** Resolving `latest` to a digest at
   import time would roughly triple Coolify's yield. It is a policy decision
   about who owns the pin, not an engineering problem, and it should be made
   deliberately rather than by leaving the rule as it is.
4. **Take CasaOS for presentation metadata now, installation later.** Its
   icons, categories and translated descriptions are immediately useful; its
   Compose is the weakest of the four and carries the credential hazard above.

None of this changes what "reviewed" means. Expressible is the cheap part; the
five-step real run is still the gate, and 180 candidate apps is 180 apps that
have not been proven to work.
