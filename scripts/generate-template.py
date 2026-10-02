"""Assemble a reviewed template from a candidate that passed qualification.

Writing one of these by hand takes about seventy lines, most of which are
copied from somewhere: the identity from the catalog, the provenance from the
queue, the image audit from the registry, the field list from the importer.
Doing that four times was the sign it should be generated.

What is generated is everything mechanical. What is left blank, deliberately,
is everything that is a judgement:

* `promotion.state` is `withheld` with a reason saying nobody has decided yet.
* Every setup field gets a review entry copying the importer's guess about
  whether it is a credential, marked `REVIEW:` so it cannot be mistaken for a
  decision somebody made.
* `risk_notes` gets what can be derived — how many containers, whether a
  folder is shared, whether credentials are generated — and a `REVIEW:` line
  for whatever is specific to the app.

A manifest still needs a person to read it before it is offered. This just
means they start from the facts rather than from an empty file.

    python scripts/generate-template.py <app-id>
"""

import argparse
import datetime
import hashlib
import json
import os
import re
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REVIEW = "REVIEW: "


USER_AGENT = {"User-Agent": "Local-Store-template"}
# Every manifest shape a registry might answer with. Omitting the OCI ones gets
# a v1 manifest back from registries that still keep one, which lists no
# platforms at all.
MANIFEST_TYPES = ", ".join(
    (
        "application/vnd.oci.image.index.v1+json",
        "application/vnd.oci.image.manifest.v1+json",
        "application/vnd.docker.distribution.manifest.list.v2+json",
        "application/vnd.docker.distribution.manifest.v2+json",
    )
)


def opened(request, attempts=5):
    """urlopen, waiting out a registry that says it is being asked too often.

    ghcr.io answers a burst of anonymous requests with 429 and a Retry-After.
    Treating that as a refusal fails a manifest over nothing; the registry has
    said exactly how long to wait.
    """
    for attempt in range(attempts):
        try:
            return urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            if error.code != 429 or attempt == attempts - 1:
                raise
            wait = error.headers.get("Retry-After", "")
            delay = int(wait) if wait.isdigit() else 2 ** (attempt + 2)
            time.sleep(min(delay, 120))


def fetch(url):
    request = urllib.request.Request(url, headers=USER_AGENT)
    with opened(request) as response:
        return json.loads(response.read(2 * 1024 * 1024))


def split_image(image):
    """(registry host or None, repository, tag) for one image reference."""
    name, _, tag = image.rpartition(":")
    if not name:  # no tag at all; the guard test refuses those anyway
        name, tag = image, "latest"
    head, slash, rest = name.partition("/")
    if slash and ("." in head or ":" in head or head == "localhost"):
        return head, rest, tag
    return None, (name if slash else f"library/{name}"), tag


def registry_get(host, path, accept=None):
    """One authenticated GET against a registry, answering its own challenge.

    Registries disagree about where tokens come from — Docker Hub, ghcr.io and
    quay.io all use different URLs, and lscr.io hands its callers to ghcr.io.
    Asking unauthenticated first and reading `WWW-Authenticate` is how the
    protocol says to find out, and it means a registry nobody anticipated works
    without a special case here.
    """
    url = f"https://{host}{path}"
    headers = dict(USER_AGENT)
    if accept:
        headers["Accept"] = accept
    try:
        with opened(urllib.request.Request(url, headers=headers)) as response:
            return response.read(8 * 1024 * 1024)
    except urllib.error.HTTPError as refusal:
        if refusal.code != 401:
            raise
        challenge = refusal.headers.get("WWW-Authenticate") or ""
    if not challenge.lower().startswith("bearer "):
        raise SystemExit(f"{host}: asks for {challenge.split(' ')[0]}, which needs a login")
    fields = dict(re.findall(r'(\w+)="([^"]*)"', challenge))
    realm = fields.pop("realm", None)
    if not realm:
        raise SystemExit(f"{host}: sent an authentication challenge naming no token endpoint")
    query = urllib.parse.urlencode(fields)
    grant = fetch(f"{realm}?{query}" if query else realm)
    token = grant.get("token") or grant.get("access_token")
    if not token:
        raise SystemExit(f"{host}: its token endpoint returned no token")
    headers["Authorization"] = f"Bearer {token}"
    with opened(urllib.request.Request(url, headers=headers)) as response:
        return response.read(8 * 1024 * 1024)


