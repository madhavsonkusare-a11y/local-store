"""Check the dependency graph against the RustSec advisory database.

`cargo audit` is the tool; this wraps it so the gate behaves the same way the
licence and MSRV gates do — a clear failure naming what changed, and an
explicit refusal rather than a silent pass when the tool or database is
missing. A scan that quietly checks nothing is worse than no scan, because it
reads like evidence.

Offline by default: the advisory database is expected to be already cloned
(`cargo audit fetch`, or a previous run). `--refresh` updates it. Unfixable
advisories may be accepted by listing them in `catalog/advisory-policy.json`
with a reason and a review date; nothing is ignored without one.
"""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "catalog" / "advisory-policy.json"


def load_policy():
    if not POLICY.exists():
        return {}
    policy = json.loads(POLICY.read_text(encoding="utf-8"))
    accepted = {}
    for entry in policy.get("accepted", []):
        for field in ("id", "reason", "reviewed_on"):
            if not entry.get(field):
                raise SystemExit(f"advisory policy entry is missing {field}: {entry}")
        accepted[entry["id"]] = entry
    return accepted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--refresh", action="store_true")
    args = parser.parse_args()

    if shutil.which("cargo-audit") is None:
        raise SystemExit(
            "cargo-audit is not installed, so nothing was scanned.\n"
            "Install it with: cargo install cargo-audit --locked"
        )
    accepted = load_policy()

    command = ["cargo", "audit", "--json"]
    if not args.refresh:
        command.append("--no-fetch")
    result = subprocess.run(command, cwd=ROOT, capture_output=True)
    if not result.stdout.strip():
        detail = result.stderr.decode("utf-8", errors="replace")
        raise SystemExit(f"cargo audit produced no report:\n{detail}")
    report = json.loads(result.stdout)

    findings = []
    for warning_kind, entries in (report.get("warnings") or {}).items():
        for entry in entries:
            advisory = (entry.get("advisory") or {}).get("id") or warning_kind
            package = (entry.get("package") or {}).get("name", "?")
            findings.append((advisory, package, warning_kind))
    for entry in (report.get("vulnerabilities") or {}).get("list", []):
        advisory = entry["advisory"]["id"]
        findings.append((advisory, entry["package"]["name"], "vulnerability"))

    unresolved = [f for f in findings if f[0] not in accepted]
    stale = [key for key in accepted if not any(f[0] == key for f in findings)]

    for advisory, package, kind in sorted(findings):
        state = "accepted" if advisory in accepted else "UNRESOLVED"
        print(f"{state:11} {kind:14} {advisory} ({package})")
    if stale:
        raise SystemExit(
            "advisory policy accepts advisories that no longer apply; remove them: "
            + ", ".join(sorted(stale))
        )
    if unresolved:
        raise SystemExit(
            f"{len(unresolved)} advisory finding(s) with no recorded decision. "
            "Fix them, or record why they are accepted in catalog/advisory-policy.json."
        )
    print(f"no unresolved advisories across {report['database']['advisory-count']} known")
    return 0


if __name__ == "__main__":
    sys.exit(main())
