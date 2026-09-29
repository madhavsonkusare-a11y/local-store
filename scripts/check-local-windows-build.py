"""Check the actual Tauri output did not substitute a sidecar for the launcher."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import subprocess


def verify(build: Path, version: str) -> None:
    launcher = build / "local-store.exe"
    sidecar = build / "local-store-mcp.exe"
    if not launcher.is_file() or not sidecar.is_file():
        raise ValueError("Windows build is missing the launcher or MCP sidecar")
    if sha256(launcher.read_bytes()).digest() == sha256(sidecar.read_bytes()).digest():
        raise ValueError("Tauri bundled the MCP sidecar as the launcher")
    result = subprocess.run(
        [str(launcher), "--version"], capture_output=True, timeout=20, check=False
    )
    if result.returncode != 0 or version.encode() not in result.stdout:
        raise ValueError(
            "the bundled launcher did not return its product version; "
            "Tauri may have selected another Cargo binary"
        )
    bundle = build / "bundle" / "nsis"
    if not any(bundle.glob("*-setup.exe")):
        raise ValueError("Windows build has no NSIS installer")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-dir", type=Path, required=True)
    args = parser.parse_args()
    version = json.loads(
        (Path(__file__).resolve().parents[1] / "tauri.conf.json").read_text(encoding="utf-8")
    )["version"]
    verify(args.build_dir.resolve(), version)
    print(f"verified launcher identity, sidecar separation, and NSIS installer for {version}")


if __name__ == "__main__":
    main()
