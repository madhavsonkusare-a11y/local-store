"""Validate engine pins, or build/export an isolated development WSL rootfs.

No WSL registration, privileged container, host socket mount or daemon change.
Outputs stay in .cache/engine; this is not a signed release artifact.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from pathlib import PurePosixPath
import re
import subprocess
import tarfile
import urllib.request
from urllib.parse import urlsplit
import uuid

ROOT = Path(__file__).resolve().parents[1]
ENGINE = ROOT / "engine"
EXPECTED = {"docker-ce", "docker-ce-cli", "containerd.io",
            "docker-compose-plugin", "docker-buildx-plugin"}
PACKAGE_LOCK = ENGINE / "packages.lock.tsv"
MANAGED_ENGINE = False


def docker_command(*args):
    if not MANAGED_ENGINE:
        return ["docker", *args]
    # Explicit development build mode. Never changes Docker's global context.
    def project(value):
        value = str(value)
        if re.match(r"^[A-Za-z]:[\\/]", value):
            return "/mnt/" + value[0].lower() + "/" + value[3:].replace("\\", "/")
        return value
    return ["wsl.exe", "--distribution", "local-store-engine-v1", "--user", "root",
            "--exec", "/usr/bin/docker", "--host", "unix:///var/run/docker.sock",
            *(project(arg) for arg in args)]


def verify_managed_builder():
    state = ROOT / ".cache/engine/real-wsl-proof/state"
    journal_bytes = (state / "bootstrap.json").read_bytes()
    if len(journal_bytes) > 16 * 1024:
        raise ValueError("Managed-engine journal exceeds its bound")
    journal = json.loads(journal_bytes)
    token = (state / "ownership-token").read_bytes()
    if (journal.get("schema_version") != 1 or journal.get("state") != "verified"
            or journal.get("distro") != "local-store-engine-v1"
            or token != journal.get("ownership_token", "").encode()
            or not 32 <= len(token) <= 128
            or Path(journal.get("install_dir", "")).resolve() != (ROOT / ".cache/engine/real-wsl-proof/data").resolve()):
        raise ValueError("Development engine ownership could not be verified")
    path = str((state / "ownership-token").resolve()).replace("\\", "/")
    if not re.match(r"^[A-Za-z]:/", path):
        raise ValueError("The managed builder requires Windows")
    source = "/mnt/" + path[0].lower() + "/" + path[3:]
    subprocess.run(["wsl.exe", "--distribution", "local-store-engine-v1", "--user", "root",
                    "--exec", "/usr/bin/cmp", "--silent", source,
                    "/usr/share/local-store/ownership-token"], check=True, timeout=60,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def validate(lock):
    if lock.get("schema_version") != 1 or lock.get("platform") != "linux/amd64":
        raise ValueError("Unsupported engine lock schema/platform")
    if not re.fullmatch(r"ubuntu:24\.04@sha256:[0-9a-f]{64}", lock["base_image"]):
        raise ValueError("Base image must be an exact Ubuntu 24.04 digest")
    snapshot = lock.get("ubuntu_snapshot", "")
    if not re.fullmatch(r"20[0-9]{6}T[0-9]{6}Z", snapshot):
        raise ValueError("Ubuntu archive snapshot must be an exact UTC timestamp")
    try:
        parsed_snapshot = datetime.strptime(snapshot, "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc)
    except ValueError as error:
        raise ValueError("Ubuntu archive snapshot is not a real UTC timestamp") from error
    if parsed_snapshot >= datetime.now(timezone.utc):
        raise ValueError("Ubuntu archive snapshot cannot be in the future")
    packages = lock["packages"]
    if len(packages) != len(EXPECTED) or {p["name"] for p in packages} != EXPECTED:
        raise ValueError("Missing or duplicate engine packages")
    for p in packages:
        url = urlsplit(p["url"])
        if (url.scheme != "https" or url.netloc != "download.docker.com"
                or not url.path.startswith("/linux/ubuntu/dists/noble/pool/stable/amd64/")
                or not url.path.endswith(".deb") or url.query or url.fragment
                or "/../" in url.path):
            raise ValueError("Unexpected engine package origin")
        if (p["architecture"] != "amd64" or not p["version"]
                or not re.fullmatch(r"[0-9a-f]{64}", p["sha256"])
                or not 0 < p["size_bytes"] <= 250_000_000):
            raise ValueError("Invalid engine package pin")
    inventory = parse_inventory(PACKAGE_LOCK.read_bytes())
    if len(inventory) < 100:
        raise ValueError("Engine package inventory is unexpectedly small")
    for package in packages:
        if inventory.get(package["name"]) != (package["version"], package["architecture"]):
            raise ValueError("Engine package inventory disagrees with component pins")


def parse_inventory(data):
    if not data or b"\r" in data or not data.endswith(b"\n") or len(data) > 128 * 1024:
        raise ValueError("Invalid engine package inventory encoding")
    inventory = {}
    previous = ""
    for raw in data.decode("utf-8").splitlines():
        parts = raw.split("\t")
        if len(parts) != 3:
            raise ValueError("Invalid engine package inventory row")
        name, version, architecture = parts
        if (not re.fullmatch(r"[a-z0-9][a-z0-9+.-]*", name)
                or name <= previous or not version or any(c.isspace() for c in version)
                or architecture not in {"all", "amd64"}):
            raise ValueError("Invalid or unordered engine package inventory")
        inventory[name] = (version, architecture)
        previous = name
    return inventory


def fetch_package(package, target):
    def valid():
        return (target.is_file() and target.stat().st_size == package["size_bytes"]
                and digest(target) == package["sha256"])
    if valid():
        return
    partial = target.with_suffix(".partial")
    try:
        with urllib.request.urlopen(package["url"], timeout=60) as response:
            final_url = urlsplit(response.url)
            if final_url.scheme != "https" or final_url.netloc != "download.docker.com":
                raise ValueError("Unexpected package redirect")
            with partial.open("wb") as out:
                received = 0
                while chunk := response.read(1024 * 1024):
                    received += len(chunk)
                    if received > package["size_bytes"]:
                        raise ValueError("Package exceeds locked size")
                    out.write(chunk)
        if partial.stat().st_size != package["size_bytes"] or digest(partial) != package["sha256"]:
            raise ValueError("Package does not match locked size/SHA-256")
        partial.replace(target)
    finally:
        partial.unlink(missing_ok=True)


def docker(*args, timeout=120):
    return subprocess.check_output(docker_command(*args), text=True, encoding="utf-8", timeout=timeout).strip()


def inventory_drift(locked, observed):
    """Return package-level changes without treating a diagnosis as a release build."""
    expected = parse_inventory(locked)
    actual = parse_inventory(observed)
    return [
        {"name": name, "locked": expected.get(name), "observed": actual.get(name)}
        for name in sorted(expected.keys() | actual.keys())
        if expected.get(name) != actual.get(name)
    ]


def manifest_tar(path, *, suffix=None):
    """Hash regular Docker-copied files without extracting untrusted tar paths."""
    files = {}
    total = 0
    with tarfile.open(path, "r:") as archive:
        for entry in archive:
            raw = entry.name
            name = raw.removeprefix("./")
            parts = PurePosixPath(name).parts
            if (not name or name.startswith("/") or "\\" in name
                    or any(part in {"", ".."} for part in parts)):
                raise ValueError("Unsafe apt provenance tar path")
            if entry.isdir():
                continue
            if not entry.isfile() or entry.size < 0 or entry.size > 250_000_000:
                raise ValueError("Unexpected apt provenance tar entry")
            if suffix and not name.endswith(suffix):
                continue
            if name in files or len(files) >= 1000:
                raise ValueError("Duplicate or excessive apt provenance files")
            total += entry.size
            if total > 1_000_000_000:
                raise ValueError("Apt provenance exceeds size limit")
            stream = archive.extractfile(entry)
            if stream is None:
                raise ValueError("Missing apt provenance file")
            with stream:
                sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
            files[name] = {"size_bytes": entry.size, "sha256": sha256}
    return dict(sorted(files.items()))


def capture_apt_provenance(image, output, lock):
    """Save APT-verified indexes and downloaded Ubuntu archives for review."""
    container = docker("create", "--network", "none", "--label",
                       "io.local-store.purpose=engine-apt-provenance", image)
    try:
        for source, filename in (("/var/cache/apt/archives", "ubuntu-archives.tar"),
                                 ("/var/lib/apt/lists", "ubuntu-indexes.tar")):
            with (output / filename).open("wb") as destination:
                subprocess.run(docker_command("cp", container + ":" + source, "-"),
                               stdout=destination, check=True, timeout=120)
    finally:
        docker("rm", container)
    archives = manifest_tar(output / "ubuntu-archives.tar", suffix=".deb")
    indexes = manifest_tar(output / "ubuntu-indexes.tar")
    if len(archives) < 20:
        raise ValueError("Too few cached Ubuntu package archives")
    snapshot = lock["ubuntu_snapshot"]
    for suite in ("noble", "noble-updates", "noble-security"):
        needle = snapshot + "_dists_" + suite + "_InRelease"
        if not any(name.endswith(needle) for name in indexes):
            raise ValueError("Missing signed Ubuntu snapshot index: " + suite)
    manifest = {"schema_version": 1, "status": "development_capture_only",
                "ubuntu_snapshot": snapshot, "image_id": image,
                "archives_tar_sha256": digest(output / "ubuntu-archives.tar"),
                "indexes_tar_sha256": digest(output / "ubuntu-indexes.tar"),
                "archives": archives, "indexes": indexes,
                "limitations": [
                    "APT verified signed Ubuntu indexes during the build; this manifest records their bytes but does not independently re-verify signatures.",
                    "Archive bytes are observed and hashed, not yet matched to a reviewed transitive-package lock.",
                ]}
    (output / "ubuntu-provenance.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def build_stage(lock, context, image_file, stage):
    subprocess.run(docker_command("build", "--platform", lock["platform"],
                    "--target", stage,
                    "--build-arg", "BASE_IMAGE=" + lock["base_image"],
                    "--build-arg", "UBUNTU_SNAPSHOT=" + lock["ubuntu_snapshot"],
                    "--build-arg", "UBUNTU_CA_CERTIFICATES_VERSION="
                    + parse_inventory(PACKAGE_LOCK.read_bytes())["ca-certificates"][0],
                    "--build-arg", "UBUNTU_OPENSSL_VERSION="
                    + parse_inventory(PACKAGE_LOCK.read_bytes())["openssl"][0],
                    "--iidfile", str(image_file), str(context)), check=True, timeout=1200)
    image = image_file.read_text().strip()
    details = json.loads(docker("image", "inspect", image))[0]
    if details["Os"] + "/" + details["Architecture"] != lock["platform"]:
        raise ValueError("Built platform differs from lock")
    return image


def build(lock, inspect_inventory=False):
    # Unique output/name: concurrent runs never share a partial artifact/container.
    output = ROOT / ".cache" / "engine" / ("build-" + uuid.uuid4().hex)
    context = output / "context"
    packages = context / "packages"
    packages.mkdir(parents=True)
    for p in lock["packages"]:
        print("Checking package:", p["name"], p["version"], flush=True)
        fetch_package(p, packages / (p["name"] + ".deb"))
    for name in ("Dockerfile", "wsl.conf", "daemon.json", "components.lock.json",
                 "packages.lock.tsv"):
        # Windows Git checkouts may use CRLF; Dockerfile continuation/config
        # bytes must not depend on the builder's host line-ending preference.
        (context / name).write_bytes((ENGINE / name).read_bytes().replace(b"\r\n", b"\n"))
    package_image = build_stage(lock, context, output / "package-image-id", "package-install")
    package_container = docker("create", "--network", "none", "--label",
                               "io.local-store.purpose=engine-package-inspection", package_image)
    try:
        docker("cp", package_container + ":/usr/share/local-store/packages.tsv",
               str(output / "packages.tsv"))
    finally:
        docker("rm", package_container)
    observed = (output / "packages.tsv").read_bytes()
    if inspect_inventory:
        differences = inventory_drift((context / "packages.lock.tsv").read_bytes(), observed)
        report = {"schema_version": 1, "status": "diagnostic_only",
                  "platform": lock["platform"], "image_id": package_image,
                  "ubuntu_snapshot": lock["ubuntu_snapshot"],
                  "package_lock_sha256": digest(context / "packages.lock.tsv"),
                  "packages_sha256": digest(output / "packages.tsv"),
                  "drift": differences}
        (output / "inventory-drift.json").write_text(
            json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print("Inventory diagnosis:", output / "inventory-drift.json", flush=True)
        return
    if observed != (context / "packages.lock.tsv").read_bytes():
        raise ValueError("Installed package inventory differs from the reviewed full lock")
    provenance = capture_apt_provenance(package_image, output, lock)
    image = build_stage(lock, context, output / "image-id", "verified")
    container = docker("create", "--network", "none", "--label",
                       "io.local-store.purpose=engine-payload-export", image)
    try:
        # Keep Linux symlinks inside a tar; extracting them on Windows requires
        # privileges that a payload build should never need.
        with (output / "notices.tar").open("wb") as notices:
            subprocess.run(docker_command("cp", container + ":/usr/share/doc", "-"),
                           stdout=notices, check=True, timeout=120)
        inventory = parse_inventory(observed)
        for p in lock["packages"]:
            if inventory.get(p["name"]) != (p["version"], p["architecture"]):
                raise ValueError("Installed package version differs from lock: " + p["name"])
        versions = {}
        for name, args in {"engine": ["dockerd", "--version"], "cli": ["docker", "--version"],
                           "compose": ["docker", "compose", "version", "--short"],
                           "containerd": ["containerd", "--version"], "runc": ["runc", "--version"]}.items():
            versions[name] = docker("run", "--rm", "--network", "none", "--read-only", image, *args)
        archive = output / "rootfs.tar"
        docker("export", "--output", str(archive), container, timeout=300)
        evidence = {"schema_version": 1, "status": "development_build_only",
                    "ubuntu_snapshot": lock["ubuntu_snapshot"],
                    "component_lock_sha256": digest(context / "components.lock.json"),
                    "build_inputs_sha256": {name: digest(context / name) for name in
                                            ("Dockerfile", "wsl.conf", "daemon.json",
                                             "components.lock.json", "packages.lock.tsv")},
                    "image_id": image, "platform": lock["platform"], "versions": versions,
                    "rootfs_sha256": digest(archive), "rootfs_bytes": archive.stat().st_size,
                    "packages_sha256": digest(output / "packages.tsv"),
                    "package_lock_sha256": digest(context / "packages.lock.tsv"),
                    "ubuntu_provenance_sha256": digest(output / "ubuntu-provenance.json"),
                    "ubuntu_archive_count": len(provenance["archives"]),
                    "ubuntu_index_count": len(provenance["indexes"]),
                    "notices_sha256": digest(output / "notices.tar"),
                    "package_count": len(inventory), "wsl_boot_tested": False,
                    "daemon_started": False, "signed": False,
                    "limitations": lock["limitations"]}
        (output / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
        print("Payload and evidence:", output, flush=True)
    finally:
        # Only the exact container created above. No volume, image or global prune.
        docker("rm", container)


def main():
    global MANAGED_ENGINE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="download, build and export; default only validates")
    parser.add_argument("--inspect-inventory", action="store_true",
                        help="build the unverified package stage and report drift; never export")
    parser.add_argument("--managed-engine", action="store_true",
                        help="use the verified local development WSL engine; Docker Desktop is not needed")
    args = parser.parse_args()
    lock = json.loads((ENGINE / "components.lock.json").read_text(encoding="utf-8"))
    validate(lock)
    if args.build and args.inspect_inventory:
        parser.error("--build and --inspect-inventory are mutually exclusive")
    if args.build or args.inspect_inventory:
        if args.managed_engine:
            verify_managed_builder()
            MANAGED_ENGINE = True
        build(lock, inspect_inventory=args.inspect_inventory)
    else:
        print("Engine pins valid; no build or release qualification implied.")


if __name__ == "__main__":
    main()
