"""Validate engine pins, or build/export an isolated development WSL rootfs.

No WSL registration, privileged container, host socket mount or daemon change.
Outputs stay in .cache/engine; this is not a signed release artifact.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.request
from urllib.parse import urlsplit
import uuid

ROOT = Path(__file__).resolve().parents[1]
ENGINE = ROOT / "engine"
EXPECTED = {"docker-ce", "docker-ce-cli", "containerd.io",
            "docker-compose-plugin", "docker-buildx-plugin"}
PACKAGE_LOCK = ENGINE / "packages.lock.tsv"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def validate(lock):
    if lock.get("schema_version") != 1 or lock.get("platform") != "linux/amd64":
        raise ValueError("Unsupported engine lock schema/platform")
    if not re.fullmatch(r"ubuntu:24\.04@sha256:[0-9a-f]{64}", lock["base_image"]):
        raise ValueError("Base image must be an exact Ubuntu 24.04 digest")
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
    return subprocess.check_output(["docker", *args], text=True, encoding="utf-8", timeout=timeout).strip()


def build(lock):
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
    image_file = output / "image-id"
    subprocess.run(["docker", "build", "--platform", lock["platform"],
                    "--build-arg", "BASE_IMAGE=" + lock["base_image"],
                    "--iidfile", str(image_file), str(context)], check=True, timeout=1200)
    image = image_file.read_text().strip()
    details = json.loads(docker("image", "inspect", image))[0]
    if details["Os"] + "/" + details["Architecture"] != lock["platform"]:
        raise ValueError("Built platform differs from lock")
    container = docker("create", "--network", "none", "--label",
                       "io.local-store.purpose=engine-payload-export", image)
    try:
        docker("cp", container + ":/usr/share/local-store/packages.tsv", str(output / "packages.tsv"))
        # Keep Linux symlinks inside a tar; extracting them on Windows requires
        # privileges that a payload build should never need.
        with (output / "notices.tar").open("wb") as notices:
            subprocess.run(["docker", "cp", container + ":/usr/share/doc", "-"],
                           stdout=notices, check=True, timeout=120)
        observed = (output / "packages.tsv").read_bytes()
        if observed != (context / "packages.lock.tsv").read_bytes():
            raise ValueError("Installed package inventory differs from the reviewed full lock")
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
                    "component_lock_sha256": digest(context / "components.lock.json"),
                    "build_inputs_sha256": {name: digest(context / name) for name in
                                            ("Dockerfile", "wsl.conf", "daemon.json",
                                             "components.lock.json", "packages.lock.tsv")},
                    "image_id": image, "platform": lock["platform"], "versions": versions,
                    "rootfs_sha256": digest(archive), "rootfs_bytes": archive.stat().st_size,
                    "packages_sha256": digest(output / "packages.tsv"),
                    "package_lock_sha256": digest(context / "packages.lock.tsv"),
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true", help="download, build and export; default only validates")
    args = parser.parse_args()
    lock = json.loads((ENGINE / "components.lock.json").read_text(encoding="utf-8"))
    validate(lock)
    if args.build:
        build(lock)
    else:
        print("Engine pins valid; no build or release qualification implied.")


if __name__ == "__main__":
    main()
