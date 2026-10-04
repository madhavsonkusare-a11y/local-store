> Research snapshot, not an approved implementation or pricing policy.

> Historical research, indexed October 4, 2026. The project is paused. This study is not current release scope; use [V1_TASKS.md](../V1_TASKS.md). Recheck upstream facts before adopting its proposals.
> External product, protocol, licensing, eligibility and pricing claims require
> fresh primary-source verification before implementation or commercial decisions.
> Current V1 scope and task status live in [V1_TASKS.md](../V1_TASKS.md).

# Monetization

Five revenue paths for Local Store, ranked by what the project can actually
support today — plus the licensing fact that rules one popular option out, and
a working fix for Windows code signing from India. Research current as of
7 September 2026; figures are ranges under stated assumptions, not forecasts.

## The honest read

The revenue problem is not pricing. The repository is public with zero stars,
the first commit landed 26 August 2026, and the README still says live-container
lifecycle verification is pending with two v0.5 gates unchecked. Every model
below converts attention into money; there is currently none to convert.

That is a reason to sequence the work, not postpone it. The rails that cost
nothing — a Sponsors button, a Store listing, a SignPath application — go up
now. The rails that cost trust go up only after `v1.0` installs cleanly on a
machine that has never seen this code.

The market is real. Coolify — one unfunded developer, no VC — reported roughly
$15,700 gross in a single month against $2,800 of expenses, split between
hosted plans and donations. r/selfhosted is above 820,000 members and r/homelab
above 1.1 million. People in this niche do pay.

## What this project actually owns

Separating the parts that can be sold from the parts that are merely
redistributed. Entry counts come from the `source` field in
`src/generated/catalog.json`.

| Asset | Origin | License | Can it be gated? |
| --- | --- | --- | --- |
| Launcher, windowing, CLI, runtime | Original — 7,674 lines of Rust | MIT | **Yes** — copyright held |
| Verification evidence | Original — lifecycle results, image digests, per-arch pass/fail | Unencumbered | **Yes** — original work |
| Catalog entries (1,342) | awesome-selfhosted | CC-BY-SA-3.0 | No — share-alike |
| Catalog entries (270) | Runtipi app-store | GPL-3.0 | No — copyleft |
| Catalog entries (540) | Coolify + CasaOS | Apache-2.0 | Permissive, but not exclusive |
| Bundled artwork (752 icons) | Homarr dashboard-icons, upstream brands | Apache-2.0 + trademarks | No — third-party marks |

**The instinctive plan — making the large catalog the paid feature — is closed.**
`THIRD_PARTY_NOTICES.md` already commits that adapted Runtipi material stays
under GPL-3.0 and adapted awesome-selfhosted material stays under CC-BY-SA-3.0.
That is 1,612 of 1,672 entries carrying copyleft or share-alike obligations. A
proprietary catalog tier means either breaking those terms or unpicking the
imports.

Sell the launcher and the verification. Give the catalog away — it was always
going to be commoditised, since Runtipi, CasaOS and Coolify publish the same
source data.

The second row is the one with a moat. `docs/one-click-roadmap.md` already
identifies the hard problem: *keep discovery coverage separate from verified-
install coverage.* Anyone can list 1,672 apps. Knowing that a given app
installs, passes a health check, survives a restart and preserves its data on
`x86_64-pc-windows-msvc` at a specific image digest is expensive, perishable,
and nobody else is selling it.

## Five paths, ranked

Ranked by fit with where the project is today, not by ceiling.

### 1. Sell the binary in the Microsoft Store

Keep the source MIT and the GitHub releases free forever. List the identical
app in the Microsoft Store at a small one-time price. This is the Krita model —
the Steam and Microsoft Store builds are the same GPL software, sold for
convenience — and it took that project from two full-time developers to four.

