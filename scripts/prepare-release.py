"""Stage explicit release payloads with checksums and unsigned build metadata."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil

TARGETS = {"x86_64-pc-windows-msvc", "aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def prepare(build, output, target, version, source_sha, run_url=None):
    if target not in TARGETS or not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("expected a supported Rust target and full source commit SHA")
    if not version or any(character in version for character in "/\\\n\r"):
        raise ValueError("invalid release version")
    build, output = Path(build).resolve(), Path(output).resolve()
    if output.exists() and any(output.iterdir()):
        raise ValueError("release staging directory must be empty")
    windows = target.endswith("windows-msvc")
    binary = build / ("local-store.exe" if windows else "local-store")
    extensions = {".msi", ".exe"} if windows else ({".dmg"} if "apple" in target else {".deb", ".rpm", ".AppImage"})
    installers = sorted(path for path in (build / "bundle").rglob("*")
                        if path.is_file() and path.suffix in extensions
                        and (path.suffix != ".exe" or path.name.endswith("setup.exe")))
    if not binary.is_file() or not installers:
        raise ValueError("release requires the compiled CLI and at least one platform installer")
    payloads = [(binary, f"local-store-{target}{'.exe' if windows else ''}")]
    if windows:
        mcp_binary = build / "local-store-mcp.exe"
        if not mcp_binary.is_file():
            raise ValueError("Windows release requires the compiled local-store-mcp.exe")
        payloads.append((mcp_binary, f"local-store-mcp-{target}.exe"))
    payloads += [(path, path.name) for path in installers]
    names = set()
    for path, name in payloads:
        if not path.resolve().is_relative_to(build):
            raise ValueError("release input resolves outside its build directory")
        if name.casefold() in names or any(character in name for character in "\n\r"):
            raise ValueError("duplicate or invalid release filename")
        names.add(name.casefold())
    output.mkdir(parents=True, exist_ok=True)
    for path, name in payloads:
        shutil.copy2(path, output / name)
    files = [{"name": name, "sha256": sha256(output / name), "size_bytes": (output / name).stat().st_size}
             for _, name in payloads]
    manifest_name = f"build-{target}.json"
    manifest = {"schema_version": 1, "version": version, "target": target,
                "source_commit": source_sha, "workflow_run_url": run_url,
                "attested": False, "files": files}
    (output / manifest_name).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    names = sorted([name for _, name in payloads] + [manifest_name])
    (output / f"SHA256SUMS-{target}.txt").write_text(
        "".join(f"{sha256(output / name)}  {name}\n" for name in names), encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=sorted(TARGETS))
    parser.add_argument("--build-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    version = json.loads((root / "tauri.conf.json").read_text())["version"]
    ref = os.environ.get("GITHUB_REF", "")
    if ref.startswith("refs/tags/") and ref != f"refs/tags/v{version}":
        raise SystemExit("release tag must match the configured product version")
    run_url = None
    if os.environ.get("GITHUB_RUN_ID"):
        run_url = f"{os.environ['GITHUB_SERVER_URL']}/{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
    try:
        manifest = prepare(args.build_dir, args.output_dir, args.target, version,
                           os.environ.get("GITHUB_SHA", ""), run_url)
    except ValueError as error:
        raise SystemExit(str(error)) from error
    print(f"staged {len(manifest['files'])} payloads for {args.target} with SHA-256 checksums")


if __name__ == "__main__":
    main()
