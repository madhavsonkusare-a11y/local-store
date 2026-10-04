# What this project learned the hard way

> Source reference reviewed October 4, 2026. The project is paused and the installed development app/engine were removed. Scope and accepted proof remain in [V1_TASKS.md](V1_TASKS.md); resume context is in [agent-handoff.md](agent-handoff.md).

Things that cost real time, found by running the code rather than reading it.
Each one bit at least once and several bit repeatedly. If you are about to
touch the plan model, the importers or anything that talks to Docker, this is
the page worth reading first.

## Windows canonical paths cannot be mounted

`Path::canonicalize` on Windows returns an extended-length path,
`\\?\D:\Data\...`. Docker Compose builds a bind-mount source out of whatever it
is given, and that form makes it refuse the whole file with **"too many
colons"** — the drive colon inside the prefix is read as a mount separator.

It is worth canonicalizing anyway: that is what stops a path merely *pointing*
at somewhere refused. So resolve first, then strip the prefix before the path
reaches Docker or the registry.

This bit three times in one day:

1. Adoption started an app from a canonicalized Compose path and failed.
   Crucially `compose down` never resolves mounts, so `discard` had always
   worked and the bug looked like it was in adoption.
2. The path was also being written into the registry, which would have made the
   adopted app fail to start for the rest of its life rather than once.
3. A shared folder is a mount source too, and `docker_path` had been rewritten
   with a three-character prefix instead of four — silently disarming it.

`crate::folders::docker_path` is the one implementation. Use it; do not write
another.

## "Is this declared value used?" must look at mounts

Three separate places asked whether a declared setup field was referenced, and
all three looked only at environment values:

- `PlanTemplate::validate` called a folder field declared-but-unused.
- The Runtipi importer's field filter dropped it, then refused the template for
  referring to a key nothing declared.
- Earlier, the same shape dropped a time-zone field as inert, leaving an app in
  UTC with no way to change it.

A value can reach a container through an environment entry **or** a mount
source. Anything answering "is this used?" has to consider both.

## A guard has to compare at the right granularity

The check that stops a reinstall running over unrecognised files compared the
directory listing against each *full* declared data path. An app keeping its
data in `data/.ollama` puts a single `data` directory on disk, so the listing
and the list never matched and **every keep-data reinstall of any app with a
nested data path was refused**. It would have hit a real person on their second
install of Ollama or Metabase.

Compare what is actually on disk against what would actually be on disk.

## Timeouts tuned for the simple case fail the real one

The first-start health wait was sixty seconds. That is fine for something that
only has to open a port, and not enough for WordPress running its own
installer or Metabase initialising a schema. Both were rolled back **while
still working**.

Giving up early throws away a working app; waiting longer costs time that is
bounded, visible and cancellable. Prefer the second.

## Our own rules can be stricter than the thing they model

The plan required every environment name to be `NAME_LIKE_THIS`. Uppercase is a
convention, not a rule: Ghost configures itself with `database__client`,
Jellyfin with `JELLYFIN_PublishedServerUrl`. Ten apps were refused for a house
style wearing a safety rule's clothes.

Before refusing something, check whether the thing you are modelling refuses
it. Then write the check against what actually matters — here, that a name
cannot break out of the line it renders into.

## Sources do not agree on format

CapRover ships YAML; its importer takes JSON, and the import report converts in
Python. A batch runner that read the files directly reported four of eight apps
as "no longer importable" when they were fine.

Normalise at extraction, once, where the conversion is already understood.

## Evidence must be derived, not declared

Two evidence files claimed things that had stopped being true:

- One said `browser_first_use: "not_tested"` while the test ran the browser
  probe three times. The string was left over from a run that predated those
  calls.
- Two said `promotion: "withheld"` after their apps were approved.

Both now read from the thing they describe. A related trap: a check that passes
*trivially* must say so. The credential step reported "keeps the credentials it
generated" for apps that generate none — true, and useless, and
indistinguishable from the real thing.

## Measure before choosing, and check the measurement

Ranking candidates by Docker Hub pull count scored every app that ships a
Postgres or Redis sidecar identically, in the billions, because the first image
in a definition is often not the app. Falling back to the least-pulled image did
the same thing more quietly for apps on ghcr.io, which publishes no pull count
at all. A pull count now counts only when the image is identifiably the app's
own: **borrowing a database's popularity is worse than having no number.**

The same mistake came back in a different place. The batch skipped any
definition that *contained* an infrastructure image, so every app shipping its
own Postgres or MariaDB — WordPress, Nextcloud, Joplin, Monica, Guacamole, fifty
in all — was passed over as "not an app". The rule now asks whether *every*
image is infrastructure. **An app is what it is, not what it stores its data
in.**

