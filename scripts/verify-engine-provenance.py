"""Independently verify retained engine artifacts without Docker, WSL or extraction.

Requires GnuPG gpg/gpgv, python-lz4 and zstandard; optional fetch flags capture
public signed Docker metadata and the pinned official Ubuntu OCI graph.
Writes public audit evidence, never approves a
distributable payload or infers a supported Windows host from a tar checksum.
"""
import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import io
import json
import lzma
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import urllib.error
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
REVIEW = ROOT / ".cache/engine-independent-review"
UBUNTU_KEYS = {"F6ECB3762474EDA9D21B7022871920D1991BC93C", "790BC7277767219C42C86F933B4FE6ACC0B21F32"}
DOCKER_KEY = "9DC858229FC7DD38854AE2D88D81803C0EBFCD88"
DEFAULT_BUILD = ROOT / ".cache/engine/build-644d5c2b5ccf44abad695ac79b06d8d2"
REFERENCE = ROOT / "docs/evidence/engine-session-payload-development-2026-10-01.json"
MAX_INDEX = 256 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fields(data):
    """Deb822 paragraphs; duplicate fields and invalid continuation refuse."""
    result, previous = {}, None
    for line in data.splitlines():
        if line[:1] in {" ", "\t"}:
            if previous is None:
                raise ValueError("orphan metadata continuation")
            result[previous] += "\n" + line[1:]
        else:
            key, separator, value = line.partition(":")
            if not separator or not re.fullmatch(r"[A-Za-z0-9-]+", key) or key in result:
                raise ValueError("invalid or duplicate metadata field")
            previous = key
            result[key] = value.strip()
    return result


def paragraphs(data):
    paragraph = []
    for line in io.TextIOWrapper(io.BytesIO(data), encoding="utf-8"):
        if line.strip():
            paragraph.append(line.rstrip("\n"))
        elif paragraph:
            yield fields("\n".join(paragraph))
            paragraph = []
    if paragraph:
        yield fields("\n".join(paragraph))


def safe_name(name):
    path = PurePosixPath(name)
    # Tar paths are POSIX strings and are never extracted. A literal backslash
    # is valid in systemd's escaped unit filenames; it is not a Windows path.
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("unsafe tar member")
    return str(path)


def members(path, bound):
    result = {}
    with tarfile.open(path, "r:") as archive:
        total = 0
        for member in archive:
            name = safe_name(member.name)
            if not member.isfile():
                if member.isdir() or member.issym() or member.islnk():
                    continue
                raise ValueError("unexpected tar member type")
            if name in result or member.size > bound or len(result) >= 1000:
                raise ValueError("duplicate or oversized tar member")
            total += member.size
            if total > 1024 * 1024 * 1024:
                raise ValueError("tar metadata exceeds bound")
            result[name] = archive.extractfile(member).read(member.size + 1)
    return result


def root_metadata(path):
    selected, notices, links = {}, {}, {}
    wanted = {"var/lib/dpkg/status", "usr/share/local-store/packages.tsv",
              "usr/share/keyrings/ubuntu-archive-keyring.gpg"}
    with tarfile.open(path, "r:") as archive:
        for member in archive:
            name = safe_name(member.name)
            if name in wanted or name.startswith("usr/share/common-licenses/") or (name.startswith("usr/share/doc/") and name.endswith("/copyright")):
                if member.isfile():
                    if member.size > 1024 * 1024:
                        raise ValueError("rootfs metadata exceeds bound")
                    data = archive.extractfile(member).read(member.size + 1)
                    if name in wanted:
                        selected[name] = data
                    else:
                        notices[name] = data
                elif member.issym() or member.islnk():
                    links[name] = member.linkname
            elif name.startswith("usr/share/doc/") and member.issym():
                links[name] = member.linkname
    if selected.keys() != wanted:
        raise ValueError("required rootfs metadata is missing")
    return selected, notices, links


def executable(name):
    located = shutil.which(name)
    if located:
        return Path(located)
    candidate = Path("C:/Program Files/Git/usr/bin") / (name + ".exe")
    if candidate.is_file():
        return candidate
    raise ValueError("installed GnuPG verifier is required")


def gpg_path(path, executable_path):
    absolute = path.resolve().as_posix()
    if "/Git/usr/bin/" in executable_path.as_posix() and re.match(r"^[A-Za-z]:/", absolute):
        return "/" + absolute[0].lower() + absolute[2:]
    return absolute


