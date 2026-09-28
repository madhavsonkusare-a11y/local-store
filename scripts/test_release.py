"""Release staging must reject incomplete or ambiguous payloads."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("release", Path(__file__).with_name("prepare-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.build, self.output = self.root / "release", self.root / "staged"
        (self.build / "bundle/nsis").mkdir(parents=True)
        (self.build / "local-store.exe").write_bytes(b"test binary")
        (self.build / "local-store-mcp.exe").write_bytes(b"test MCP binary")
        (self.build / "bundle/nsis/Local Store_x64-setup.exe").write_bytes(b"test installer")
        (self.build / "local-store.pdb").write_bytes(b"debug symbols")

    def prepare(self):
        return release.prepare(self.build, self.output, "x86_64-pc-windows-msvc", "1.0.0", "a" * 40)

    def test_payloads_have_verifiable_hashes_and_source_metadata(self):
        manifest = self.prepare()
        self.assertEqual(manifest["source_commit"], "a" * 40)
        self.assertFalse(manifest["attested"])
        self.assertEqual(len(manifest["files"]), 3)
        self.assertEqual(
            {item["name"] for item in manifest["files"]},
            {"local-store-x86_64-pc-windows-msvc.exe",
             "local-store-mcp-x86_64-pc-windows-msvc.exe",
             "Local Store_x64-setup.exe"},
        )
        for item in manifest["files"]:
            self.assertEqual(item["sha256"], hashlib.sha256((self.output / item["name"]).read_bytes()).hexdigest())
        self.assertFalse((self.output / "local-store.pdb").exists())
        checksum_file = self.output / "SHA256SUMS-x86_64-pc-windows-msvc.txt"
        for line in checksum_file.read_text().splitlines():
            digest, name = line.split("  ", 1)
            self.assertEqual(digest, release.sha256(self.output / name))
        saved = json.loads((self.output / "build-x86_64-pc-windows-msvc.json").read_text())
        self.assertEqual(saved, manifest)

    def test_missing_installer_cannot_publish_only_a_binary(self):
        (self.build / "bundle/nsis/Local Store_x64-setup.exe").unlink()
        with self.assertRaisesRegex(ValueError, "at least one"):
            self.prepare()
        self.assertFalse(self.output.exists())

    def test_missing_mcp_binary_cannot_publish_windows_release(self):
        (self.build / "local-store-mcp.exe").unlink()
        with self.assertRaisesRegex(ValueError, "local-store-mcp.exe"):
            self.prepare()
        self.assertFalse(self.output.exists())

    def test_duplicate_names_are_rejected_before_copying(self):
        (self.build / "bundle/other").mkdir()
        (self.build / "bundle/other/Local Store_x64-setup.exe").write_bytes(b"different installer")
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.prepare()
        self.assertFalse(self.output.exists())

    def test_old_outputs_are_not_mixed_into_a_new_release(self):
        self.output.mkdir()
        stale = self.output / "old.exe"
        stale.write_bytes(b"keep")
        with self.assertRaisesRegex(ValueError, "must be empty"):
            self.prepare()
        self.assertEqual(stale.read_bytes(), b"keep")

    def test_source_identity_is_required(self):
        with self.assertRaisesRegex(ValueError, "commit SHA"):
            release.prepare(self.build, self.output, "x86_64-pc-windows-msvc", "1.0.0", "main")

    def test_macos_and_linux_stage_only_their_own_installer_formats(self):
        (self.build / "local-store").write_bytes(b"unix binary")
        (self.build / "bundle/Local Store_arm64.dmg").write_bytes(b"mac installer")
        (self.build / "bundle/local-store_amd64.deb").write_bytes(b"linux installer")
        for target, extension in [("aarch64-apple-darwin", ".dmg"), ("x86_64-unknown-linux-gnu", ".deb")]:
            with self.subTest(target=target):
                output = self.root / target
                manifest = release.prepare(self.build, output, target, "1.0.0", "a" * 40)
                installers = [item for item in manifest["files"] if not item["name"].startswith("local-store-" + target)]
                self.assertEqual(len(installers), 1)
                self.assertTrue(installers[0]["name"].endswith(extension))


if __name__ == "__main__":
    unittest.main()