Also: `NOASSERTION` from GitHub means "no licence file detected", not "not open
source". Reading it as a refusal excluded WordPress and qBittorrent, both GPL.

And a cache keyed on the wrong field never refills: stars and licence came from
one call, but the skip was keyed on stars, so every project cached before
licences were collected kept a null licence permanently.

## Rank orders apps, not the definitions of one app

When two sources package the same app, the batch ran whichever ranked higher.
For Grocy and Tautulli that was CapRover, whose definitions pin images from
2020 and 2021; Runtipi's pinned the releases upstream published that week. Both
qualified either way — the old images work — and both would have been withheld
for their age.

A qualification result proves one definition. Choose the packaging before
proving it (`--only app --source runtipi`), and make anything that consumes the
proof check it is about the same definition: the template generator now reads
the source from the result and refuses a mismatch.

## A proof has to be about what runs

Grafana was approved and offered on a proof of `grafana/grafana:7.4.3` —
CapRover's definition, from 2021 — while the template installs Runtipi's
`grafana-oss:13.0.2`. The app people would have installed had never been run.
The guard only checked that the proof file *existed*.

It now requires the proof's images to equal the images the approved template
runs, and the proof to have passed. An image pin changes what runs without
changing which file the review points at, so after a pin the app has to be
qualified again *as offered* — `qualify_batch --offered --only app` runs the
reviewed mapping, pins and all.

## Two harnesses at once make each other's proofs worthless

The bystander check fails if any container that is not the run's own
disappears. A batch and a second run started side by side each saw the other's
cleanup as a stranger's containers being removed. Qualification now takes a
machine-wide lock; a second run waits for its turn.

## A timeout carries no reason, and the reason is about to be deleted

Five candidates in a row failed with "Health check timed out" and nothing else.
Every other install failure explains itself; a timeout only reports that time
passed. The explanation is in the containers' state and logs — and a failed
install rolls back, removing exactly those containers.

A timed-out install now reads `ps` and the log tail *before* cleaning up and
carries them in the error. That helps a person whose install failed as much as
it helps a batch.

## Registries say how to authenticate; ask them

The image audit refused anything not on Docker Hub, which blocked 89 images.
Docker Hub, ghcr.io and quay.io each put their token endpoint somewhere
different, and lscr.io hands callers to ghcr.io. Rather than special-casing
each, request unauthenticated, read the `WWW-Authenticate` challenge, and fetch
the token it names. Read the build date from the image config — `created` — not
a push date, because "last rebuilt" is what a promotion decision turns on.

And a registry answering 429 has said how long to wait. Waiting is not failing.

Docker Hub counts a manifest **GET** as a pull against an anonymous limit of a
few per hour, and a day of batch runs spends it. A **HEAD** returns the same
`Docker-Content-Digest` and is not counted, so resolve digests with HEAD; and
prefer `docker pull`, which uses the engine's login and its larger allowance,
over anonymous requests when an image has to be fetched.

## Upstream's timings assume upstream's storage

Penpot's official Compose gives Postgres about twelve seconds to pass its
health check. That suits a Docker volume. Local Store keeps data in a folder on
the host, where the same first start took nineteen seconds, and the install
failed with the database "unhealthy". A long start period costs nothing when
the start is fast — the first success ends it — so give one.

## A health check has to ask the way a browser does

The health probe sent `Host: localhost` for `http://localhost:<port>`. HTTP
wants the port there, every browser sends it, and Joplin routes on the whole
Host — so it answered 404 to the probe while serving `/login` to everyone else,
and was rolled back as unhealthy for weeks. Reproducing by hand, with the exact
request the probe makes, found it in one step.

## A definition is more than its Compose file

Runtipi ships starting files beside some definitions and copies them into the
app's data folder on install. Taking only the definition left Notemark's proxy
mounting a file that was not there — Docker made a directory, nginx refused to
start — and Glance with no `glance.yml`. Forty-one candidates ship such files.

## "No licence detected" still has to be read

`NOASSERTION` does not mean closed, which is why it is not a refusal — but it
does not mean open either. Joplin's repository is AGPL, except `packages/server`,
which is under a non-commercial personal-use licence; the server is what we
would ship. Check the licence of the part you run.

## A probe that only opens the first page proves only the first page

Maxun's frontend is static files: it answered immediately while its backend
was still starting, and the standard check passed an app whose every action
would have failed. The check that caught it asks each of an app's other
published ports to answer. Anything an app's own pages call is worth probing;
"it opened" is not the same as "it works".

## An upstream image can simply go away

