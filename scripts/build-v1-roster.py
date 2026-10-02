"""Build the frozen V1 planning matrix from reviewed membership and repo evidence.

Offline; never qualifies, installs or promotes an app. --check refuses drift.
Membership changes belong in catalog/v1-roster-selection.json, not the ranking.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def identity(url):
    parsed = urlsplit(url)
    return parsed.netloc.lower() + parsed.path.lower().rstrip("/").removesuffix(".git")


def build():
    selection = read("catalog/v1-roster-selection.json")
    upstream = read("catalog/v1-roster-upstream-evidence.json")
    # The frozen member IDs survive a separately reviewed candidate promotion.
    # New cohort counts must be explicit in the reviewed selection diff.
    cohorts = selection.get("reviewed_cohorts", {"existing_offering": 52, "expansion_candidate": 48})
    assert set(cohorts) == {"existing_offering", "expansion_candidate"}
    assert all(type(count) is int and count >= 0 for count in cohorts.values()) and sum(cohorts.values()) == 100
    promoted = upstream.get("promoted", {})
    assert not set(upstream["selected"]) & set(promoted), "upstream candidate evidence duplicated across cohorts"
    catalog = {c["id"]: c for c in read("src/generated/catalog.json")["entries"]}
    queue = read("catalog/candidate-queue.json")["candidates"]
    ranking = read("catalog/candidate-ranking.json")["candidates"]
    templates = {p.stem: read(str(p.relative_to(ROOT))) for p in (ROOT / "src/templates").glob("*.json")}
    recipes = {p.stem: read(str(p.relative_to(ROOT))) for p in (ROOT / "src/recipes").glob("*.json")}
    offered = set(recipes) | {key for key, value in templates.items() if value["promotion"]["state"] == "approved"}
    apps = selection["apps"]
    assert len(apps) == selection["target_count"] == 100, "roster must contain exactly 100 apps"
    assert len({a["id"] for a in apps}) == 100, "duplicate app ID"
    assert len({a["catalog_id"] for a in apps}) == 100, "duplicate canonical catalog entry"
    assert {a["id"] for a in apps if a["cohort"] == "existing_offering"} == offered, "offering membership changed; review roster"
    assert Counter(a["cohort"] for a in apps) == cohorts, "cohort counts changed; review selection counts"
    assert {a["id"] for a in apps if a["cohort"] == "expansion_candidate"} == set(upstream["selected"])
    assert set(promoted).issubset(offered), "promoted upstream snapshots must name approved offerings"
    seen = set()
    rows = []
    for app in apps:
        ident = app["id"]
        entry = catalog[app["catalog_id"]]
        manifest = templates.get(ident) or recipes.get(ident)
        existing = app["cohort"] == "existing_offering"
        repo = upstream["selected"].get(ident) or promoted.get(ident)
        if repo:
            assert not repo["isArchived"] and not repo["isDisabled"], f"{ident}: upstream is unavailable"
        canonical = identity(repo["url"] if repo else manifest["source_url"])
        assert canonical not in seen, f"duplicate project identity: {canonical}"
        seen.add(canonical)
        candidates = [c for c in queue if app["candidate_id"] is not None and c["id"] == app["candidate_id"]]
        candidates.sort(key=lambda c: (not c["importable"], c["identity"].startswith("unresolved:"), c.get("service_count") or 100000, c["source"]))
        preferred = candidates[0] if candidates else None
        rank = next((i + 1 for i, c in enumerate(ranking) if c["id"] == app["candidate_id"]), None)
        audits = []
        evidence = []
        if existing:
            if ident in templates:
                audits = manifest["requirements"]["images"]
                proof = manifest["lifecycle_proof"]
                assert (ROOT / proof).is_file(), f"missing proof: {proof}"
                evidence.append(proof)
            else:
                audits = [{"image": manifest["image"], **manifest["requirements"]["image_audit"], "container_platforms": manifest["requirements"]["container_platforms"]}]
                evidence.append("docs/evidence/recipe-lifecycle-windows-2026-09-07.json")
        fields = preferred.get("fields") if preferred else None
        if existing and manifest.get("config"):
            config = json.loads(manifest["config"]["content"])
            # Do not export defaults, environment values or credential material.
            fields = [{"key": f.get("env_variable"), "type": f.get("type"), "required": f.get("required"), "generated": f.get("type") == "random"} for f in config.get("form_fields", [])]
        source_path = ("src/templates/" if ident in templates else "src/recipes/") + ident + ".json" if manifest else None
        deployment = [{"source": c["source"], "id": c["id"], "importable_snapshot": c["importable"], "blockers": c["blockers"], "provenance": c["provenance"], "images_snapshot": c.get("images"), "service_count": c.get("service_count")} for c in candidates]
        risk_notes = manifest.get("risk_notes", []) if manifest else []
        methods = ["review_app_api_or_official_mcp", "isolated_browser_fallback"]
        if ident in {"homer", "glance", "homepage", "flatnotes", "silverbullet", "file-browser-quantum"}:
            methods.append("review_scoped_app_files")
        if ident == "ollama":
            methods = ["review_app_api"]
        task = app["acceptance_task"]
        assert task and app["selection_reason"]
        rows.append({
            **app, "name": entry["name"], "category": entry["category"],
            "canonical_project": canonical, "source_url": repo["url"] if repo else manifest["source_url"],
            "license_snapshot": entry["licenses"], "ranking_position_snapshot": rank,
            "manifest": source_path, "current_manifest_promotion": manifest.get("promotion", {}).get("state") if manifest else None,
            "deployment_candidates": deployment,
            "preferred_candidate_for_review": None if not preferred else preferred["source"] + ":" + preferred["id"],
            "deployment_status": "offered_baseline" if existing else "candidate_review_required" if preferred else "deployment_discovery_required",
            "setup": {"fields_snapshot": fields, "first_run_and_external_requirements": "must_prove_with_app_task", "baseline_risk_notes": risk_notes},
            "resources": {"measured_ram_mb": None, "measured_disk_mb": None, "startup_seconds": None, "status": "not_measured_by_roster; Q02 required", "baseline_storage": manifest.get("data_storage") if manifest else None},
            "maintenance": {"repository_check": repo, "repository_checked_at": upstream["checked_at"] if repo else None, "reviewed_image_audits": audits, "image_refresh_and_v1_review": "required; repository activity does not qualify image pins"},
            "engine_platform": {"target": "Windows x64 / managed WSL Linux amd64", "managed_engine_proof": "not_run", "baseline_image_platforms": sorted({p for a in audits for p in a.get("container_platforms", [])})},
            "proof": {"baseline_evidence": evidence, "level": "recorded_baseline_lifecycle; not_v1_certification" if existing else "candidate_only", "v1_task_result": "not_run"},
            "agent_access": {"status": "not_implemented", "proposed_methods_to_review": methods, "acceptance_task": task, "grant_login_and_revocation_proof": "required"},
            "v1_acceptance": {gate: "pending" for gate in ["maintenance_and_license_review", "managed_engine_install", "all_service_health", "resource_measurement", "first_use_task", "persistence_and_cleanup", "agent_useful_access", "agent_permission_denial", "v3_flow"]},
        })
    inputs = ["catalog/v1-roster-selection.json", "catalog/v1-roster-upstream-evidence.json", "catalog/candidate-queue.json", "catalog/candidate-ranking.json", "src/generated/catalog.json"]
    inputs += [str(p.relative_to(ROOT)).replace("\\", "/") for folder in ("src/templates", "src/recipes") for p in sorted((ROOT / folder).glob("*.json"))]
    matrix = {"schema_version": 1, "roster_version": selection["roster_version"], "frozen_at": selection["frozen_at"], "scope": "Planning matrix only; no app promotion or universal access claim", "input_sha256": {p: hashlib.sha256((ROOT / p).read_bytes().replace(b"\r\n", b"\n")).hexdigest() for p in inputs}, "summary": {"total": len(rows), "existing_offerings": len(offered), "expansion_candidates": cohorts["expansion_candidate"], "v1_qualified": 0, "agent_access_implemented": 0}, "apps": rows}
    lines = ["# Frozen V1 app roster", "", "Generated by `python scripts/build-v1-roster.py`; verify with `--check`.", "", f'**100 distinct planning members: {cohorts["existing_offering"]} existing offerings + {cohorts["expansion_candidate"]} expansion candidates.**', "Membership is frozen for planning; it is not approval to install new candidates.", "The full [acceptance matrix](../catalog/v1-roster.json) records source choices,", "setup fields, baseline image audits, resources, engine/proof and agent gates.", "All managed-engine and agent-access gates remain pending. Resource numbers are", "unmeasured; no RAM/disk estimate is invented. See [V1 tasks](V1_TASKS.md).", "", "## Selection and changes", "", "Retain the " + ("original" if promoted else "current") + " 52 and broaden non-AI categories using the pinned catalog", "and reach ranking as inputs, not as an automatic promotion score. CPU Ollama", "is one project, not separate CPU/GPU offerings. Aliased catalog/recipe IDs are", "explicitly reconciled. Forty-eight expansion repositories were checked via", "GitHub on September 12; activity does not prove current image maintenance.", "", "File Browser and Pingvin Share were excluded after the live check reported", "archived repositories; File Browser Quantum and Audiobookshelf take those", "candidate slots. Existing stale/withheld manifests (e.g. Actual) retain their", "status: selection requires new review, not approval of their old pins.", "", "Change membership only through a reviewed selection diff, explain exclusions", "and replacements, then regenerate. Do not silently fill failures with the next", "ranked row. All 100 must pass applicable V1 gates or require an owner scope decision.", ""]
    if promoted:
        lines += ["Reviewed promotions change cohort counts while preserving the same 100 members.", "Their original repository snapshots remain under `promoted` in the upstream", "evidence file. The separately selected ten launch apps remain unchanged.", ""]
    for cohort, title in [("existing_offering", "Existing offerings: requalification required"), ("expansion_candidate", "Expansion candidates: deployment and qualification required")]:
        lines += ["## " + title, "", "| App | Category | Deployment review | Useful task to prove |", "| --- | --- | --- | --- |"]
        for row in rows:
            if row["cohort"] != cohort:
                continue
            route = row["manifest"] if cohort == "existing_offering" else row["preferred_candidate_for_review"] or "Find/review deployment"
            fields = [f'[{row["name"]}]({row["source_url"]})', row["category"], route, row["acceptance_task"]]
            lines.append("| " + " | ".join(str(f).replace("|", "\\|") for f in fields) + " |")
        lines.append("")
    return {"catalog/v1-roster.json": json.dumps(matrix, indent=2, ensure_ascii=False) + "\n", "docs/v1-app-roster.md": "\n".join(lines).rstrip() + "\n"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for path, content in build().items():
        target = ROOT / path
        if args.check:
            if not target.is_file() or target.read_text(encoding="utf-8") != content:
                raise SystemExit("V1 roster is stale: " + path)
        else:
            target.write_text(content, encoding="utf-8", newline="\n")
    cohorts = read("catalog/v1-roster-selection.json").get("reviewed_cohorts", {"existing_offering": 52, "expansion_candidate": 48})
    print(f'V1 roster: 100 distinct apps; {cohorts["existing_offering"]} offerings + {cohorts["expansion_candidate"]} candidates; no V1/agent qualification claimed.')


if __name__ == "__main__":
    main()
