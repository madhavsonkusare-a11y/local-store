# Reviewed recipe requirements

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

The three original reviewed recipes use schema 3. Each manifest declares a Linux Docker
engine, Compose v2, local persistent storage and the platforms published for its
pinned container image. These describe **container compatibility**, not tested
Windows/macOS/Linux installers or completed lifecycle verification. The launcher
does not benchmark resource needs or guarantee a minimum RAM/disk allocation.
Docker's platform selection remains authoritative at installation time.

| Recipe | Pinned image | Published container platforms | Metadata source |
| --- | --- | --- | --- |
| Memos | `neosmemo/memos:0.30.0` | `linux/amd64`, `linux/arm/v7`, `linux/arm64` | [Docker Hub tag](https://hub.docker.com/v2/repositories/neosmemo/memos/tags/0.30.0) |
| n8n | `docker.n8n.io/n8nio/n8n:2.37.10` | `linux/amd64`, `linux/arm64` | [Docker Hub tag](https://hub.docker.com/v2/repositories/n8nio/n8n/tags/2.37.10) |
| Uptime Kuma | `louislam/uptime-kuma:2.5.3` | `linux/amd64`, `linux/arm/v7`, `linux/arm64` | [Docker Hub tag](https://hub.docker.com/v2/repositories/louislam/uptime-kuma/tags/2.5.3) |

Metadata was checked September 7, 2026. n8n's official registry hostname uses
Docker Hub's authentication service and `n8nio/n8n` namespace. Registry access
returned a pull-rate limit; its public tag listing provided the recorded
metadata without downloading image layers. Entries with `unknown/unknown`
platforms are attestations and are excluded from the CPU list.

`catalog/recipe-platforms/*.json` preserves the upstream tag responses.
`requirements.image_audit` stores the source URL, review date and image-index
digest. `python scripts/check-recipe-platforms.py` compares each declaration to
that cache offline; CI also runs `scripts/test_recipe_platforms.py`. A changed
tag, source, digest or platform list fails the check.

To inspect an upstream change deliberately:

```text
python scripts/check-recipe-platforms.py --refresh
```

Refresh requests are limited to tag metadata, with a 30-second timeout and
1 MiB response cap. They require network access but no Docker daemon. Refresh
updates the cache, then compares it to the existing declaration: it does not
silently approve changed upstream images. Review the diff before changing the
manifest's digest, platforms or review date. Compose installs the exact reviewed `image:tag@sha256:...` reference.
The validator rejects missing, changed or mismatched digest pins.

The Rust schema rejects missing requirements, old schemas, unknown fields,
unsupported/duplicate platforms and malformed audit evidence. Address checks
parse the actual loopback host and published port; `52300` cannot satisfy a
`5230` declaration. Republish changes only the authority port, preserving health
paths/queries and all requirement metadata. Docker's own Compose parser remains
the final configuration check through `scripts/validate-recipes.py`.

Local storage follows the existing recipes: Memos and Uptime Kuma bind their
managed `data/` directories; n8n uses its named Docker volume. Arbitrary external
data locations remain unsupported by Local Store's deletion ownership guard.
Upstream [Memos deployment instructions](https://usememos.com/docs/deploy/docker-compose)
describe persistent storage; [Uptime Kuma's versioned instructions](https://github.com/louislam/uptime-kuma/blob/2.5.3/README.md#-how-to-install)
explicitly exclude NFS. Historical lifecycle results are recorded in
[recipe-lifecycle-proof.md](recipe-lifecycle-proof.md); they do not certify a
new engine, app version or release candidate.