def registry_digest(host, repository, reference):
    """The digest a registry serves for a tag or digest, asked with HEAD.

    Docker Hub counts a manifest GET as a pull against an anonymous limit of a
    few per hour, and a long batch spends it; a HEAD is free and carries the
    same Docker-Content-Digest header. Returns None when the reference does not
    exist.
    """
    url = f"https://{host}/v2/{repository}/manifests/{reference}"
    headers = {**USER_AGENT, "Accept": MANIFEST_TYPES}
    for attempt in range(2):
        try:
            with opened(urllib.request.Request(url, headers=headers, method="HEAD")) as response:
                return response.headers.get("Docker-Content-Digest")
        except urllib.error.HTTPError as refusal:
            if refusal.code == 404:
                return None
            if refusal.code != 401 or attempt:
                raise
            challenge = refusal.headers.get("WWW-Authenticate") or ""
        fields = dict(re.findall(r'(\w+)="([^"]*)"', challenge))
        realm = fields.pop("realm", None)
        if not realm:
            raise SystemExit(f"{host}: sent an authentication challenge naming no token endpoint")
        grant = fetch(f"{realm}?{urllib.parse.urlencode(fields)}")
        headers["Authorization"] = f"Bearer {grant.get('token') or grant.get('access_token')}"
    return None


def index_digest(image):
    """The manifest-list digest a tag points at now — what an install pins."""
    host, repository, tag = split_image(image)
    return registry_digest(host or "registry-1.docker.io", repository, tag)


def registry_audit(host, repository, tag, image):
    """The same audit for any registry that speaks the distribution API.

    Docker Hub's own API reports when a tag was last *pushed*. Other registries
    publish no such field, so this reads the build date out of the image's own
    config — which is what "last rebuilt" actually means, and is the number a
    promotion decision turns on.
    """
    body = registry_get(host, f"/v2/{repository}/manifests/{tag}", MANIFEST_TYPES)
    manifest = json.loads(body)
    platforms = {}
    if "manifests" in manifest:
        for entry in manifest["manifests"]:
            platform = entry.get("platform") or {}
            # Attestations and signatures ride alongside the real images and
            # describe no platform. Counting them would claim architectures the
            # app cannot run on.
            if platform.get("os") in (None, "unknown"):
                continue
            if platform.get("architecture") in (None, "unknown"):
                continue
            key = "/".join(
                part
                for part in (platform["os"], platform["architecture"], platform.get("variant"))
                if part
            )
            platforms[key] = entry["digest"]
        if not platforms:
            raise SystemExit(f"{image}: the registry lists no platforms for this tag")
        # Every architecture of a tag is built together, so one config carries
        # the build date for all of them.
        first = next((key for key in sorted(platforms) if key.endswith("amd64")), None)
        child = json.loads(
            registry_get(
                host,
                f"/v2/{repository}/manifests/{platforms[first or sorted(platforms)[0]]}",
                MANIFEST_TYPES,
            )
        )
    else:
        # A single-platform image is its own manifest and names no platform, so
        # the only honest answer comes from its config rather than a default.
        child = manifest

    config = json.loads(registry_get(host, f"/v2/{repository}/blobs/{child['config']['digest']}"))
    if not platforms:
        if not config.get("os") or not config.get("architecture"):
            raise SystemExit(f"{image}: names no platform anywhere, so portability is unknown")
        key = "/".join(
            part
            for part in (config["os"], config["architecture"], config.get("variant"))
            if part
        )
        # A content digest is the hash of the manifest that was served, so
        # there is no need to ask the registry what it calls this tag.
        platforms[key] = "sha256:" + hashlib.sha256(body).hexdigest()

    created = (config.get("created") or "")[:10]
    if not created:
        raise SystemExit(f"{image}: its config records no build date, so staleness cannot be shown")
    return {
        "image": image,
        "source_url": f"https://{host}/v2/{repository}/manifests/{tag}",
        "checked_at": datetime.date.today().isoformat(),
        "last_updated": created,
        "container_platforms": sorted(platforms),
        "digests": dict(sorted(platforms.items())),
    }


