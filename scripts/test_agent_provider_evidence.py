import importlib.util
import tempfile
import unittest
from pathlib import Path
import hashlib

spec = importlib.util.spec_from_file_location("provider_evidence", Path(__file__).with_name("check-agent-provider-evidence.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ProviderProofContract(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        (self.root / "provider.rs").write_bytes(b"reviewed provider")
        self.inputs = {"provider_sha256": "provider.rs"}
        self.receipt = {
            "schema_version": 1, "passed": True, "failure": None,
            "app": "memos", "recorded_at_unix": 1000,
            "engine": {"schema_version": 2, "program": "wsl.exe", "endpoint": "wsl://local-store-engine-v1"},
            "steps": ["private read", "approved write", "denial", "cleanup"],
            "provider_sha256": hashlib.sha256(b"reviewed provider").hexdigest(),
        }

    def validate(self):
        checker.validate("memos", self.receipt, self.inputs, 1000, self.root)

    def test_source_changes_invalidate_an_otherwise_passing_proof(self):
        self.validate()
        (self.root / "provider.rs").write_bytes(b"changed provider")
        with self.assertRaises(ValueError):
            self.validate()

    def test_failure_wrong_engine_and_future_dates_do_not_become_verified(self):
        for patch in [{"passed": False}, {"schema_version": True}, {"failure": "failed"}, {"recorded_at_unix": 2000}, {"recorded_at_unix": "1000"}, {"engine": {"schema_version": 2, "program": "docker", "endpoint": "default"}}]:
            with self.subTest(patch=patch):
                saved = dict(self.receipt)
                self.receipt.update(patch)
                with self.assertRaises(ValueError):
                    self.validate()
                self.receipt = saved

    def test_missing_file_ownership_checks_are_refused(self):
        self.receipt.update({"proof": "flatnotes-scoped-markdown-files", "owned_cleanup": True, "bystanders_preserved": True})
        with self.assertRaises(ValueError):
            checker.validate("flatnotes", self.receipt, self.inputs, 1000, self.root)
        self.receipt["native_engine_files_unchanged"] = True
        checker.validate("flatnotes", self.receipt, self.inputs, 1000, self.root)


if __name__ == "__main__":
    unittest.main()
