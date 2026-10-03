"""Targeted offline rejection checks against actual captured signed metadata."""
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import unittest

SCRIPT = Path(__file__).with_name("verify-engine-provenance.py")
SPEC = importlib.util.spec_from_file_location("engine_provenance", SCRIPT)
review = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(review)


class ProvenanceBoundary(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.keyring = review.REVIEW / "ubuntu-keyring.gpg"
        indexes = review.members(review.DEFAULT_BUILD / "ubuntu-indexes.tar", 128 * 1024 * 1024)
        cls.signed = next(data for name, data in indexes.items() if name.endswith("_InRelease"))
        # A real signed cleartext field is modified while preserving the armor.
        if b"Origin: Ubuntu" not in cls.signed or not cls.keyring.is_file():
            raise RuntimeError("Run the independent retained-artifact review first")

    def test_actual_signed_metadata_is_accepted(self):
        _, hashes, observed = review.verify_signature(self.signed, self.keyring, "boundary-valid", review.UBUNTU_KEYS)
        self.assertTrue(hashes)
        self.assertIn(observed["primary_key"], review.UBUNTU_KEYS)

    def test_modified_cleartext_is_rejected(self):
        modified = self.signed.replace(b"Origin: Ubuntu", b"Origin: Untrust", 1)
        with self.assertRaisesRegex(ValueError, "signature failed"):
            review.verify_signature(modified, self.keyring, "boundary-modified", review.UBUNTU_KEYS)

    def test_valid_signature_with_foreign_anchor_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unreviewed trust anchor"):
            review.verify_signature(self.signed, self.keyring, "boundary-foreign", {review.DOCKER_KEY})


if __name__ == "__main__":
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(ProvenanceBoundary)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    receipt = {
        "schema_version": 1,
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "passed": result.wasSuccessful(),
        "checks": result.testsRun,
        "review_script_sha256": review.file_sha(SCRIPT),
        "test_script_sha256": review.file_sha(Path(__file__)),
        "signed_metadata_sha256": review.sha(ProvenanceBoundary.signed),
        "proof": "actual retained Ubuntu InRelease accepts its reviewed primary key, rejects modified cleartext and rejects a foreign trust anchor",
        "limits": ["Filesystem and GnuPG checks only; no WSL or engine lifecycle claim"],
    }
    target = review.ROOT / "docs/evidence/engine-provenance-boundary-2026-10-02.json"
    target.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(0 if result.wasSuccessful() else 1)
