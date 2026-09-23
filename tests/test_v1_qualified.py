"""Release-count refusals: a stale proof must never quietly count toward 50."""

import copy
from datetime import datetime, timezone
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("v1_qualified", ROOT / "scripts/check-v1-qualified.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class V1QualifiedTests(unittest.TestCase):
    def setUp(self):
        self.now = int(datetime.now(timezone.utc).timestamp())
        self.entry = gate.read_json(ROOT, "catalog/v1-qualified-apps.json")["apps"][0]
        roster = gate.read_json(ROOT, "catalog/v1-roster.json")
        self.roster = {app["id"]: app for app in roster["apps"] if app.get("cohort") == "existing_offering"}

    def test_current_memos_proof_counts_once(self):
        result = gate.validate(ROOT, self.now)
        self.assertEqual(result["qualified"], 1)
        self.assertEqual(result["remaining"], 49)

    def test_changed_manifest_or_evidence_is_refused(self):
        for field in ("manifest_sha256", "evidence_sha256"):
            with self.subTest(field=field):
                changed = copy.deepcopy(self.entry)
                changed[field] = "0" * 64
                with self.assertRaisesRegex(ValueError, "hash changed"):
                    gate.validate_entry(ROOT, changed, self.roster, self.now)

    def test_stale_or_escaped_proof_is_refused(self):
        with self.assertRaisesRegex(ValueError, "stale"):
            gate.validate_entry(ROOT, self.entry, self.roster, self.now + 31 * 86400)
        changed = copy.deepcopy(self.entry)
        changed["evidence"] = "docs/evidence/../../catalog/v1-roster.json"
        with self.assertRaisesRegex(ValueError, "escaped path"):
            gate.validate_entry(ROOT, changed, self.roster, self.now)


if __name__ == "__main__":
    unittest.main()