def hub_repository(image):
    host, repository, _ = split_image(image)
    return None if host else repository


def image_audit(image):
    """Digests and rebuild date for one image, from the registry itself."""
    host, repository, tag = split_image(image)
    if host:
        return registry_audit(host, repository, tag, image)
    url = f"https://hub.docker.com/v2/repositories/{repository}/tags/{tag}"
    data = fetch(url)
    platforms = {}
    for entry in data.get("images", []):
        if entry.get("os") in (None, "unknown") or entry.get("architecture") in (None, "unknown"):
            continue
        key = "/".join(
            part for part in (entry["os"], entry["architecture"], entry.get("variant")) if part
        )
        platforms[key] = entry["digest"]
    if not platforms:
        raise SystemExit(f"{image}: the registry lists no platforms for this tag")
    return {
        "image": image,
        "source_url": url,
        "checked_at": datetime.date.today().isoformat(),
        "last_updated": data["last_updated"][:10],
        "container_platforms": sorted(platforms),
        "digests": dict(sorted(platforms.items())),
    }


def facts_for(candidate):
    """Ask the importer what this definition actually becomes."""
    provenance = candidate["provenance"]
    path = ROOT / ".cache" / "definitions" / provenance["revision"] / provenance["path"]
    if path.suffix in (".yml", ".yaml"):
        path = path.with_suffix(".json")
    if not path.is_file():
        raise SystemExit(f"{path} is missing; run scripts/extract-definitions.py first")
    # Prefer the built binary. Going through `cargo run` takes the build lock,
    # which fights a batch running in another window and fails to link.
    # Builds may live off this drive (CARGO_TARGET_DIR), so Docker's disk and
    # cargo's output do not fill the same one.
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    built = [
        target / "release" / "examples" / "template_facts.exe",
        target / "debug" / "examples" / "template_facts.exe",
        ROOT / "target" / "release" / "examples" / "template_facts.exe",
        ROOT / "target" / "debug" / "examples" / "template_facts.exe",
        ROOT / "target" / "release" / "examples" / "template_facts",
        ROOT / "target" / "debug" / "examples" / "template_facts",
    ]
    binary = next((path for path in built if path.is_file()), None)
    command = (
        [str(binary)]
        if binary
        else ["cargo", "run", "--quiet", "--locked", "--example", "template_facts", "--"]
    )
    done = subprocess.run(
        command + [candidate["source"], candidate["id"], str(path)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if done.returncode != 0:
        raise SystemExit(done.stderr.strip() or "the importer refused this definition")
    return json.loads(done.stdout)


def verify_candidate_source(candidate):
    """Require cached mapping inputs to match the checksum-pinned archive."""
    provenance = candidate['provenance']
    source = candidate['source']
    pin = json.loads((ROOT / 'catalog/import-audit-sources.json').read_text())[source]
    if any(provenance.get(key) != pin.get(key) for key in ('repository', 'revision')) or provenance.get('archive_sha256') != pin.get('sha256'):
        raise SystemExit('candidate provenance differs from pinned import source')
    archive = ROOT / '.cache/catalog' / f"{source}-{pin['revision']}.zip"
    if hashlib.sha256(archive.read_bytes()).hexdigest() != pin['sha256']:
        raise SystemExit('candidate archive checksum mismatch')
    path = ROOT / '.cache/definitions' / pin['revision'] / provenance['path']
    with zipfile.ZipFile(archive) as bundle:
        def read(relative):
            members = [item for item in bundle.infolist() if item.filename.split('/', 1)[-1] == relative]
            if len(members) != 1 or members[0].file_size > 1024 * 1024:
                raise SystemExit('candidate definition missing, duplicate or oversized')
            return bundle.read(members[0])
        original = read(provenance['path'])
        if source == 'caprover':
            import yaml
            if yaml.safe_load(original) != json.loads(path.with_suffix('.json').read_text()):
                raise SystemExit('extracted candidate differs from pinned archive')
        elif original.decode().strip() != path.read_text().strip():
            raise SystemExit('extracted candidate differs from pinned archive')
        if source == 'runtipi':
            config_path = str(Path(provenance['path']).parent / 'config.json').replace('\\', '/')
            if read(config_path).decode().strip() != path.with_name('config.json').read_text().strip():
                raise SystemExit('candidate configuration differs from pinned archive')


def reviewed_image_pins(values, images):
    pins = []
    for value in values:
        original, separator, replacement = value.partition('=')
        if not separator or original not in images or original == replacement or any(pin['definition'] == original for pin in pins):
            raise SystemExit('image pin must name one actual image and a different tag')
        left, right = split_image(original), split_image(replacement)
        if left[:2] != right[:2] or not right[2] or right[2] == 'latest':
            raise SystemExit('image pin may change a concrete tag, not its image repository')
        pins.append({'definition':original, 'replacement':replacement, 'reason':REVIEW + 'review release compatibility and qualify this exact pinned replacement before promotion.'})
    return pins


def data_storage(facts, app):
    """Where this app's data actually lives, in the words the review shows."""
    if facts["managed_directories"]:
        where = f"Local Store managed folder / {app}"
    elif facts.get("named_volumes"):
        where = f"A Docker volume Local Store manages for {app}"
    else:
        where = "None: it keeps no data of its own"
    if facts["shared_folders"]:
        where += ", plus a folder you choose"
    return where


def risk_notes(facts, catalog):
    notes = []
    services = facts["services"]
    ports = 1 + len(facts.get("companion_ports", []))
    counted = "one port" if ports == 1 else f"{ports} ports"
    notes.append(
        f"Creates {services} Docker container{'s' if services != 1 else ''} and publishes "
        f"{counted} on this computer's loopback interface only."
    )
    # Said, because nobody would guess it: the app's own pages call these
    # directly, so they are addresses on this computer like the main one.
    for companion in facts.get("companion_ports", []):
        notes.append(
            f"Its {companion['service']} service answers on a second loopback address, which "
            "the app's own pages call directly. Local Store opens only the main one."
        )
    jobs = facts.get("jobs", [])
    if jobs:
        named = ", ".join(jobs)
        notes.append(
            f"Runs {named} once each time it starts, and starts the app only if "
            f"{'it succeeds' if len(jobs) == 1 else 'they succeed'}."
        )
    if facts["shared_folders"]:
        notes.append(
            "Reads and writes a folder on this computer that you choose during setup. Nothing "
            "outside that folder is shared with it."
        )
    if facts["generated_credentials"]:
        count = facts["generated_credentials"]
        notes.append(
            f"Generates {count} credential{'s' if count != 1 else ''} of its own, keeps "
            f"{'them' if count != 1 else 'it'} beside the app's data, and reuses "
            f"{'them' if count != 1 else 'it'} if you reinstall."
        )
    # Said from what the plan mounts, not assumed: Adminer and Whoogle keep
    # nothing, and Beszel keeps its data in a named volume, yet all three were
    # once described as keeping it in the managed folder.
    if facts["managed_directories"]:
        notes.append(
            "Keeps its data in this app's managed folder. A keep-data uninstall leaves it; "
            "deleting data removes it permanently."
        )
    elif facts.get("named_volumes"):
        notes.append(
            "Keeps its data in a Docker volume that Local Store manages for it. A keep-data "
            "uninstall leaves the volume; deleting data removes it permanently."
        )
    else:
        notes.append(
            "Keeps no data of its own, so there is nothing for an uninstall to keep or delete."
        )
    notes.append(
        "Proven on Windows with Docker's Linux engine. macOS and Linux hosts are unverified."
    )
    notes.append(
        REVIEW + "say what this app does that somebody should know before installing it, and "
        "remove this line."
    )
    return notes


def upstream_release(source_url):
    """Upstream's latest release, when the project publishes them on GitHub.

    Every promotion so far has turned on one question — is this the release
    upstream is on? — and it was being answered by hand each time. This only
    reports it; deciding what a gap means stays with the reviewer.
    """
    match = re.match(r"https://github\.com/([^/]+/[^/#?]+)", source_url or "")
    if not match:
        return None
    repository = match.group(1).removesuffix(".git")
    try:
        release = fetch(f"https://api.github.com/repos/{repository}/releases/latest")
    except urllib.error.HTTPError:
        return None
    return release.get("tag_name"), (release.get("published_at") or "")[:10]


def catalog_entry(catalog, app):
    """The catalog entry this app is, by whichever name the catalog knows it.

    Upstream ids and catalog ids are written by different people: the catalog
    calls Navidrome `navidrome-music-server`, Runtipi calls it `navidrome`.
    Matching only on the id left a generated manifest with no description,
    category or project link — all of which the catalog already had.
    """
    wanted = app.replace("-", "").replace("_", "").replace(" ", "").casefold()

    def matches(value):
        return value.replace("-", "").replace("_", "").replace(" ", "").casefold() == wanted

    # An id first, then a display name, then an alias. Aliases collide: Open
    # WebUI is an alias of "Ollama With Open Webui", whose entry was returned
    # for `open-webui` — with that app's name, description and icon.
    for pick in (
        lambda entry: matches(entry["id"]),
        lambda entry: matches(entry.get("name", "")),
        lambda entry: any(matches(alias) for alias in entry.get("aliases", [])),
    ):
        for entry in catalog["entries"]:
            if pick(entry):
                return entry
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app")
    parser.add_argument("--source", help="runtipi or caprover, when an app is in both")
    parser.add_argument('--draft', action='store_true', help='emit a withheld proposal without claiming lifecycle proof')
    parser.add_argument('--output', help='new proposal path under catalog/promotion-proposals/ (required with --draft)')
    parser.add_argument('--image-pin', action='append', default=[], help='reviewable same-image tag substitution DEFINITION=REPLACEMENT')
    args = parser.parse_args()
    if not re.fullmatch(r'[a-z0-9][a-z0-9-]{0,79}', args.app):
        raise SystemExit('invalid candidate ID')
    if args.draft != bool(args.output):
        raise SystemExit('--draft requires --output; proof-backed generation keeps its existing output path')
    if args.image_pin and not args.draft:
        raise SystemExit('new image pins require a withheld draft and qualification of that exact reviewed plan')
    target = ROOT / 'src/templates' / f'{args.app}.json'
    if args.draft:
        target = (ROOT / args.output).resolve()
        if not target.is_relative_to((ROOT / 'catalog/promotion-proposals').resolve()) or target.suffix != '.json' or target.exists():
            raise SystemExit('draft output must be a new JSON file under catalog/promotion-proposals/')

    # A manifest for an app nobody has run is a manifest that cannot be
    # completed: the platform gate requires a lifecycle proof, and there is
    # nothing honest to point it at. Requiring the run first also means the
    # generated file is about an app that demonstrably works.
    result_path = ROOT / ".cache" / "qualification" / f"{args.app}.json"
    if not args.draft and not result_path.is_file():
        raise SystemExit(
            f"{args.app} has no qualification result. Run it first with "
            "LOCAL_STORE_RUN_DOCKER_TEST=1 cargo run --release --example qualify_batch"
        )
    result = None if args.draft else json.loads(result_path.read_text(encoding="utf-8"))
    if result is not None and not result.get("passed"):
        failed = next((s for s in result.get("steps", []) if not s["passed"]), {})
        raise SystemExit(
            f"{args.app} did not pass qualification (failed at {failed.get('step')!r}). "
            "Fix that before writing a manifest for it."
        )

    queue = json.loads((ROOT / "catalog" / "candidate-queue.json").read_text(encoding="utf-8"))
    matches = [
        candidate
        for candidate in queue["candidates"]
        if candidate["id"] == args.app
        and candidate["importable"]
        and (args.source is None or candidate["source"] == args.source)
    ]
    if not matches:
        raise SystemExit(f"no importable candidate named {args.app}")
    # An app carried by two sources has two different definitions, and only one
    # of them was run. Picking the other would produce a manifest whose proof is
    # about something else, so let the run decide and only ask when it cannot.
    proven = result.get("source_revision") if result else None
    if proven and len(matches) > 1:
        matches = [c for c in matches if c["provenance"]["revision"] == proven] or matches
    if len({candidate["source"] for candidate in matches}) > 1:
        raise SystemExit(
            f"{args.app} is in more than one source; choose with --source "
            + ", ".join(sorted({candidate['source'] for candidate in matches}))
        )
    candidate = matches[0]
    if proven and candidate["provenance"]["revision"] != proven:
        raise SystemExit(
            f"{args.app} was qualified from revision {proven[:10]} but this candidate comes from "
            f"{candidate['provenance']['revision'][:10]}. Re-run qualification against the source "
            "you want to ship, so the manifest and its proof describe the same definition."
        )

    verify_candidate_source(candidate)
    facts = facts_for(candidate)
    image_pins = reviewed_image_pins(args.image_pin, facts['images'])
    final_images = [next((pin['replacement'] for pin in image_pins if pin['definition'] == image), image) for image in facts['images']]

    catalog = json.loads((ROOT / "src" / "generated" / "catalog.json").read_text(encoding="utf-8"))
    entry = catalog_entry(catalog, args.app)
    ranking = json.loads(
        (ROOT / "catalog" / "candidate-ranking.json").read_text(encoding="utf-8")
    )
    ranked = next((r for r in ranking["candidates"] if r["id"] == args.app), {})

    provenance = candidate["provenance"]
    definition_path = ROOT / ".cache" / "definitions" / provenance["revision"] / provenance["path"]
    if definition_path.suffix in (".yml", ".yaml"):
        definition_path = definition_path.with_suffix(".json")
    definition = definition_path.read_text(encoding="utf-8")
    config_path = definition_path.parent / "config.json"

    manifest = {
        "schema_version": 1,
        "id": args.app,
        "display_name": (entry or {}).get("name") or args.app,
        "catalog_name": (entry or {}).get("name") or args.app,
        "description": (entry or {}).get("description")
        or (REVIEW + "one sentence describing this app"),
        "category": (entry or {}).get("category") or (REVIEW + "a catalog category"),
        "license": ((ranked.get("licenses") or [None])[0]) or (REVIEW + "the upstream licence"),
        "source_url": (entry or {}).get("source_url") or (REVIEW + "the project's repository"),
        "documentation_url": (entry or {}).get("website_url")
        or (entry or {}).get("source_url")
        or (REVIEW + "where its own setup instructions live"),
        "verified_at": datetime.date.today().isoformat(),
        "lifecycle_proof": '' if args.draft else f"docs/evidence/{args.app}-qualification.json",
        "origin": {
            "importer": candidate["source"],
            "repository": provenance["repository"],
            "revision": provenance["revision"],
            "path": provenance["path"],
            "license": REVIEW + "the licence of the definition, not of the app",
        },
        "requirements": {
            "docker_engine_os": "linux",
            "compose_major": 2,
            "local_storage_required": True,
            "images": [
            {**image_audit(image), "index_digest": index_digest(image)}
            for image in final_images
        ],
        },
        "promotion": {
            "state": "withheld",
            "reason": REVIEW
            + "nobody has decided whether to offer this app. Record the decision and why.",
        },
        "data_storage": data_storage(facts, args.app),
        "risk_notes": risk_notes(facts, entry),
        # The importer's guess, marked as a guess. A review says which of these
        # answers is genuinely a credential; upstream cannot.
        "fields": {
            field["key"]: {
                "label": REVIEW + field["upstream_label"],
                "sensitive": field["importer_thinks_sensitive"],
            }
            for field in facts["fields"]
        },
        "definition": definition,
    }
    if image_pins:
        manifest['image_pins'] = image_pins
    if args.draft:
        manifest['risk_notes'] = [note for note in manifest['risk_notes'] if not note.startswith('Proven on Windows')]
        manifest['risk_notes'].append('Not qualified on the Local Store managed engine. This withheld proposal is not installable from the catalog.')
    # What Runtipi copies into the data folder travels with the definition,
    # verbatim, so the review covers it. template_facts already refused to
    # continue quietly if any of it was binary.
    if facts.get("binary_seeds"):
        raise SystemExit(
            f"{args.app} ships binary seed files {facts['binary_seeds']}, which a manifest cannot carry"
        )
    seeds = []
    for relative in facts.get("seed_files", []):
        seed = definition_path.parent / relative
        seeds.append({"path": relative, "content": seed.read_text(encoding="utf-8")})
    if seeds:
        manifest["seeds"] = seeds
    if candidate["source"] == "runtipi" and config_path.is_file():
        manifest["config"] = {
            "path": str(Path(provenance["path"]).parent / "config.json").replace("\\", "/"),
            "content": config_path.read_text(encoding="utf-8"),
        }

    # The proof travels with the manifest that names it.
    evidence = ROOT / "docs" / "evidence" / f"{args.app}-qualification.json"
    if result is not None:
        evidence.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")

    # A recent rebuild is the fact a promotion turns on most often, and eight
    # apps were once approved in a batch without anybody looking at it — three
    # of them ran images last rebuilt in 2021, 2022 and 2023. Say it loudly.
    oldest = min(image["last_updated"] for image in manifest["requirements"]["images"])
    age_days = (datetime.date.today() - datetime.date.fromisoformat(oldest)).days
    stale = age_days > 365

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    outstanding = json.dumps(manifest).count(REVIEW)
    print(f"wrote {target.relative_to(ROOT)}" + (f" and {evidence.relative_to(ROOT)}" if result else '; no lifecycle proof or catalog approval claimed'))
    print(f"{outstanding} thing(s) marked {REVIEW.strip()} still need a person")
    if stale:
        print(
            f"  WARNING: the oldest image was last rebuilt {oldest} — {age_days // 365} year(s) "
            "ago. An old image is not a fault by itself, but it is the fact a promotion decision "
            "turns on most often. Withhold unless there is a reason not to."
        )
    else:
        print(f"  images last rebuilt {oldest}, which is current enough to consider offering")
    latest = upstream_release(manifest["source_url"])
    if latest:
        tag, published = latest
        pinned = [image["image"].rsplit(":", 1)[-1] for image in manifest["requirements"]["images"]]
        bare = tag.lstrip("vV")
        same = any(pin.lstrip("vV").split("-")[0] == bare for pin in pinned)
        print(
            f"  upstream's latest release is {tag}, published {published}: "
            + ("the definition pins it" if same else f"the definition pins {', '.join(pinned)}")
        )
    print("then: record a lifecycle proof, set verified_at, and decide promotion")


if __name__ == "__main__":
    main()
