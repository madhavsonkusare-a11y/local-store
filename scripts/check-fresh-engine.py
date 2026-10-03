#!/usr/bin/env python3
"""Opt-in fresh payload boot on an existing WSL2 host, not a clean Windows test.

Reserve the serial engine proof slot before running. Imports only a unique
fixture distro, verifies its token before unregistering, and retains uncertain
imports for recovery. Never changes the selected product engine or WSL features.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import tempfile
import time
import uuid

STATE_FILES = ("bootstrap.json", "ownership-token", "selected-engine.json", "engine-selection-configured")
MARKER = "/usr/share/local-store/qualification-owner"


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def command(args: list[str], *, data: bytes | None = None, timeout: int = 120) -> bytes:
    result = subprocess.run(args, input=data, capture_output=True, timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(f"Proof command failed (exit {result.returncode}); fixture retained if ownership is uncertain")
    return result.stdout


def distro_names() -> set[str]:
    raw = command(["wsl.exe", "--list", "--quiet"])
    text = raw.decode("utf-16-le" if b"\x00" in raw else "utf-8", errors="strict")
    return {name.strip().lstrip("\ufeff") for name in text.splitlines() if name.strip()}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    args = parser.parse_args()
    if not args.run or os.name != "nt":
        parser.error("Use --run on Windows after reserving the serial engine slot")
    root = Path(__file__).resolve().parents[1]
    payload = root / "target/release/engine/rootfs.tar"
    build = json.loads((root / "docs/evidence/engine-session-payload-development-2026-10-01.json").read_text())
    if payload.stat().st_size != build["rootfs_bytes"] or digest(payload) != build["rootfs_sha256"]:
        raise RuntimeError("Staged payload does not match the retained development export")
    native = Path(os.environ["LOCALAPPDATA"]) / "local-store/engine/wsl-state"
    original = {name: (native / name).read_bytes() for name in STATE_FILES}
    inventory = distro_names()
    name = "local-store-qualification-" + uuid.uuid4().hex
    if name in inventory:
        raise RuntimeError("Fresh fixture name already exists")
    cache = (root / ".cache/engine").resolve()
    cache.mkdir(parents=True, exist_ok=True)
    fixture = Path(tempfile.mkdtemp(prefix="fresh-boot-", dir=cache)).resolve()
    if fixture.parent != cache:
        raise RuntimeError("Fixture is outside the engine proof cache")
    install = fixture / "wsl"
    token = secrets.token_hex(32).encode()
    (fixture / "ownership-token").write_bytes(token)
    (fixture / "fixture.json").write_text(json.dumps({"distribution": name, "install_directory": str(install)}))
    prefix = ["wsl.exe", "--distribution", name, "--user", "root", "--exec", "/usr/bin/env", "-i", "PATH=/usr/sbin:/usr/bin:/sbin:/bin"]
    imported = False
    marker_set = False
    receipt = None
    started = time.monotonic()
    try:
        command(["wsl.exe", "--import", name, str(install), str(payload), "--version", "2"], timeout=240)
        imported = True
        imported_seconds = round(time.monotonic() - started, 2)
        command([*prefix, "tee", MARKER], data=token)
        command([*prefix, "chmod", "0400", MARKER])
        marker_set = True
        command([*prefix, "cmp", "--silent", MARKER, "-"], data=token)
        packages = command([*prefix, "dpkg-query", "-W", "-f=${Package}\t${Version}\t${Architecture}\n"])
        if packages != (root / "engine/packages.lock.tsv").read_bytes():
            raise RuntimeError("Fresh boot package inventory differs from the exact locked export")
        command([*prefix, "systemctl", "is-active", "docker"], timeout=90)
        info = json.loads(command([*prefix, "docker", "--host", "unix:///var/run/docker.sock", "info", "--format", "{{json .}}"], timeout=90))
        compose = command([*prefix, "docker", "compose", "version", "--short"]).decode().strip()
        if info.get("ServerVersion") != "29.8.0" or compose != "5.5.1":
            raise RuntimeError("Fresh daemon/Compose versions differ from the payload contract")
        vhd = install / "ext4.vhdx"
        receipt = {"schema_version": 1, "passed": True,
            "proof": "fresh_payload_import_on_existing_windows_wsl_host",
            "recorded_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "rootfs_sha256": build["rootfs_sha256"], "rootfs_bytes": payload.stat().st_size,
            "script_sha256": digest(Path(__file__)), "package_inventory_sha256": hashlib.sha256(packages).hexdigest(),
            "package_count": len(packages.splitlines()), "import_seconds": imported_seconds,
            "ready_seconds": round(time.monotonic() - started, 2),
            "fresh_vhd_file_bytes": vhd.stat().st_size, "engine_version": info["ServerVersion"],
            "compose_version": compose, "systemd_docker_active": True,
            "existing_engine_state_unchanged": False, "fixture_unregistered": False,
            "limits": ["Existing Windows host with WSL prerequisites already installed",
                "Direct WSL fixture import, not the product's fixed-name setup transaction",
                "No native WebView or failed-prerequisite proof", "No image pulls or app task",
                "VHD file length is an observation, not a supported minimum or physical allocated-size measurement"]}
    finally:
        if imported and marker_set:
            # Both the private external record and the live in-distro marker are
            # required. Never unregister by a name alone or terminate all WSL.
            if (fixture / "ownership-token").read_bytes() != token or name in inventory:
                raise RuntimeError("Fixture ownership changed; retained for recovery")
            command([*prefix, "cmp", "--silent", MARKER, "-"], data=token)
            command(["wsl.exe", "--unregister", name], timeout=120)
            if distro_names() != inventory:
                raise RuntimeError("Distribution inventory changed; proof fixture retained for review")
            if any((native / item).read_bytes() != value for item, value in original.items()):
                raise RuntimeError("Selected product engine state changed during proof")
            if receipt:
                receipt["fixture_unregistered"] = True
                receipt["existing_engine_state_unchanged"] = True
            shutil.rmtree(fixture)
        elif not imported and name not in distro_names():
            shutil.rmtree(fixture)
        else:
            raise RuntimeError("Unverified import retained in the engine proof cache for explicit recovery")
    if receipt:
        output = root / "docs/evidence/windows-fresh-payload-2026-10-02.json"
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"passed": True, "receipt": str(output), "ready_seconds": receipt["ready_seconds"], "vhd_file_bytes": receipt["fresh_vhd_file_bytes"]}))


if __name__ == "__main__":
    main()
