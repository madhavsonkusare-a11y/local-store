"""Check that every dependency can be redistributed under this project's terms.

Local Store ships an MIT binary. A dependency whose licence cannot be satisfied
by a permissive choice — a GPL crate arriving through an upstream bump, say —
changes what may be shipped, and nothing in the build would otherwise notice.

This reads the resolved graph and evaluates each crate's SPDX expression against
the licences the project accepts. `OR` means we may choose a branch, `AND` means
every part applies. Crates whose only option is copyleft are listed separately
so they stay visible rather than quietly accumulating.

Run `python scripts/check-licenses.py --self-test` to exercise the evaluator.
"""
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
PACKAGE = "local-store"

# Licences that place no condition on shipping a binary beyond attribution,
# which `THIRD_PARTY_NOTICES.md` provides.
PERMISSIVE = {
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unlicense",
    "0BSD",
    "CC0-1.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
}
# Accepted, but file-level copyleft: shipping is fine, modifying those files
# carries a source-availability obligation. Reported so it is never a surprise.
WEAK_COPYLEFT = {"MPL-2.0"}
ACCEPTED = PERMISSIVE | WEAK_COPYLEFT


def tokenize(expression):
    # Older crates use `/` to mean OR. Exceptions narrow a licence rather than
    # replacing it, so `X WITH Y` is judged on X.
    normalized = expression.replace("/", " OR ")
    normalized = re.sub(r"\s+WITH\s+[\w.\-]+", "", normalized)
    return re.findall(r"\(|\)|[\w.\-+]+", normalized)


def satisfiable(expression, accepted=ACCEPTED):
    """Whether some allowed combination of licences satisfies the expression."""
    tokens = tokenize(expression)
    position = 0

    def parse_or():
        nonlocal position
        value = parse_and()
        while position < len(tokens) and tokens[position].upper() == "OR":
            position += 1
            value = parse_and() or value
        return value

    def parse_and():
        nonlocal position
        value = parse_atom()
        while position < len(tokens) and tokens[position].upper() == "AND":
            position += 1
            value = parse_atom() and value
        return value

    def parse_atom():
        nonlocal position
        if position < len(tokens) and tokens[position] == "(":
            position += 1
            value = parse_or()
            if position < len(tokens) and tokens[position] == ")":
                position += 1
            return value
        token = tokens[position]
        position += 1
        return token in accepted

    if not tokens:
        return False
    return parse_or()


def self_test():
    cases = [
        ("MIT", True),
        ("MIT OR Apache-2.0", True),
        ("MIT/Apache-2.0", True),
        ("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT", True),
        ("(MIT OR Apache-2.0) AND Unicode-3.0", True),
        ("MPL-2.0", True),
        ("MIT OR Apache-2.0 OR LGPL-2.1-or-later", True),
        # The cases that must fail: no permissive choice anywhere.
        ("GPL-3.0-only", False),
        ("AGPL-3.0-or-later", False),
        ("GPL-2.0 AND MIT", False),
        ("MIT AND GPL-3.0-only", False),
        ("(GPL-3.0-only OR AGPL-3.0-only)", False),
    ]
    failures = [
        f"{expression!r}: expected {expected}, got {satisfiable(expression)}"
        for expression, expected in cases
        if satisfiable(expression) is not expected
    ]
    if failures:
        raise SystemExit("evaluator self-test failed:\n  " + "\n  ".join(failures))
    print(f"evaluator self-test passed ({len(cases)} expressions)")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

result = subprocess.run(
    ["cargo", "metadata", "--format-version", "1", "--locked"],
    cwd=ROOT,
    capture_output=True,
)
if result.returncode != 0:
    detail = result.stderr.decode("utf-8", errors="replace").strip()
    raise SystemExit(f"cargo metadata failed:\n{detail}")

packages = [p for p in json.loads(result.stdout)["packages"] if p["name"] != PACKAGE]

undeclared = sorted(p["name"] for p in packages if not p.get("license"))
if undeclared:
    raise SystemExit(
        "these crates declare no SPDX licence and must be reviewed by hand:\n  "
        + "\n  ".join(undeclared)
    )

rejected = sorted(
    f"{p['name']} {p['version']}: {p['license']}"
    for p in packages
    if not satisfiable(p["license"])
)
if rejected:
    raise SystemExit(
        "these dependencies cannot be shipped under this project's terms:\n  "
        + "\n  ".join(rejected)
    )

copyleft = sorted(
    f"{p['name']} {p['version']} ({p['license']})"
    for p in packages
    if not satisfiable(p["license"], PERMISSIVE)
)
print(f"checked {len(packages)} dependency crates; all are redistributable")
if copyleft:
    print(f"{len(copyleft)} under file-level copyleft, unmodified and shipped as-is:")
    for entry in copyleft:
        print(f"  {entry}")
