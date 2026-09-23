#!/usr/bin/env python3
"""Fail closed on stale or mislabelled V1 app-task evidence.

Default mode validates the curated list and reports its count. --release-gate
also fails until 50 approved offerings have current managed-engine task proof.
This is a ledger validator, not a substitute for a reviewer assessing each task.
"""

import argparse
from datetime import date, datetime, timezone
from hashlib import sha256
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
REQUIRED_STEPS = {
    "installs with one action",
    "survives a restart",
    "is usable first install",
    "is usable after restart",
    "reinstalls over data it kept",
    "is usable after a keep-data reinstall",
    "removes everything it created",
    "leaves other containers alone",
}


def read_json(root, path):
    return json.loads((root / path).read_text(encoding="utf-8"))


def file_bytes(root, relative, prefix):
    if not isinstance(relative, str) or not relative.startswith(prefix):
        raise ValueError(f"path must be under {prefix}")
    path = (root / relative).resolve()
    if not path.is_relative_to((root / prefix).resolve()) or not path.is_file():
        raise ValueError(f"missing or escaped path: {relative}")
    return path.read_bytes()


def digest(data):
    return sha256(data).hexdigest()


def require_digest(actual, expected, label):
    if not isinstance(expected, str) or not HEX64.fullmatch(expected) or actual != expected:
        raise ValueError(f"{label} hash changed; review and requalify")


def fresh_day(value, max_days, today):
    try:
        observed = date.fromisoformat(value)
    except (TypeError, ValueError):
        return False
    return 0 <= (today - observed).days <= max_days


def validate_entry(root, entry, roster, now):
    app_id = entry.get("id")
    if app_id not in roster:
        raise ValueError(f"{app_id}: not an existing approved offering in the V1 roster")
    manifest_path = entry.get("manifest")
    if manifest_path != roster[app_id].get("manifest"):
        raise ValueError(f"{app_id}: manifest differs from the V1 roster")
    prefix = "src/recipes/" if manifest_path.startswith("src/recipes/") else "src/templates/"
    manifest_bytes = file_bytes(root, manifest_path, prefix)
    require_digest(digest(manifest_bytes), entry.get("manifest_sha256"), f"{app_id} manifest")
    manifest = json.loads(manifest_bytes)
    if manifest.get("id") != app_id:
        raise ValueError(f"{app_id}: manifest identity changed")

    evidence_bytes = file_bytes(root, entry.get("evidence"), "docs/evidence/")
    require_digest(digest(evidence_bytes), entry.get("evidence_sha256"), f"{app_id} evidence")
    evidence = json.loads(evidence_bytes)
    identity = evidence.get("identity") or {}
    if evidence.get("app") != app_id or evidence.get("schema_version") != 2 or evidence.get("passed") is not True:
        raise ValueError(f"{app_id}: no passing schema-2 proof")
    if identity.get("host_os") != "windows" or identity.get("host_arch") != "x86_64" or identity.get("engine") != {
        "schema_version": 2,
        "program": "wsl.exe",
        "endpoint": "wsl://local-store-engine-v1",
    }:
        raise ValueError(f"{app_id}: proof is not from the selected managed Windows engine")
    if prefix == "src/recipes/":
        expected_source = manifest.get("source_url")
        expected_adapter = "recipe"
        expected_revision = ""
        expected_images = [manifest.get("image")]
        expected_image_day = manifest.get("requirements", {}).get("image_audit", {}).get("checked_at")
    else:
        origin = manifest.get("origin", {})
        expected_source = f"{origin.get('repository')}#{origin.get('path')}"
        expected_adapter = origin.get("importer")
        expected_revision = origin.get("revision")
        images = manifest.get("requirements", {}).get("images", [])
        expected_images = [image.get("image") for image in images]
        expected_image_day = min((image.get("checked_at", "") for image in images), default="")
        if manifest.get("promotion", {}).get("state") != "approved":
            raise ValueError(f"{app_id}: template is not approved")
    if (identity.get("source_locator") != expected_source
            or identity.get("source_adapter") != expected_adapter
            or identity.get("source_revision") != expected_revision
            or set(identity.get("requested_images", [])) != set(expected_images)):
        raise ValueError(f"{app_id}: source or requested images differ from the reviewed manifest")
    if not isinstance(identity.get("resolved_image_ids"), dict) or not identity["resolved_image_ids"]:
        raise ValueError(f"{app_id}: no resolved image identity")

    today = datetime.fromtimestamp(now, timezone.utc).date()
    recorded = evidence.get("recorded_at_unix")
    if not isinstance(recorded, int) or not 0 <= now - recorded <= 30 * 86400:
        raise ValueError(f"{app_id}: proof is stale or from the future")
    if not fresh_day(identity.get("source_observed_on"), 90, today) or not fresh_day(identity.get("images_observed_on"), 30, today):
        raise ValueError(f"{app_id}: source or image observation is stale")
    if identity.get("source_observed_on") != manifest.get("verified_at") or identity.get("images_observed_on") != expected_image_day:
        raise ValueError(f"{app_id}: observation dates differ from the reviewed manifest")

    task = entry.get("task")
    probe_bytes = file_bytes(root, entry.get("probe"), "scripts/")
    if not isinstance(task, str) or len(task) < 30 or evidence.get("first_use") != task:
        raise ValueError(f"{app_id}: task proof description changed")
    probe_material = task.encode() + b"\0script-source:" + probe_bytes
    require_digest(digest(probe_material), identity.get("probe_sha256"), f"{app_id} probe")
    steps = evidence.get("steps")
    if not isinstance(steps, list) or not all(isinstance(step, dict) and step.get("passed") is True for step in steps):
        raise ValueError(f"{app_id}: a qualification step failed")
    if not REQUIRED_STEPS.issubset({step.get("step") for step in steps}):
        raise ValueError(f"{app_id}: required lifecycle or task steps are absent")
    measurements = evidence.get("measurements") or {}
    if measurements.get("samples", 0) < 3 or measurements.get("idle_named_volume_bytes") is None:
        raise ValueError(f"{app_id}: resource proof is incomplete")


def validate(root, now):
    roster_data = read_json(root, "catalog/v1-roster.json")
    roster = {app["id"]: app for app in roster_data["apps"] if app.get("cohort") == "existing_offering"}
    if len(roster) != 52:
        raise ValueError("the 52-offering V1 baseline has changed")
    curated = read_json(root, "catalog/v1-qualified-apps.json")
    if curated.get("schema_version") != 1 or curated.get("target") != 50 or not isinstance(curated.get("apps"), list):
        raise ValueError("invalid V1 qualification ledger")
    ids = [entry.get("id") for entry in curated["apps"]]
    if len(ids) != len(set(ids)) or len(ids) > 52:
        raise ValueError("duplicate or excessive V1 qualification entries")
    for entry in curated["apps"]:
        validate_entry(root, entry, roster, now)
    return {"qualified": len(ids), "target": 50, "remaining": max(0, 50 - len(ids)), "ids": sorted(ids)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release-gate", action="store_true", help="fail while fewer than 50 apps qualify")
    args = parser.parse_args()
    try:
        result = validate(ROOT, int(datetime.now(timezone.utc).timestamp()))
    except (ValueError, KeyError, TypeError, OSError, json.JSONDecodeError) as error:
        print(f"V1 qualification ledger refused: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return int(args.release_gate and result["qualified"] < result["target"])


if __name__ == "__main__":
    raise SystemExit(main())
