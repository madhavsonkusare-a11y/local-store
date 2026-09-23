#!/usr/bin/env python3
"""Suggest reusable proof fixtures for the 52 V1 tasks; never qualify an app.

Only public roster fields are sent to TypeSafe. The key is read from a file or
TYPESAFE_API_KEY and is never written to the output. Review every suggestion.
"""

import argparse
from datetime import datetime, timezone
from hashlib import sha256
import json
import math
import os
from pathlib import Path
import sys
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
ROSTER = ROOT / "catalog/v1-roster.json"
OUTPUT = ROOT / "catalog/v1-task-triage.json"
ENDPOINT = "https://api.typesafe.ai/v1/systemone"
ARCHETYPES = {
    "content_roundtrip": "Create a note, document, record, or other content item and read it back after restart and reinstall.",
    "file_roundtrip": "Upload a synthetic file and download or read its exact bytes after restart and reinstall.",
    "workflow_run": "Create or configure a workflow, job, or automation and observe its controlled execution.",
    "monitoring": "Configure a monitor or metric source and observe a synthetic target or event.",
    "database_client": "Connect to an isolated fixture database and query a synthetic table.",
    "external_fixture": "Needs a controlled external service, model, device, feed, or account to prove a useful task.",
    "admin_setup": "A meaningful task is primarily initial account, configuration, or administrative setup.",
    "other": "None of the listed reusable task archetypes describes the acceptance task well.",
}


def request_for(apps):
    state = {app["id"]: {"name": app["name"], "category": app["category"],
                         "acceptance_task": app["acceptance_task"]} for app in apps}
    questions = {
        app["id"]: {
            "type": "choice",
            "instructions": f"Which reusable verification fixture best fits the acceptance task for `state.{app['id']}`? Choose other if none fits. This is planning, not proof that the app works.",
            "criteria": ARCHETYPES,
        } for app in apps
    }
    return {"model": "jev-latest", "state": state, "questions": questions}


def evaluate(body, key):
    request = Request(ENDPOINT, data=json.dumps(body).encode("utf-8"),
                      headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
                      method="POST")
    try:
        with urlopen(request, timeout=45) as response:
            return json.load(response)
    except HTTPError as error:
        raise RuntimeError(f"TypeSafe HTTP {error.code}; no app was qualified") from None
    except URLError as error:
        raise RuntimeError(f"TypeSafe connection failed ({type(error.reason).__name__}); no app was qualified") from None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--key-file", type=Path, help="file containing the TypeSafe API key")
    parser.add_argument("--limit", type=int, default=52, help="pilot on the first N existing offerings")
    parser.add_argument("--dry-run", action="store_true", help="show public request shape without an API call")
    args = parser.parse_args()
    if not 1 <= args.limit <= 52:
        parser.error("--limit must be 1..52")
    roster_bytes = ROSTER.read_bytes()
    apps = [app for app in json.loads(roster_bytes)["apps"] if app.get("cohort") == "existing_offering"]
    if len(apps) != 52 or len({app["id"] for app in apps}) != 52:
        parser.error("the reviewed 52-offering baseline changed")
    apps = apps[:args.limit]
    if args.dry_run:
        print(json.dumps({"apps": len(apps), "archetypes": list(ARCHETYPES),
                          "public_fields": ["id", "name", "category", "acceptance_task"],
                          "requests": (len(apps) + 7) // 8}, sort_keys=True))
        return 0
    key = args.key_file.read_text(encoding="utf-8").strip() if args.key_file else os.environ.get("TYPESAFE_API_KEY", "").strip()
    if key.startswith("TYPESAFE_API_KEY="):
        key = key.partition("=")[2].strip().strip('"')
    if not key or any(char.isspace() for char in key):
        parser.error("a single-line TypeSafe key is required")
    rows = []
    models = set()
    total_tokens = 0
    for start in range(0, len(apps), 8):
        batch = apps[start:start + 8]
        result = evaluate(request_for(batch), key)
        models.add(result["model"])
        total_tokens += sum(result.get("usage", {}).get(field, 0) or 0 for field in ("input_tokens", "output_tokens"))
        for app in batch:
            answer = result["answers"][app["id"]]
            choice = answer["choice"]
            probabilities = answer["probabilities"]
            confidence = answer["confidence"]
            if (answer["type"] != "choice" or choice not in ARCHETYPES
                    or set(probabilities) != set(ARCHETYPES)
                    or not isinstance(confidence, (int, float)) or not math.isfinite(confidence)
                    or not 0 <= confidence <= 1
                    or any(not isinstance(value, (int, float)) or not math.isfinite(value)
                           or not 0 <= value <= 1 for value in probabilities.values())
                    or abs(sum(probabilities.values()) - 1) > 0.02):
                raise RuntimeError(f"invalid TypeSafe answer for {app['id']}; no output written")
            rows.append({"id": app["id"], "acceptance_task": app["acceptance_task"],
                         "suggested_fixture": choice, "confidence": confidence,
                         "probabilities": probabilities, "review": "unreviewed"})
    output = {"schema_version": 1, "meaning": "triage suggestions only; never release proof",
              "roster_sha256": sha256(roster_bytes).hexdigest(),
              "generated_at": datetime.now(timezone.utc).isoformat(),
              "models": sorted(models), "api_tokens": total_tokens, "apps": rows}
    destination = OUTPUT if args.limit == 52 else ROOT / ".cache/v1-task-triage-pilot.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"triaged": len(rows), "output": str(destination.relative_to(ROOT)),
                      "model": sorted(models), "review_required": len(rows), "api_tokens": total_tokens}))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, KeyError, ValueError, RuntimeError, json.JSONDecodeError) as error:
        print(f"Task triage failed: {error}", file=sys.stderr)
        sys.exit(1)
