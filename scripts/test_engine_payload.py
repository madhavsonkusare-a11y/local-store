"""Offline refusal tests for the development engine payload builder."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("engine_payload", Path(__file__).with_name("build-engine-payload.py"))
payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(payload)


class PayloadTests(unittest.TestCase):
    def setUp(self):
        self.lock = json.loads((payload.ENGINE / "components.lock.json").read_text(encoding="utf-8"))

    def test_pins_and_origin_are_required(self):
        payload.validate(self.lock)
        for key, value in [("sha256", "bad"), ("url", "https://example.org/app.deb"),
                           ("architecture", "arm64"), ("size_bytes", 0)]:
            with self.subTest(key=key):
                changed = copy.deepcopy(self.lock)
                changed["packages"][0][key] = value
                with self.assertRaises(ValueError):
                    payload.validate(changed)
        self.lock["packages"][0] = self.lock["packages"][1]
        with self.assertRaises(ValueError):
            payload.validate(self.lock)

    def test_corrupt_or_oversized_download_never_becomes_a_package(self):
        expected = b"reviewed"
        package = {"url": self.lock["packages"][0]["url"], "size_bytes": len(expected),
                   "sha256": hashlib.sha256(expected).hexdigest()}
        for body in [b"tampered", b"too long to be accepted", b"short"]:
            with self.subTest(body=body), tempfile.TemporaryDirectory() as directory:
                stream = io.BytesIO(body)
                stream.url = package["url"]
                target = Path(directory) / "package.deb"
                with patch.object(payload.urllib.request, "urlopen", return_value=stream):
                    with self.assertRaises(ValueError):
                        payload.fetch_package(package, target)
                self.assertFalse(target.exists())
                self.assertFalse(target.with_suffix(".partial").exists())

    def test_a_verified_cached_package_requires_no_network(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "package.deb"
            target.write_bytes(b"reviewed")
            package = {"size_bytes": 8, "sha256": hashlib.sha256(b"reviewed").hexdigest()}
            with patch.object(payload.urllib.request, "urlopen", side_effect=AssertionError("network")):
                payload.fetch_package(package, target)

    def test_full_inventory_lock_refuses_drift_and_malformed_rows(self):
        locked = payload.PACKAGE_LOCK.read_bytes()
        self.assertGreaterEqual(len(payload.parse_inventory(locked)), 100)
        for changed in [
            locked.replace(b"docker-ce\t5:29.8.0", b"docker-ce\t5:29.7.0"),
            locked.replace(b"\n", b"\r\n"),
            locked + locked.splitlines(keepends=True)[0],
        ]:
            with self.subTest(changed=changed[:40]), tempfile.TemporaryDirectory() as directory:
                replacement = Path(directory) / "packages.lock.tsv"
                replacement.write_bytes(changed)
                with patch.object(payload, "PACKAGE_LOCK", replacement):
                    with self.assertRaises(ValueError):
                        payload.validate(self.lock)


if __name__ == "__main__":
    unittest.main()