def verify_signature(data, keyring, stem, expected):
    gpgv = executable("gpgv")
    source, output = REVIEW / (stem + ".InRelease"), REVIEW / (stem + ".Release")
    source.write_bytes(data)
    output.unlink(missing_ok=True)
    command = [str(gpgv), "--homedir", gpg_path(REVIEW, gpgv), "--status-fd", "1",
               "--keyring", gpg_path(keyring, gpgv), "--output", gpg_path(output, gpgv), gpg_path(source, gpgv)]
    result = subprocess.run(command, capture_output=True, text=True, timeout=30)
    signatures = [line.split() for line in result.stdout.splitlines() if line.startswith("[GNUPG:] VALIDSIG ")]
    if result.returncode or len(signatures) != 1 or signatures[0][-1] not in expected:
        raise ValueError("signature failed or used an unreviewed trust anchor: " + stem)
    release = fields(output.read_text(encoding="utf-8"))
    hashes = {}
    for line in release.get("SHA256", "").splitlines():
        if not line.strip():
            continue
        digest, length, name = line.split()
        if not re.fullmatch(r"[0-9a-f]{64}", digest) or name in hashes:
            raise ValueError("invalid signed metadata hash")
        hashes[name] = (digest, int(length))
    if not hashes:
        raise ValueError("signed SHA256 table absent")
    return release, hashes, {"signer": signatures[0][2], "primary_key": signatures[0][-1], "inrelease_sha256": sha(data), "date": release.get("Date"), "suite": release.get("Suite")}


def lz4_decode(data, expected_length):
    # Reuse the maintained decoder; never trust its declared content length.
    sys.path.insert(0, str(REVIEW / "vendor"))
    import lz4.frame
    decoder = lz4.frame.LZ4FrameDecompressor()
    decoded = decoder.decompress(data, max_length=expected_length + 1)
    if len(decoded) != expected_length or not decoder.eof or decoder.unused_data:
        raise ValueError("compressed index is truncated, trailing or oversized")
    return decoded


def signed_packages(indexes, keyring, wanted):
    releases, observations, signed = {}, [], []
    for name, data in indexes.items():
        if name.endswith("_InRelease"):
            release, hashes, observed = verify_signature(data, keyring, sha(name.encode())[:16], UBUNTU_KEYS)
            releases[name.removesuffix("_InRelease")] = hashes
            observed["file"] = name
            signed.append(observed)
    records = {}
    for name, data in indexes.items():
        if not name.endswith("_Packages.lz4"):
            continue
        match = re.fullmatch(r"(.+_dists_[^_]+)_(main|restricted|universe|multiverse)_binary-amd64_Packages\.lz4", name)
        if not match or match[1] not in releases:
            raise ValueError("package index has no verified release")
        relative = match[2] + "/binary-amd64/Packages"
        digest, length = releases[match[1]][relative]
        if not 0 <= length <= MAX_INDEX:
            raise ValueError("signed index length exceeds bound")
        decoded = lz4_decode(data, length)
        if sha(decoded) != digest:
            raise ValueError("package index disagrees with signed Release")
        observations.append({"file": name, "signed_path": relative, "sha256": digest, "size_bytes": length})
        for record in paragraphs(decoded):
            if record.get("Package") in wanted:
                key = (record["Package"], record["Version"], record["Architecture"])
                records.setdefault(key, []).append(record)
    return records, signed, observations