LobeHub's bucket was made by MinIO's `mc`, whose tags the registry API still
listed while `docker pull` answered "repository does not exist": MinIO has
stopped publishing it. The bucket is now made by a signed S3 request from
LobeHub's own start script — no extra image, nothing to disappear. MinIO's
server images stopped in September 2025 too, which is why two apps here use
RustFS.

## Pulling by digest does not restore a tag

After the images an app was proven with were removed, re-pulling
`image:tag@sha256:...` brought the data back but left nothing named
`image:tag`, so the pin check found no local image and called every one of
Dify's fifteen a mismatch. Pull the tag as well before checking pins.

## An app's version is not always its image's tag

Dify's front door is an nginx, and the store reported `1.31.5-alpine` as
Dify's version because it read the tag of the image the plan publishes. Where
a reviewed definition names the app's own version, that is the one to show.

## A full disk corrupts images silently

D: filled while qualification pulled large AI images, and Docker went down.
After recovery, Vane's image failed with `exec format error`: its
`docker-entrypoint.sh` was 0 bytes locally, while the same layer in the
registry holds 388. Two pulls that timed out during the full-disk window had
left temporary leases pinning half-written snapshots, and every later pull —
even of the official `node` image — reused them instead of unpacking again.
Removing the two dead leases (`ctr -n moby leases rm`, via a throwaway
`docker:dind` with containerd's socket mounted) and re-pulling fixed it.

After any disk-full event, suspect every image unpacked during it, not just
the one that crashed.

## Freeing space inside Docker's disk does not give it back

Removing images took Docker's own filesystem from 83 GB to 26 GB, but its
virtual disk on D: kept growing — 81 GB, then 88 GB — because new writes went
to fresh regions rather than the freed ones. Only compacting the file returns
space to the drive. Qualification now needs a disk guard, and builds live on a
different drive (CARGO_TARGET_DIR) from Docker's disk.

## MySQL 8 cannot keep data on a Windows host folder

It sees a case-insensitive filesystem, sets `lower_case_table_names=2`, and
then refuses its own data dictionary. Huginn's all-in-one image failed this
way. MariaDB copes and Postgres copes; use one of them.

## The binary proves the manifest it was built with

Manifests are compiled into the batch binary. One built 21 seconds before two
manifests were regenerated proved the old definitions, and both failed for
reasons already fixed. An offered run now refuses when the compiled manifest
differs from the file on disk.

## Prove a test can fail

Every claim worth making has been checked by reintroducing the bug it guards
against — a destructive keep-data uninstall, a review that says a password is
not sensitive, a removed platform placeholder. A test that has never failed has
not been shown to test anything.

## A refusal is a finding, not a failure

The review mechanism records "no" as readily as "yes". CodiMD is withheld
because its images were last rebuilt in 2020; Planka because it is proprietary,
pins a release a year old, runs its database with `trust` authentication, and
uses a floating tag. Writing that down is more useful than a longer catalogue.

## Qualifying apps in bulk costs system-drive space permanently

Docker Desktop on Windows keeps its images in a virtual disk under
`%LOCALAPPDATA%`, on C:. That file **only grows**. Deleting images frees space
*inside* it and returns nothing to the drive, and compacting it needs
Administrator rights.

A batch that pulls a hundred images therefore consumes tens of gigabytes of the
system drive whatever it cleans up afterwards. On this machine it took C: from
6 GB to 331 MB, at which point Docker could no longer create its own sockets and
crashed on every start with "The file cannot be accessed by the system" — an
error that looks nothing like "the disk is full".

Two things that follow:

- **Check free space before a long batch, not after.** An option added to make
  re-runs fast (`--keep-images`) is the same option that fills a drive.
- **Move Docker's disk off the system drive.** Stop Docker, `wsl --shutdown`,
  move `%LOCALAPPDATA%\Docker\wsl\disk` to another drive, and junction the old
  path to the new one:

      New-Item -ItemType Junction -Path "$env:LOCALAPPDATA\Docker\wsl\disk" -Target "D:\DockerData\disk"

  No elevation, no data loss, and every image and container survives. That
  turned 0.3 GB free into 47 GB here.

Recovering a Docker Desktop that will not start, in order: quit it and
`com.docker.backend`, remove or rename `%LOCALAPPDATA%\Docker\run` and
`%LOCALAPPDATA%\docker-secrets-engine` (their socket files become undeletable
when the disk fills), `wsl --shutdown`, then start Docker Desktop and let *it*
boot the VM — starting the distro by hand leaves the data disk unmounted and
every API call returns 500.

## What the batch is for

Running candidates in bulk found three real defects in one afternoon — the
nested data path, the sixty-second timeout, and the YAML that could not be
read. None was visible from reading the code. If a change claims to unlock
apps, run the apps.
