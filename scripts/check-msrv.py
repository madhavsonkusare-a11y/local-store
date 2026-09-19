"""Check that the declared minimum Rust version is not lower than the tree needs.

`rust-version` in Cargo.toml is a promise to anyone building this project. Cargo
does not verify it against dependencies at build time, so on a newer toolchain a
stale value stays wrong and silently passes every test. This reads the resolved
dependency graph and fails if any crate in it requires more than we claim.

It is a floor, not a proof: our own code could need more than any dependency
does. Only building with the declared toolchain shows that, which CI does.
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = "local-store"


def version(text):
    """`1.88` and `1.88.0` are the same requirement; compare them as numbers."""
    parts = [int(part) for part in text.split("-")[0].split(".")]
    return tuple(parts + [0] * (3 - len(parts)))


def metadata():
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise SystemExit(f"cargo metadata failed:\n{detail}")
    return json.loads(result.stdout)


packages = metadata()["packages"]

declared = next(
    (p.get("rust_version") for p in packages if p["name"] == PACKAGE),
    None,
)
if not declared:
    raise SystemExit(f"{PACKAGE} declares no rust-version")

required = [
    (p["name"], p["version"], p["rust_version"])
    for p in packages
    if p.get("rust_version") and p["name"] != PACKAGE
]
if not required:
    raise SystemExit("no dependency declares a rust-version; the graph looks wrong")

highest = max(required, key=lambda entry: version(entry[2]))
if version(declared) < version(highest[2]):
    demanding = sorted(
        f"{name} {ver} needs {rust}"
        for name, ver, rust in required
        if version(rust) == version(highest[2])
    )
    raise SystemExit(
        f"{PACKAGE} declares rust-version {declared}, but its dependencies need "
        f"{highest[2]}:\n  " + "\n  ".join(demanding)
    )

print(
    f"MSRV {declared} covers every dependency"
    f" (highest requirement {highest[2]}, from {highest[0]})"
)
sys.exit(0)