def deb_control(data):
    if not data.startswith(b"!<arch>\n"):
        raise ValueError("invalid Debian ar header")
    offset, control = 8, None
    while offset < len(data):
        header = data[offset:offset + 60]
        if len(header) != 60 or header[58:] != b"`\n":
            raise ValueError("invalid Debian ar member")
        name, length = header[:16].decode().strip().removesuffix("/"), int(header[48:58])
        body = data[offset + 60:offset + 60 + length]
        if len(body) != length:
            raise ValueError("truncated Debian ar member")
        if name.startswith("control.tar"):
            if control is not None or length > 2 * 1024 * 1024:
                raise ValueError("duplicate or oversized Debian control")
            if name.endswith(".zst"):
                import zstandard
                with zstandard.ZstdDecompressor().stream_reader(io.BytesIO(body)) as reader:
                    body = reader.read(2 * 1024 * 1024 + 1)
            elif name.endswith(".gz"):
                with gzip.GzipFile(fileobj=io.BytesIO(body)) as reader:
                    body = reader.read(2 * 1024 * 1024 + 1)
            elif name.endswith(".xz"):
                with lzma.LZMAFile(io.BytesIO(body)) as reader:
                    body = reader.read(2 * 1024 * 1024 + 1)
            elif name != "control.tar":
                raise ValueError("unsupported Debian control compression")
            if len(body) > 2 * 1024 * 1024:
                raise ValueError("oversized decompressed Debian control")
            with tarfile.open(fileobj=io.BytesIO(body), mode="r:") as archive:
                candidate = next(m for m in archive if safe_name(m.name) == "control" and m.isfile())
                if candidate.size > 128 * 1024:
                    raise ValueError("oversized Debian control fields")
                control = fields(archive.extractfile(candidate).read().decode())
        offset += 60 + length + length % 2
    if control is None or offset != len(data):
        raise ValueError("missing Debian control")
    return control


def fetch(url, target, maximum):
    if target.is_file():
        if target.stat().st_size > maximum:
            raise ValueError("cached public metadata exceeds bound")
        return target.read_bytes()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), urllib.request.HTTPRedirectHandler())
    with opener.open(url, timeout=30) as response:
        if response.url != url:
            raise ValueError("unexpected public metadata redirect")
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("public metadata exceeds bound")
    target.write_bytes(data)
    return data


def docker_records(fetch_allowed):
    names = {"docker.asc": ("https://download.docker.com/linux/ubuntu/gpg", 128 * 1024),
             "docker.InRelease": ("https://download.docker.com/linux/ubuntu/dists/noble/InRelease", 1024 * 1024),
             "docker.Packages.gz": ("https://download.docker.com/linux/ubuntu/dists/noble/stable/binary-amd64/Packages.gz", 16 * 1024 * 1024)}
    for name, (url, bound) in names.items():
        if fetch_allowed:
            fetch(url, REVIEW / name, bound)
        elif not (REVIEW / name).is_file():
            return {}, {"verified": False, "reason": "signed Docker metadata has not been captured"}
        elif (REVIEW / name).stat().st_size > bound:
            raise ValueError("cached Docker metadata exceeds bound")
    gpg = executable("gpg")
    keyring = REVIEW / "docker-keyring.gpg"
    keyring.unlink(missing_ok=True)
    subprocess.run([str(gpg), "--batch", "--homedir", gpg_path(REVIEW, gpg), "--output", gpg_path(keyring, gpg), "--dearmor", gpg_path(REVIEW / "docker.asc", gpg)], check=True, capture_output=True, timeout=30)
    release, hashes, observed = verify_signature((REVIEW / "docker.InRelease").read_bytes(), keyring, "docker-verified", {DOCKER_KEY})
    compressed = (REVIEW / "docker.Packages.gz").read_bytes()
    expected = hashes["stable/binary-amd64/Packages.gz"]
    if (sha(compressed), len(compressed)) != expected:
        raise ValueError("Docker Packages.gz disagrees with signed index")
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
        data = stream.read(MAX_INDEX + 1)
    if len(data) > MAX_INDEX:
        raise ValueError("Docker index exceeds bound")
    if "stable/binary-amd64/Packages" in hashes and (sha(data), len(data)) != hashes["stable/binary-amd64/Packages"]:
        raise ValueError("Docker uncompressed index disagrees with signed index")
    result = {(p["Package"], p["Version"], p["Architecture"]): p for p in paragraphs(data)}
    observed.update({"verified": True, "packages_gz_sha256": sha(compressed), "capture_scope": "current official signed index; not a build-time signature receipt"})
    return result, observed


def source_identity(record):
    source = record.get("Source", record["Package"])
    match = re.fullmatch(r"([a-z0-9+.-]+)(?: \(([^\s()]+)\))?", source)
    if not match:
        raise ValueError("invalid source package identity")
    return match[1], match[2] or record["Version"]


