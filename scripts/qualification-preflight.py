#!/usr/bin/env python3
"""Offline queue for the 52 Windows V1 offerings; never reports an app as proven."""

import argparse
from collections import Counter
from datetime import date, datetime, timezone
from hashlib import sha256
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load(relative):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


def age(day, today):
    try:
        delta = (today - date.fromisoformat(day)).days
        return delta if delta >= 0 else None
    except (ValueError, TypeError):
        return None


def inspect(app, suggestion, qualified, today):
    manifest_path = app["manifest"]
    manifest_bytes = (ROOT / manifest_path).read_bytes()
    manifest = json.loads(manifest_bytes)
    issues = []
    if manifest.get("id") != app["id"]:
        issues.append("manifest_id_changed")
    if manifest_path.startswith("src/templates/") and manifest.get("promotion", {}).get("state") != "approved":
        issues.append("promotion_changed")
    if age(manifest.get("verified_at"), today) is None or age(manifest["verified_at"], today) > 90:
        issues.append("source_review_stale")
    requirements = manifest.get("requirements", {})
    if manifest_path.startswith("src/templates/"):
        images = requirements.get("images", [])
    else:
        images = [{**requirements.get("image_audit", {}), "image": manifest.get("image"),
                   "container_platforms": requirements.get("container_platforms", [])}]
    if not images or any(not image.get("image") for image in images):
        issues.append("image_inventory_missing")
    if any(age(image.get("checked_at"), today) is None or age(image["checked_at"], today) > 30 for image in images):
        issues.append("image_review_stale")
    if any("linux/amd64" not in image.get("container_platforms", []) for image in images):
        issues.append("windows_engine_platform_unreviewed")
    if not suggestion or suggestion.get("acceptance_task") != app["acceptance_task"]:
        issues.append("task_triage_stale")
    return {
        "id": app["id"], "fixture_suggestion": suggestion.get("suggested_fixture") if suggestion else None,
        "fixture_review": suggestion.get("review") if suggestion else None,
        "task_qualified": app["id"] in qualified,
        "manifest_sha256": sha256(manifest_bytes).hexdigest(),
        "images": [image.get("image") for image in images], "preflight_issues": issues,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="include each app in machine-readable output")
    args = parser.parse_args()
    roster_bytes = (ROOT / "catalog/v1-roster.json").read_bytes()
    roster = json.loads(roster_bytes)
    apps = [app for app in roster["apps"] if app.get("cohort") == "existing_offering"]
    if len(apps) != 52 or len({app["id"] for app in apps}) != 52:
        parser.error("the 52-offering V1 baseline changed")
    triage = load("catalog/v1-task-triage.json")
    if triage.get("roster_sha256") != sha256(roster_bytes).hexdigest():
        parser.error("task triage is stale; rerun or review it")
    suggestions = {app["id"]: app for app in triage["apps"]}
    if len(suggestions) != 52:
        parser.error("task triage does not cover the 52 existing offerings")
    qualified = {app["id"] for app in load("catalog/v1-qualified-apps.json")["apps"]}
    rows = [inspect(app, suggestions.get(app["id"]), qualified, datetime.now(timezone.utc).date()) for app in apps]
    issues = Counter(issue for row in rows for issue in row["preflight_issues"])
    groups = Counter(row["fixture_suggestion"] or "missing" for row in rows)
    result = {"offered": len(rows), "task_qualified_recorded": sum(row["task_qualified"] for row in rows),
              "triage_reviewed": sum(row["fixture_review"] not in (None, "unreviewed") for row in rows),
              "fixture_groups": dict(sorted(groups.items())), "preflight_issue_counts": dict(sorted(issues.items())),
              "preflight_clean": sum(not row["preflight_issues"] for row in rows)}
    if args.json:
        result["apps"] = rows
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