This ranks first for reasons mostly unrelated to revenue. Microsoft dropped
individual developer registration fees in September 2025 and company fees in
May 2026, non-game apps may use their own commerce and keep 100% of revenue,
and **Microsoft re-signs the package**. See
[Solving the signing problem](#solving-the-signing-problem).

| | |
| --- | --- |
| Price | $7–12 one-time |
| Setup cost | $0 registration |
| Realistic year 1 | $300–3,000 |
| License risk | None — MIT permits sale |
| Start when | v1.0 installs clean |

### 2. Open-core Pro, priced as a perpetual licence

Gate features written entirely in-house, all outside the catalog: encrypted
secrets vault, scheduled backup and restore of installed app data, update
management with rollback across installed stacks, multi-machine registry sync,
workspaces and profiles, LAN discovery, family or team sharing. Every one sits
inside code this project holds copyright on.

Price it the way Beekeeper Studio does — pay for twelve months, keep every
version released in that window forever. Homelabbers are structurally hostile
to subscriptions; they self-host precisely to escape them. A perpetual-fallback
licence removes the objection while still producing renewal revenue.

**This path has a prerequisite cost.** See
[The conflict with path 2](#the-conflict-with-path-2).

| | |
| --- | --- |
| Price | $39–59/yr, perpetual fallback |
| Build cost | Licence server + gated features + ~$219–300/yr certificate |
| At 300 licences | ~$14k/yr gross |
| License risk | None if the catalog stays free |
| Start when | ≥1,000 real users |

### 3. Sponsorship, configured now and forgotten

GitHub Sponsors takes 0% and costs nothing to enable. Add `FUNDING.yml`, a tier
set, and one honest README line about what funding buys — signing, test
hardware, the Apple Developer membership. Then stop thinking about it.

Be realistic about the curve. Coolify earns roughly $4,500/month from GitHub
Sponsors plus $1,200/month from Open Collective, but that arrived after tens of
thousands of users. Runtipi, with a smaller audience, runs on donations its
maintainer does not live on. Sponsorship is a function of adoption, never a
substitute for it.

| | |
| --- | --- |
| Platform fee | 0% (GitHub) |
| Setup cost | $0, one afternoon |
| Realistic year 1 | $0–600 |
| Start when | This week |

### 4. Sell verification, not listings

Turn phases 4–8 of the one-click roadmap into the product. A paid tier ships
continuously re-tested deployment plans: pinned image digests actually booted
on each architecture, upgrade paths validated against the previous version,
breakage alerts when an upstream image changes under an installed app. The free
build keeps the full catalog and installs whatever it can; the paid build
promises the install still works next month.

This is a subscription people accept because the underlying cost is genuinely
recurring — CI minutes, forever. It is also the only path a well-funded
competitor cannot copy in a weekend, and it is clean of the GPL and CC-BY-SA
obligations, because test results are original work regardless of whose listing
was tested.

| | |
| --- | --- |
| Price | $4–8/month |
| Build cost | CI fleet + retest pipeline |
| Prerequisite | 25+ verified apps (roadmap phase 4) |
| License risk | None |

### 5. Paid recipe work and deployment consulting

Normalising Compose definitions across four upstream formats, port allocation,
secret generation, health verification and rollback is rare expertise. Small
businesses and hobbyists pay for someone to make a specific self-hosted stack
work. Charge per recipe request, or per fixed-scope setup engagement.

Listed last because it converts hours directly into money with no leverage, and
time spent on client stacks is time not spent on the product. A bridge that
funds paths 1 and 2, not a destination.

| | |
| --- | --- |
| Rate | $150–400/engagement |
| Setup cost | $0 |
| Scales? | No — linear in hours |
| Start when | Any time |

## Solving the signing problem

Microsoft's guidance, revised August 2026, is blunt about this. Two things
follow, and the second is widely misunderstood.

**Store apps are re-signed by Microsoft and are never subject to SmartScreen
download warnings.** Nothing else achieves that. Every non-Store binary —
signed or not — shows an "unrecognized app" prompt until reputation
accumulates, which Microsoft describes as "several weeks and hundreds of clean
installs from a wide audience."

**Buying a certificate does not remove that first warning.** EV certificates
stopped granting instant reputation in 2024, and Microsoft now states that
paying the EV premium solely to avoid SmartScreen "is no longer justified."
Any advice recommending EV for this reason is working from pre-2024 knowledge.

So why sign at all? Reputation attaches to two signals — the file hash and the
publisher certificate — and only one compounds. Microsoft: "Unsigned files must
build reputation anew with every update." Every release is a new hash.
Unsigned, reputation restarts at zero forever; signed, each release inherits
the publisher trust the last one earned.

There is also a harder floor. Windows 11's Smart App Control blocks unsigned
executables outright, and unlike SmartScreen it applies to every executable,
not only downloaded ones.

### Azure Artifact Signing is not an option

Public Trust certificates cover organizations in the US, Canada, EU, UK,
Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway and
Israel; **individual developers must be in the US or Canada.** India appears on
neither list. Private Trust certificates have no geographic restriction but are
not publicly trusted, so they do nothing for SmartScreen. Rule this out and
stop budgeting the $9.99/month.

### What works, per channel

| Channel | Who signs it | Cost | First-run experience |
| --- | --- | --- | --- |
| Microsoft Store | Microsoft re-signs the package | $0 | **No warning, ever** |
| GitHub releases (Windows) | SignPath Foundation — free OV for OSS | $0 | Warns at first; publisher name shown, reputation compounds |
| macOS `.dmg` | Apple Developer ID, notarised | $99/yr | Gatekeeper passes silently |
| Linux `.deb` / AppImage | Nothing required | $0 | Existing `SHA256SUMS` suffice |
| Fallback if SignPath declines | SSL.com IV cert + eSigner cloud signing | ~$219+/yr | Same as SignPath, but paid |

**SignPath Foundation is the missing piece.** It provides free code signing to
open-source projects through an HSM-backed pipeline, with no geographic
restriction and — per their published conditions — no minimum project age, star
count or user base. Local Store qualifies on the obvious criteria today:
OSI-approved licence, public source, released artifacts, reproducible CI
builds. Applications take days to weeks.

The paid fallback matters because approval is not guaranteed. **SSL.com's
Individual Validation tier** exists for solo developers with no company behind
them, and its eSigner cloud service signs from GitHub Actions with no hardware
token — which matters from India, where a FIPS USB token would otherwise have
to clear customs. From March 2026 the CA/Browser Forum caps code-signing
certificate validity at 458 days, so this is a recurring cost with any CA.

### The conflict with path 2

SignPath Foundation's terms require an **OSI-approved licence without
commercial dual-licensing for all components**, and exclude projects containing
proprietary, non-open-source components. A closed-source Pro edition forfeits
the free certificate.

That makes month four a real fork:

- **Stay fully open.** Signing stays free forever. Store sales, sponsorship and
  the verification subscription all work without a proprietary build.
- **Go open-core.** Budget roughly $219–300/year for a certificate as a cost of
  that decision.

Both are defensible. The mistake is discovering the conflict after shipping a
gated build.

One item to disclose rather than assume when applying: the bundled catalog data
carries CC-BY-SA-3.0 and GPL-3.0 terms, and CC-BY-SA is a content licence
rather than an OSI-approved software one. Raise it in the application.

### One free improvement

Releases already ship `SHA256SUMS-<target>.txt` and `build-<target>.json`
recording version, commit, target, workflow URL and hashes, and `PUBLISH.md`
correctly declines to call that an attestation. Almost nobody does this. Once
builds are signed, state it in one line on the release page — the cheapest
trust signal available while reputation accrues.

## What the market already charges

WebCatalog is the closest commercial analogue — it also turns web apps into
desktop apps — but it is Electron, subscription-only, and aimed at SaaS rather
than self-hosted instances. That gap is the positioning: *one Rust binary, no
Electron, no account, no telemetry, for the apps you already run.*

| Project | Model | Price | Outcome |
| --- | --- | --- | --- |
| WebCatalog | Freemium subscription, closed | $5–8/user/mo | Free tier capped at 2 apps |
| Cloudron | Free 2 apps, then flat platform fee | $15–90/mo | Charges per platform, not per app |
| Coolify | Free self-host + paid cloud + donations | from $5/mo | ~$15k MRR cloud, unfunded, solo founder |
| Beekeeper Studio | GPL community + commercial Ultimate | ~$99/yr | Perpetual fallback after 12 months |
| Krita | Free download + paid store builds | one-time, low | Store income grew team 2 → 4 devs |
| Umbrel | VC + hardware | $3M seed | Sells the Umbrel Home device |
| CasaOS | Funded by hardware sales | free | Subsidised by Zimaboard / Zimablade |
| Runtipi | Donations only | free | No funding; not a living wage |

The pattern across the winners is consistent: **the software is free, and the
money sits in the layer around it** — hosting, signing, support, verification,
convenience. Nobody in this category succeeds by making the software itself
proprietary, and the two fully funded projects got there through hardware.

## A concrete first year

Combining paths 1, 3 and 5 — the three that need nothing new built.

| Line | Amount |
| --- | --- |
| Microsoft Store — 250 sales @ $9, 100% retained | $2,250 |
| GitHub Sponsors — 8 sponsors averaging $5/mo | $480 |
| Recipe / setup work — 4 engagements @ $250 | $1,000 |
| Apple Developer Program | −$99 |
| Windows code signing — SignPath Foundation | $0 |
| Domain, CI overage, misc. | −$150 |
| **Net, year one** | **≈ $3,481** |

Assumes v1.0 ships verified, one r/selfhosted launch post, and no paid
marketing. Store volume is the most sensitive input — 250 sales implies roughly
25,000 listing views at 1% conversion. Path 2 is excluded; it belongs to year
two.

Three and a half thousand dollars is not a salary. It is proof the thing
converts, which is what year one is for — and roughly where Coolify was before
the curve bent.

## Sequence

### Days 1–30 — cost $0

- Close the two unchecked v0.5 gates: clean-machine installer and
  live-container smoke tests. Do not charge for software whose own README says
  verification is pending.
- Add `FUNDING.yml` and GitHub Sponsors tiers — 0% fee, one afternoon.
- **Apply to SignPath Foundation.** Free, no geographic restriction, approval
  takes days to weeks — start it before it is needed, and while the project is
  still unambiguously 100% open source.
- Put the positioning line above the fold in the README: one binary, no
  Electron, no account, no telemetry.
- Record a 40-second capture: paste an address, click Open, a native window
  appears. That is the whole pitch and it demos in seconds.

### Days 31–60 — cost ~$99

- Tag `v1.0.0`. Post to r/selfhosted and r/homelab — 1.9M combined members, and
  "no Electron" is a headline in both.
- Register the free Microsoft Store developer account and submit. This is the
  only route that removes the SmartScreen warning outright rather than waiting
  it out.
- Wire SignPath into the release workflow so signed Windows artifacts start
  accruing publisher reputation from the first tagged build, not the tenth.
- Buy the Apple Developer membership ($99/yr) — covers signing and notarisation
  with no per-app fee.
- Submit to awesome-selfhosted. This project already consumes their data; be
  listed in it.

### Days 61–90 — cost 5% + $0.50/txn

- Price the Store build at $9. Keep the GitHub release free and say so plainly
  on the listing — the goodwill outperforms the lost sale.
- Set up Polar or Lemon Squeezy as merchant of record for direct sales. They
  handle VAT and GST across jurisdictions, which matters selling globally from
  outside the US or EU.
- Open a recipe-request issue template. Free requests go on the backlog; paid
  requests get scheduled.
- Instrument nothing. The no-telemetry stance is a genuine differentiator —
  measure with Store and GitHub download counts instead.

### Month 4+

- Decide the open-core question deliberately, not by accident — a proprietary
  Pro build costs the free SignPath certificate and ~$219–300/yr to replace it.
- If proceeding: gate backup/restore, update management, secrets vault and
  multi-machine sync behind a perpetual-fallback licence at $39–59/yr.
- Push roadmap phase 4 to 25 verified apps — the entry ticket for the
  verification subscription in path 4.
- Revisit only if the first 90 days produced real users. If they did not, the
  fix is distribution, not a better price.

## What not to do

| | |
| --- | --- |
| **Do not gate the catalog** | 1,612 of 1,672 entries carry GPL-3.0 or CC-BY-SA-3.0 obligations that `THIRD_PARTY_NOTICES.md` already acknowledges. This is the single hard constraint. |
| **Do not add telemetry or affiliate links** | "No account, no telemetry, no analytics" is the strongest sentence in the README. In this audience it is worth more than the ad revenue, and it cannot be un-broken. |
| **Do not relicense away from MIT** | Possible — the launcher copyright is held here. But community trust is the input to every path above, and a rug-pull at 0 stars buys nothing while costing the one asset that cannot be rebuilt. |
| **Do not chase VC** | No traction, no team, and a market whose successful independents are explicitly unfunded. |
| **Do not build hardware** | It works for Umbrel and CasaOS because they carry inventory, supply chains and capital. Local Store's premise is that it runs on the computer the user already owns. |
| **Do not charge before v1.0 verifies** | A refund thread on r/selfhosted is far more expensive than a delayed launch. |

## Sources

- [SmartScreen reputation for Windows app developers](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation) — Microsoft, revised August 2026
- [Artifact Signing FAQ](https://learn.microsoft.com/en-us/azure/artifact-signing/faq) — country eligibility
- [SignPath Foundation conditions](https://signpath.org/terms.html) and [programme overview](https://signpath.io/solutions/open-source-community)
- [SSL.com Individual Validation code signing](https://www.ssl.com/products/software-integrity/code-signing/iv/)
- [EV certificates no longer grant immediate reputation](https://www.todesktop.com/blog/posts/windows-apps-psa-ev-certs-do-not-grant-immediate-reputation-anymore)
- [Free individual registration, Microsoft Store](https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/)
- [Free company registration, May 2026](https://blogs.windows.com/windowsdeveloper/2026/05/07/publish-to-microsoft-store-as-a-company-now-with-free-registration-and-faster-onboarding/)
- [Coolify monthly income](https://x.com/heyandras/status/1901894087604916396), [pricing](https://coolify.io/pricing) and [Open Collective](https://opencollective.com/coollabsio)
- [WebCatalog pricing tiers, 2026](https://costbench.com/software/knowledge-management/webcatalog/)
- [Cloudron pricing](https://www.cloudron.io/pricing.html)
- [Beekeeper Studio pricing](https://www.beekeeperstudio.io/pricing/) and [Ultimate/GPL split](https://www.beekeeperstudio.io/blog/ultimate-and-gpl)
- [Krita licensing and paid store builds](https://krita.org/en/about/license/)
- [Apple notarisation and the $99 programme](https://developer.apple.com/forums/thread/121113)
- [Polar.sh merchant-of-record review, 2026](https://fungies.io/polar-sh-review-2026-2/)
- [Self-hosting subreddit sizes](https://thehiveindex.com/topics/self-hosting/platform/reddit/)
- [CasaOS vs Umbrel vs Runtipi funding comparison](https://doesmycode.work/posts/casaos-vs-umbrel-vs-runtipi/)
- [Umbrel funding profile](https://www.cbinsights.com/company/umbrel)