def base_inventory(lock, fetch_allowed):
    """Verify the pinned official OCI manifest/blob graph without an engine."""
    prefix = "https://registry-1.docker.io/v2/library/ubuntu/"
    digest = lock["base_image"].split("@")[1]
    manifest_path = REVIEW / "ubuntu-base-manifest.json"
    token = None
    if fetch_allowed:
        data = fetch("https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/ubuntu:pull", REVIEW / "public-ubuntu-read-token.json", 64 * 1024)
        token = json.loads(data)["token"]
        # The anonymous repository token is disposable and is not audit data.
        (REVIEW / "public-ubuntu-read-token.json").unlink()

    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, req, fp, code, msg, headers, newurl):
            return None

    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def read_blob(url, path, expected, bound, manifest=False):
        if path.is_file():
            if path.stat().st_size > bound:
                raise ValueError("cached OCI object exceeds declared bound")
            result = path.read_bytes()
        elif not fetch_allowed:
            return None
        else:
            headers = {"authorization": "Bearer " + token}
            if manifest:
                headers["accept"] = "application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json"
            try:
                response = opener.open(urllib.request.Request(url, headers=headers), timeout=30)
            except urllib.error.HTTPError as error:
                target = error.headers.get("Location", "")
                host = urlsplit(target).hostname or ""
                if error.code not in {302, 307} or urlsplit(target).scheme != "https" or not (host in {"production.cloudflare.docker.com", "production.cloudfront.docker.com"} or host.endswith(".r2.cloudflarestorage.com")):
                    raise ValueError("official base metadata/blob request failed") from None
                # No authorization is forwarded to a CDN origin.
                response = opener.open(target, timeout=30)
            with response:
                result = response.read(bound + 1)
            if len(result) > bound:
                raise ValueError("official OCI object exceeds declared bound")
            path.write_bytes(result)
        if "sha256:" + sha(result) != expected:
            raise ValueError("official OCI object disagrees with pinned digest")
        return result

    raw = read_blob(prefix + "manifests/" + digest, manifest_path, digest, 1024 * 1024, True)
    if raw is None:
        return {}, {"verified": False, "reason": "pinned official base manifest/layers not captured"}
    manifest = json.loads(raw)
    config = manifest.get("config", {})
    config_digest = config.get("digest", "")
    if not re.fullmatch(r"sha256:[0-9a-f]{64}", config_digest):
        raise ValueError("pinned base must be an exact image manifest")
    config_data = read_blob(prefix + "blobs/" + config_digest, REVIEW / "ubuntu-base-config.json", config_digest, 1024 * 1024)
    if config_data is None:
        return {}, {"verified": False, "reason": "pinned base config not captured"}
    configuration = json.loads(config_data)
    if configuration.get("os") != "linux" or configuration.get("architecture") != "amd64":
        raise ValueError("official base platform mismatch")
    status = None
    layers = manifest.get("layers", [])
    if not 1 <= len(layers) <= 8:
        raise ValueError("unexpected Ubuntu base layer count")
    for layer in layers:
        layer_digest, size = layer.get("digest", ""), layer.get("size", 0)
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", layer_digest) or not 0 < size <= 250 * 1024 * 1024:
            raise ValueError("invalid Ubuntu base layer identity")
        blob = read_blob(prefix + "blobs/" + layer_digest, REVIEW / ("ubuntu-base-" + layer_digest[7:] + ".blob"), layer_digest, size)
        if blob is None:
            return {}, {"verified": False, "reason": "pinned base layer not captured"}
        if len(blob) != size:
            raise ValueError("Ubuntu base layer size mismatch")
        with tarfile.open(fileobj=io.BytesIO(blob), mode="r:gz") as archive:
            for member in archive:
                if safe_name(member.name) == "var/lib/dpkg/status" and member.isfile():
                    if member.size > 1024 * 1024:
                        raise ValueError("base dpkg metadata exceeds bound")
                    status = archive.extractfile(member).read(member.size + 1)
    if status is None:
        raise ValueError("official base package inventory is absent")
    inventory = {p["Package"]: p for p in paragraphs(status) if p.get("Status") == "install ok installed"}
    return inventory, {"verified": True, "manifest_sha256": sha(raw), "config_sha256": sha(config_data), "layer_digests": [layer["digest"] for layer in layers], "created": configuration.get("created"), "package_count": len(inventory), "trust_scope": "official library/ubuntu HTTPS origin and pinned OCI digest graph; no OCI signature/attestation claim"}


