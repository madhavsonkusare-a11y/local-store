#!/usr/bin/env python3
"""Opt-in real Windows supervisor proof; no imports, upgrades or VM deletion.

Uses an isolated profile containing only existing verified ownership/selection.
The actual launcher executable exits, leaving its exact owned background worker.
Only fixture selection is removed to revoke that worker. Docker proof slots
must be serialized by the caller. This is not a native WebView/sleep-wake proof.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

DISTRO = "local-store-engine-v1"
STATE_FILES = ("bootstrap.json", "ownership-token", "selected-engine.json", "engine-selection-configured")


def run(args: list[str], env: dict[str, str], timeout: int = 120) -> subprocess.CompletedProcess[str]:
    process = subprocess.Popen(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, encoding="utf-8", errors="replace")
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        # subprocess.run retries communicate without a timeout on Windows.
        # If a descendant retained an output handle, that retry never returns,
        # preventing our exact fixture-selection revocation in finally. Wait
        # only for this owned command handle; let fixture cleanup stop its worker.
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)
        raise
    return subprocess.CompletedProcess(args, process.returncode, stdout, stderr)


def product_json(binary: Path, args: list[str], env: dict[str, str]):
    result = run([str(binary), *args], env)
    if result.returncode:
        # Doctor is already redacted; do not echo unrelated subprocess data.
        raise RuntimeError(f"Local Store {' '.join(args)} failed with exit {result.returncode}")
    return json.loads(result.stdout)


def containers(env: dict[str, str]) -> dict[str, str]:
    prefix = ["wsl.exe", "--distribution", DISTRO, "--user", "root", "--exec", "/usr/bin/env", "-i", "PATH=/usr/sbin:/usr/bin:/sbin:/bin", "/usr/bin/docker", "--host", "unix:///var/run/docker.sock"]
    ids = run([*prefix, "ps", "--all", "--quiet", "--no-trunc"], env)
    if ids.returncode:
        raise RuntimeError("Owned engine container inventory failed")
    result: dict[str, str] = {}
    for identifier in ids.stdout.split():
        if len(identifier) != 64 or any(char not in "0123456789abcdef" for char in identifier):
            raise RuntimeError("Container inventory contained an invalid ID")
        inspected = run([*prefix, "inspect", "--format", "{{.State.StartedAt}}|{{.State.Status}}", identifier], env)
        if inspected.returncode:
            raise RuntimeError("Owned container state inventory failed")
        result[identifier] = inspected.stdout.strip()
    return result


def bounded(path: Path, maximum: int) -> bytes:
    with path.open("rb") as handle:
        value = handle.read(maximum + 1)
    if len(value) > maximum:
        raise RuntimeError("Engine state exceeded its size bound")
    return value


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", help="Explicitly run the real Windows proof")
    args = parser.parse_args()
    if not args.run:
        parser.error("Use --run only after reserving the serial managed-engine proof slot")
    if os.name != "nt":
        raise RuntimeError("This proof requires Windows")
    root = Path(__file__).resolve().parents[1]
    binary = root / "target" / "release" / "local-store.exe"
    if not binary.is_file():
        raise RuntimeError("Build the current Local Store launcher first")
    native = Path(os.environ["LOCALAPPDATA"]) / "local-store" / "engine" / "wsl-state"
    original = {name: bounded(native / name, 16384) for name in STATE_FILES}
    journal = json.loads(original["bootstrap.json"])
    if journal.get("state") != "verified" or journal.get("distro") != DISTRO:
        raise RuntimeError("An existing verified owned engine is required")
    if original["ownership-token"] != journal.get("ownership_token", "").encode():
        raise RuntimeError("Native ownership marker does not match its journal")
    cache = root / ".cache" / "engine"
    cache.mkdir(parents=True, exist_ok=True)
    fixture = Path(tempfile.mkdtemp(prefix="supervisor-proof-", dir=cache))
    state = fixture / "local" / "local-store" / "engine" / "wsl-state"
    state.mkdir(parents=True)
    for name, contents in original.items():
        (state / name).write_bytes(contents)
    env = dict(os.environ)
    env["LOCALAPPDATA"] = str(fixture / "local")
    env["APPDATA"] = str(fixture / "roaming")
    env["XDG_CONFIG_HOME"] = str(fixture / "xdg")
    env.pop("LOCAL_STORE_ENGINE_SUPERVISOR_PROCESS_ONLY", None)
    env.pop("WSLENV", None)
    receipt = None
    try:
        before = containers(env)
        if product_json(binary, ["engine", "supervisor-status"], env) is not None:
            raise RuntimeError("Fresh proof profile unexpectedly contains a supervisor")
        launched = time.monotonic()
        doctor = product_json(binary, ["doctor", "--json"], env)
        if not doctor.get("ready"):
            raise RuntimeError("The actual launcher did not report a ready engine")
        live = product_json(binary, ["engine", "supervisor-status"], env)
        if not live or not live.get("running"):
            raise RuntimeError("No independent supervisor survived launcher process exit")
        # This loop performs no WSL/Docker/app requests. Only the worker itself
        # holds/checks its pipe lease while the launcher process is absent.
        idle_started = time.monotonic()
        deadline = idle_started + 95
        while time.monotonic() < deadline:
            time.sleep(min(1, deadline - time.monotonic()))
        idle_seconds = round(time.monotonic() - idle_started, 2)
        after_idle = product_json(binary, ["engine", "supervisor-status"], env)
        if not after_idle or after_idle["process_id"] != live["process_id"]:
            raise RuntimeError("The exact independently launched worker did not survive the idle interval")
        after = containers(env)
        if before != after:
            raise RuntimeError("Existing container identities or start times changed")
        # Revoke only the fixture's choice. Never alter the native user marker.
        (state / "selected-engine.json").unlink()
        stopped = time.monotonic() + 40
        while time.monotonic() < stopped:
            if product_json(binary, ["engine", "supervisor-status"], env) is None:
                break
            time.sleep(0.5)
        else:
            raise RuntimeError("The fixture supervisor did not release after selection revocation")
        if any(bounded(native / name, 16384) != value for name, value in original.items()):
            raise RuntimeError("Native engine state changed during isolated supervision proof")
        receipt = {"schema_version": 1, "passed": True,
            "proof": "actual_launcher_cli_exit_owned_engine_supervision",
            "recorded_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "launcher_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "distribution": DISTRO, "launcher_exited_before_idle": True,
            "idle_seconds": idle_seconds,
            "supervisor_start_seconds": round(idle_started - launched, 2),
            "same_worker_survived": True, "existing_container_count": len(before),
            "container_identities_start_times_unchanged": True,
            "fixture_selection_revocation_stopped_worker": True,
            "native_engine_state_unchanged": True,
            "limits": ["CLI executable exit, not native WebView close", "No new app task", "No Windows sleep/wake or reboot", "No engine removal"]}
    finally:
        # Invalid/missing fixture selection makes this exact worker exit. Wait
        # for its file lock to release before deleting any remaining fixture.
        try:
            (state / "selected-engine.json").unlink(missing_ok=True)
            deadline = time.monotonic() + 40
            while time.monotonic() < deadline:
                if product_json(binary, ["engine", "supervisor-status"], env) is None:
                    shutil.rmtree(fixture)
                    break
                time.sleep(0.5)
            else:
                raise RuntimeError("Fixture retained: its exact worker is still running")
        except Exception:
            # Retain ownership/state for explicit recovery rather than killing
            # an unverified PID or hiding a surviving background process.
            raise RuntimeError("Supervisor proof fixture retained for recovery") from None
    if receipt:
        output = root / "docs" / "evidence" / "windows-engine-supervision-2026-10-02.json"
        output.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"passed": True, "receipt": str(output), "idle_seconds": receipt["idle_seconds"]}))


if __name__ == "__main__":
    main()
