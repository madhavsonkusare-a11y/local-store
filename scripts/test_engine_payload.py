"""Offline refusal tests for the development engine payload builder."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("engine_payload", Path(__file__).with_name("build-engine-payload.py"))
payload = importlib.util.module_from_spec(spec)
spec.loader.exec_module(payload)


class PayloadTests(unittest.TestCase):
    def setUp(self):
        self.lock = json.loads((payload.ENGINE / "components.lock.json").read_text(encoding="utf-8"))

    def test_managed_builder_targets_only_its_fixed_socket_and_quotes_paths_as_arguments(self):
        with patch.object(payload, "MANAGED_ENGINE", True):
            command = payload.docker_command("build", "D:/06 Projects/dockwrap/context", "--iidfile", r"D:\06 Projects\dockwrap\image-id")
        self.assertEqual(command[:9], ["wsl.exe", "--distribution", "local-store-engine-v1", "--user", "root", "--exec", "/usr/bin/docker", "--host", "unix:///var/run/docker.sock"])
        self.assertEqual(command[10], "/mnt/d/06 Projects/dockwrap/context")
        self.assertEqual(command[12], "/mnt/d/06 Projects/dockwrap/image-id")
        self.assertNotIn("sh", command)

    def test_pins_and_origin_are_required(self):
        payload.validate(self.lock)
        for value in ["", "latest", "2026-09-13", "20269913T120000Z",
                      "20990101T120000Z", "20260913T120000Z; rm -rf /"]:
            with self.subTest(snapshot=value):
                changed = copy.deepcopy(self.lock)
                changed["ubuntu_snapshot"] = value
                with self.assertRaises(ValueError):
                    payload.validate(changed)
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

    def test_inventory_drift_reports_exact_changed_added_and_removed_packages(self):
        locked = b"alpha\t1\tamd64\ncharlie\t3\tall\n"
        observed = b"alpha\t2\tamd64\nbravo\t1\tamd64\n"
        self.assertEqual(payload.inventory_drift(locked, observed), [
            {"name": "alpha", "locked": ("1", "amd64"), "observed": ("2", "amd64")},
            {"name": "bravo", "locked": None, "observed": ("1", "amd64")},
            {"name": "charlie", "locked": ("3", "all"), "observed": None},
        ])

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

    def test_apt_provenance_tar_hashes_regular_files_without_extracting(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "archives.tar"
            with tarfile.open(target, "w") as archive:
                blob = b"package bytes"
                entry = tarfile.TarInfo("archives/example.deb")
                entry.size = len(blob)
                archive.addfile(entry, io.BytesIO(blob))
            self.assertEqual(payload.manifest_tar(target, suffix=".deb"), {
                "archives/example.deb": {"size_bytes": len(blob),
                                          "sha256": hashlib.sha256(blob).hexdigest()}
            })

    def test_apt_provenance_tar_refuses_unsafe_paths_and_links(self):
        for name, kind in [("../escape.deb", tarfile.REGTYPE),
                           ("/absolute.deb", tarfile.REGTYPE),
                           ("archives/link.deb", tarfile.SYMTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                target = Path(directory) / "archives.tar"
                with tarfile.open(target, "w") as archive:
                    entry = tarfile.TarInfo(name)
                    entry.type = kind
                    archive.addfile(entry)
                with self.assertRaises(ValueError):
                    payload.manifest_tar(target, suffix=".deb")


if __name__ == "__main__":
    unittest.main()