def resolve_notice(path, notices, links):
    """Resolve captured POSIX doc links in memory; never touch the host tree."""
    seen = set()
    for _ in range(8):
        if path in notices:
            return path
        if path in seen:
            return None
        seen.add(path)
        parts = PurePosixPath(path).parts
        candidate = next(("/".join(parts[:i]) for i in range(len(parts), 0, -1) if "/".join(parts[:i]) in links), None)
        if candidate is None:
            return None
        target = links[candidate]
        joined = (PurePosixPath(candidate).parent / target).parts if not target.startswith("/") else PurePosixPath(target[1:]).parts
        normalized = []
        for part in joined:
            if part == "..":
                if not normalized:
                    return None
                normalized.pop()
            elif part != ".":
                normalized.append(part)
        path = "/".join(normalized + list(parts[len(PurePosixPath(candidate).parts):]))
        if not path.startswith(("usr/share/doc/", "usr/share/common-licenses/")):
            return None
    return None


def review(build, fetch_allowed, fetch_base=False):
    build = build.resolve()
    if not build.is_relative_to((ROOT / ".cache/engine").resolve()):
        raise ValueError("retained build must be inside the engine cache")
    REVIEW.mkdir(parents=True, exist_ok=True)
    evidence = json.loads((build / "evidence.json").read_text())
    reference = json.loads(REFERENCE.read_text())
    provenance = json.loads((build / "ubuntu-provenance.json").read_text())
    lock = json.loads((ROOT / "engine/components.lock.json").read_text())
    if (evidence["rootfs_sha256"], evidence["rootfs_bytes"]) != (reference["rootfs_sha256"], reference["rootfs_bytes"]):
        raise ValueError("cache artifact differs from the selected committed payload receipt")
    for name, expected in reference["build_inputs_sha256"].items():
        if sha((ROOT / "engine" / name).read_bytes().replace(b"\r\n", b"\n")) != expected:
            raise ValueError("current build inputs differ from the selected payload receipt")
    if file_sha(build / "rootfs.tar") != evidence["rootfs_sha256"] or (build / "rootfs.tar").stat().st_size != evidence["rootfs_bytes"]:
        raise ValueError("rootfs disagrees with retained receipt")
    for name, expected in [("ubuntu-indexes.tar", provenance["indexes_tar_sha256"]), ("ubuntu-archives.tar", provenance["archives_tar_sha256"]), ("notices.tar", evidence["notices_sha256"])]:
        if file_sha(build / name) != expected:
            raise ValueError("retained archive disagrees with manifest")
    indexes, archives = members(build / "ubuntu-indexes.tar", 128 * 1024 * 1024), members(build / "ubuntu-archives.tar", 250 * 1024 * 1024)
    archives = {n: d for n, d in archives.items() if n.endswith(".deb")}
    for actual, expected in [(indexes, provenance["indexes"]), (archives, provenance["archives"])]:
        if actual.keys() != expected.keys() or any(sha(data) != expected[name]["sha256"] or len(data) != expected[name]["size_bytes"] for name, data in actual.items()):
            raise ValueError("retained member hashes or inventory disagree")
    metadata, notices, links = root_metadata(build / "rootfs.tar")
    inventory = metadata["usr/share/local-store/packages.tsv"]
    locked = (ROOT / "engine/packages.lock.tsv").read_bytes().replace(b"\r\n", b"\n")
    if inventory != locked or inventory != (build / "packages.tsv").read_bytes():
        raise ValueError("rootfs installed inventory disagrees with current lock")
    installed = {p["Package"]: p for p in paragraphs(metadata["var/lib/dpkg/status"]) if p.get("Status") == "install ok installed"}
    rows = [tuple(line.split("\t")) for line in inventory.decode().splitlines()]
    if len(installed) != len(rows) or any((installed[name]["Version"], installed[name]["Architecture"]) != (version, architecture) for name, version, architecture in rows):
        raise ValueError("independent dpkg status inventory disagrees")
    keyring = REVIEW / "ubuntu-keyring.gpg"
    keyring.write_bytes(metadata["usr/share/keyrings/ubuntu-archive-keyring.gpg"])
    records, signed, observations = signed_packages(indexes, keyring, set(installed))
    archive_records, verified_archives = {}, []
    for name, data in archives.items():
        control = deb_control(data)
        identity = (control["Package"], control["Version"], control["Architecture"])
        candidates = [p for p in records.get(identity, []) if p.get("SHA256") == sha(data) and p.get("Size") == str(len(data))]
        if not candidates:
            raise ValueError("Ubuntu archive has no exact signed index match: " + name)
        source, source_version = source_identity(candidates[0])
        archive_records[identity] = candidates[0]
        verified_archives.append({"file": name, "package": identity[0], "version": identity[1], "architecture": identity[2], "sha256": sha(data), "source": source, "source_version": source_version, "installed": identity[0] in installed and installed[identity[0]]["Version"] == identity[1]})
    docker, docker_observed = docker_records(fetch_allowed)
    components = []
    for package in lock["packages"]:
        path = build / "context/packages" / (package["name"] + ".deb")
        if not path.is_file() or file_sha(path) != package["sha256"] or path.stat().st_size != package["size_bytes"]:
            raise ValueError("Docker package archive disagrees with lock")
        record = docker.get((package["name"], package["version"], package["architecture"]))
        if docker_observed.get("verified") and (not record or record.get("SHA256") != package["sha256"] or record.get("Size") != str(package["size_bytes"])):
            raise ValueError("Docker package has no exact signed index match")
        components.append({"package": package["name"], "sha256": package["sha256"], "signed_index_verified": bool(record)})
    inherited, coverage = [], []
    base, base_observed = base_inventory(lock, fetch_base)
    docker_names = {p["name"] for p in lock["packages"]}
    for name, version, architecture in rows:
        origin = "docker-component" if name in docker_names else "verified-retained-ubuntu-archive" if (name, version, architecture) in archive_records else "inherited-base-package"
        if origin == "inherited-base-package":
            inherited.append(name)
            if base_observed.get("verified") and (name not in base or (base[name]["Version"], base[name]["Architecture"]) != (version, architecture)):
                raise ValueError("inherited package disagrees with verified official base")
        source, source_version = source_identity(installed[name])
        notice_path = "usr/share/doc/" + name + "/copyright"
        resolved_notice = resolve_notice(notice_path, notices, links)
        license_hints = sorted(set(re.findall(r"(?im)^License:\s*(.+)$", notices[resolved_notice].decode("utf-8", errors="replace")))) if resolved_notice else []
        coverage.append({"package": name, "version": version, "architecture": architecture, "origin": origin, "source": source, "source_version": source_version, "copyright_present": resolved_notice is not None, "copyright_file": resolved_notice, "copyright_sha256": sha(notices[resolved_notice]) if resolved_notice else None, "license_hints": license_hints, "signed_current_or_snapshot_binary_metadata": (name, version, architecture) in records})
    return {"schema_version": 1, "recorded_at": datetime.now(timezone.utc).isoformat(), "status": "independent_provenance_review_not_distribution_approval", "rootfs_sha256": evidence["rootfs_sha256"], "rootfs_bytes": evidence["rootfs_bytes"], "package_count": len(rows), "installed_inventory_verified": True, "ubuntu_archive_count": len(verified_archives), "ubuntu_index_count": len(indexes), "ubuntu_signed_releases": signed, "ubuntu_verified_package_indexes": observations, "ubuntu_verified_archives": verified_archives, "docker_signed_index": docker_observed, "docker_components": components, "ubuntu_base": base_observed, "inherited_base_packages": inherited, "package_source_notice_inventory": coverage, "rootfs_common_license_files": sorted(n for n in notices if n.startswith("usr/share/common-licenses/")), "notice_archive_sha256": evidence["notices_sha256"], "source_archives_bundled": False, "distribution_approved": False, "supported_windows_host_verified": False, "review_script_sha256": file_sha(Path(__file__)), "limitations": ["Inherited Ubuntu base-image packages are inventoried separately; retained added-package archives do not prove their binary bytes. OCI provenance uses its pinned content graph, not an upstream signature claim.", "Source identity and retained notices are an obligation inventory, not corresponding-source fulfillment or legal release approval.", "No engine boot, fresh Windows acceptance, supported-host floor or signed delivery is inferred."]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--build-dir", type=Path, default=DEFAULT_BUILD)
    parser.add_argument("--fetch-docker", action="store_true")
    parser.add_argument("--fetch-base", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/evidence/engine-independent-provenance-2026-10-02.json")
    args = parser.parse_args()
    result = review(args.build_dir, args.fetch_docker, args.fetch_base)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({key: result[key] for key in ["status", "package_count", "ubuntu_archive_count", "ubuntu_index_count", "distribution_approved"]}))
