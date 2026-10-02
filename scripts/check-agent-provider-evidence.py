#!/usr/bin/env python3
"""Validate the selected app-access receipts; never infer proof from code or a page."""
import argparse
import hashlib
import json
from pathlib import Path
import time

ROOT = Path(__file__).resolve().parents[1]
API_APPS = {"gitea", "immich", "jellyfin", "kanboard", "wordpress", "uptime-kuma"}


def expectation(app):
    if app in API_APPS:
        return f"{app}-agent-api-content-2026-10-02.json", {
            "provider_sha256": "src/agent_content/app_api.rs",
            "content_boundary_sha256": "src/agent_content.rs",
            "test_sha256": "tests/managed_app_api_content.rs",
            "fixture_sha256": "scripts/app-api-read-fixture.mjs",
        }
    if app == "memos":
        return "memos-agent-content-2026-10-01.json", {
            "provider_sha256": "src/agent_content.rs",
            "test_sha256": "tests/managed_agent_content.rs",
        }
    if app == "flatnotes":
        return "flatnotes-agent-files-2026-10-02.json", {
            "provider_sha256": "src/agent_data.rs",
            "test_sha256": "src/agent_data/real_test.rs",
        }
    if app in {"n8n", "privatebin"}:
        hashes = {
            "provider_sha256": f"src/agent_content/{app}.rs",
            "content_boundary_sha256": "src/agent_content.rs",
            "test_sha256": f"tests/managed_{app}_content.rs",
        }
        if app == "privatebin":
            hashes["runner_sha256"] = "providers/privatebin/runner.mjs"
            hashes["profile_sha256"] = "providers/privatebin/seccomp_profile.json"
        return f"{app}-agent-content-2026-10-02.json", hashes
    raise ValueError("Unreviewed provider")


def validate(app, receipt, inputs, now, root=ROOT):
    if type(receipt.get("schema_version")) is not int or receipt["schema_version"] != 1 or receipt.get("passed") is not True or receipt.get("failure") is not None:
        raise ValueError("No successful provider proof")
    observed = receipt.get("recorded_at_unix")
    if type(observed) is not int or observed > now + 300 or now - observed > 30 * 86400:
        raise ValueError("Provider proof date is missing, future or stale")
    if app == "flatnotes":
        if receipt.get("proof") != "flatnotes-scoped-markdown-files" or not all(receipt.get(key) is True for key in ["owned_cleanup", "bystanders_preserved", "native_engine_files_unchanged"]):
            raise ValueError("File scope or ownership cleanup is unproved")
    else:
        engine = receipt.get("engine", {})
        if receipt.get("app") != app or engine.get("schema_version") != 2 or engine.get("program") != "wsl.exe" or engine.get("endpoint") != "wsl://local-store-engine-v1":
            raise ValueError("App or managed-engine identity mismatch")
        if app in API_APPS and (receipt.get("cleanup_passed") is not True or receipt.get("bystanders_unchanged") is not True):
            raise ValueError("Fixture cleanup is unproved")
        if app == "privatebin" and receipt.get("browser_cleanup_passed") is not True:
            raise ValueError("Isolated browser cleanup is unproved")
    steps = receipt.get("steps")
    if not isinstance(steps, list) or len(steps) < 4 or not all(isinstance(step, str) for step in steps):
        raise ValueError("Useful task and refusal steps are missing")
    for field, file in inputs.items():
        if receipt.get(field) != hashlib.sha256((root / file).read_bytes()).hexdigest():
            raise ValueError("Provider or proof source changed")


def check(root=ROOT, now=None):
    now = int(time.time()) if now is None else now
    selected = json.loads((root / "catalog/v1-qualified-apps.json").read_text(encoding="utf-8"))
    result = {"target": selected["target"], "verified": [], "pending": []}
    for row in selected["apps"]:
        app = row["id"]
        try:
            filename, inputs = expectation(app)
            path = root / "docs/evidence" / filename
            if path.stat().st_size > 65536:
                raise ValueError("Oversized proof")
            validate(app, json.loads(path.read_text(encoding="utf-8")), inputs, now, root)
            result["verified"].append(app)
        except (OSError, ValueError, KeyError, TypeError):
            result["pending"].append(app)
    directory = json.loads((root / "catalog/agent-access.json").read_text(encoding="utf-8"))
    rows = {row["offering_id"]: row for row in directory["apps"]}
    for app in list(result["verified"]):
        method = {"flatnotes": "app_files", "n8n": "official_mcp", "privatebin": "isolated_browser"}.get(app, "typed_api")
        access = "verified_read_write" if app in {"memos", "n8n", "privatebin"} else "verified_read"
        row = rows.get(app, {})
        if row.get("content_access") != access or row.get("methods") != [method] or row.get("required_setup") != ("owner_grant" if app == "flatnotes" else "credential_grant"):
            result["verified"].remove(app)
            result["pending"].append(app)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--allow-pending", action="store_true")
    args = parser.parse_args()
    result = check()
    print(json.dumps(result, sort_keys=True))
    raise SystemExit(0 if args.allow_pending or len(result["verified"]) >= result["target"] else 1)
