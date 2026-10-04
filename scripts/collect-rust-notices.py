"""Collect exact locked Rust dependency notices from the existing Cargo cache.

No build, lock update, dependency download or engine workload is performed.
--check compares committed outputs with the same cached package source files.
This is an all-target superset, not a compiled-binary dependency attestation.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "catalog/notices/rust"


def collect():
    result = subprocess.run(["cargo", "metadata", "--offline", "--locked", "--format-version", "1"], cwd=ROOT, capture_output=True, check=True)
    packages = sorted((p for p in json.loads(result.stdout)["packages"] if p["name"] != "local-store"), key=lambda p: (p["name"], p["version"]))
    supplements = json.loads((OUT / "upstream-supplements.json").read_text())["packages"]
    supplemental = {(r["name"], r["version"]): r for r in supplements}
    rows, sections, missing = [], [], []
    for package in packages:
        base = Path(package["manifest_path"]).parent
        candidates = []
        for path in base.rglob("*"):
            relative = path.relative_to(base)
            if len(relative.parts) > 3 or not path.is_file():
                continue
            name = path.name.lower()
            if name.startswith(("license", "licence", "notice", "copying", "copyright")) and path.suffix.lower() not in {".rs", ".py", ".html", ".json"}:
                candidates.append(path)
        declared = package.get("license_file")
        if declared and (base / declared).is_file():
            candidates.append(base / declared)
        files = []
        for path in sorted(set(candidates), key=lambda p: p.relative_to(base).as_posix()):
            data = path.read_bytes()
            text = data.decode("utf-8", errors="replace")
            relative = path.relative_to(base).as_posix()
            files.append({"path": relative, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
            sections.append(f"\n===== {package['name']} {package['version']} / {relative} =====\n{text.rstrip()}\n")
        extra = supplemental.get((package["name"], package["version"]))
        if extra:
            vcs = json.loads((base / ".cargo_vcs_info.json").read_text())
            if vcs["git"]["sha1"] != extra["revision"]:
                raise ValueError("Upstream notice revision differs from crate archive")
            for notice in extra["notice_files"]:
                data = (OUT / notice["path"]).read_bytes()
                if hashlib.sha256(data).hexdigest() != notice["sha256"] or len(data) != notice["bytes"]:
                    raise ValueError("Upstream notice supplement differs from receipt")
                sections.append(f"\n===== {package['name']} {package['version']} / pinned upstream {notice['path']} =====\n{data.decode('utf-8').rstrip()}\n")
                files.append(notice)
        if not files:
            missing.append(f"{package['name']} {package['version']}")
        rows.append({"name": package["name"], "version": package["version"], "license_expression": package.get("license"), "source_archive": f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download", "repository": package.get("repository"), "notice_files": files})
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--locked", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"], cwd=ROOT))
    windows_ids = {r["id"] for r in metadata["resolve"]["nodes"]}
    windows_names = {(p["name"], p["version"]) for p in metadata["packages"] if p["id"] in windows_ids}
    windows_missing = [r["name"] + " " + r["version"] for r in rows if (r["name"], r["version"]) in windows_names and not r["notice_files"]]
    if windows_missing:
        raise ValueError("Windows dependency notice files missing: " + ", ".join(windows_missing))
    # Git may use CRLF on Windows; bind the same UTF-8 lock text on every host.
    # Package source archives remain bound to their exact Cargo checksums.
    lock_text = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    inventory = {"schema_version": 1, "scope": "locked all-target Cargo metadata superset; not compiled-binary attribution or engine/browser notices", "cargo_lock_hash_format": "utf8_lf", "cargo_lock_sha256": hashlib.sha256(lock_text.encode("utf-8")).hexdigest(), "packages": rows, "windows_target_packages_without_notice_files": windows_missing, "packages_without_notice_files": missing}
    header = "Local Store locked Rust dependency notices\n\nCollected verbatim from exact-version Cargo source distributions.\nThis all-target superset includes development/platform dependencies.\nSee inventory.json for exact source archive URLs and original file hashes.\nNo engine, app container or browser-runtime redistribution approval is implied.\n"
    return (json.dumps(inventory, indent=2, ensure_ascii=False) + "\n").encode(), (header + "".join(sections)).encode(), missing


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    inventory, notices, missing = collect()
    for name, data in [("inventory.json", inventory), ("NOTICES.txt", notices)]:
        destination = OUT / name
        if args.check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise SystemExit(f"Rust notice output is stale: {destination.relative_to(ROOT)}")
        else:
            OUT.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
    print(f"Rust notices {'checked' if args.check else 'collected'}; {len(json.loads(inventory)['packages'])} exact versions; {len(missing)} without packaged notice files")
    if missing:
        print("Missing notice files: " + ", ".join(missing))


if __name__ == "__main__":
    main()
