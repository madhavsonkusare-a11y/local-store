"""Verify notice resources and exact bytes without building or installing Windows artifacts."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = ["LICENSE", "THIRD_PARTY_NOTICES.md", "src/assets/apps/LICENSE", "src/assets/LUCIDE-LICENSE", "src/fonts/OFL.txt", "src/fonts/InstrumentSans-OFL.txt", "src/fonts/IBMPlex-OFL.txt", "catalog/notices/rust/inventory.json", "catalog/notices/rust/NOTICES.txt", "catalog/notices/rust/upstream-supplements.json", "catalog/notices/rust/mpl-source.json"]


def verify(resource_dir=None):
    patterns = json.loads((ROOT / "tauri.conf.json").read_text())["bundle"]["resources"]
    resources = {p.relative_to(ROOT).as_posix() for pattern in patterns for p in ROOT.glob(pattern) if p.is_file()}
    errors = [f"Required notice not configured: {name}" for name in REQUIRED if name not in resources]
    inventory = json.loads((ROOT / "catalog/notices/rust/inventory.json").read_text())
    lock_text = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    if inventory.get("cargo_lock_hash_format") != "utf8_lf" or inventory["cargo_lock_sha256"] != sha256(lock_text.encode("utf-8")).hexdigest():
        errors.append("Rust notice inventory differs from current Cargo.lock")
    if inventory["windows_target_packages_without_notice_files"]:
        errors.append("Windows dependency notice files are missing")
    source_record = json.loads((ROOT / "catalog/notices/rust/mpl-source.json").read_text())
    checks = {(r["name"], r["version"]): r.get("checksum") for r in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]}
    for row in source_record["packages"]:
        name = "catalog/notices/rust/" + row["path"]
        digest = sha256((ROOT / name).read_bytes()).hexdigest()
        if digest != row["sha256"] or digest != checks[(row["name"], row["version"])]:
            errors.append(f"MPL source differs from exact lock checksum: {name}")
        if name not in resources:
            errors.append(f"MPL source not configured: {name}")
    notice_files = sorted(name for name in resources if name in REQUIRED or name.startswith("catalog/notices/"))
    if resource_dir:
        for name in notice_files:
            staged = resource_dir / name
            if not staged.is_file():
                errors.append(f"Staged notice missing: {name}")
            elif staged.read_bytes() != (ROOT / name).read_bytes():
                errors.append(f"Staged notice differs: {name}")
    return {"schema_version": 1, "scope": "notice configuration and optional staged resource bytes; no installer extraction, launch, signing or clean-host proof", "configured_notice_count": len(notice_files), "files": [{"path": name, "sha256": sha256((ROOT / name).read_bytes()).hexdigest()} for name in notice_files], "errors": errors, "passed": not errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resource-dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = verify(args.resource_dir.resolve() if args.resource_dir else None)
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Notice resources: {'PASS' if result['passed'] else 'FAIL'}; {result['configured_notice_count']} files")
    for error in result['errors']:
        print(error)
    raise SystemExit(0 if result['passed'] else 1)


if __name__ == "__main__":
    main()
