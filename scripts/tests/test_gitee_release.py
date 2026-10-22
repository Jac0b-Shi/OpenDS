"""Exercise integrity checks, API resume behavior and real owner-tag signatures."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("mirror", ROOT / "scripts/mirror_gitee_release.py")
mirror = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mirror)


def fixture():
    receipt = {"tag": "v0.1.0", "commit": "a" * 40, "tag_object": "b" * 40,
               "authorized_primary_fingerprint": mirror.FINGERPRINT}
    files = {"OpenDS-0.1.0-windows-x64.zip": b"binary fixture", "OpenDS-0.1.0-source.zip": b"matching source",
             "TAG-SIGNATURE.json": json.dumps(receipt).encode()}
    files["SHA256SUMS.txt"] = "".join(hashlib.sha256(data).hexdigest() + "  ./" + name + "\n"
                                    for name, data in files.items()).encode()
    release = {"tag_name": "v0.1.0", "name": "OpenDS 0.1.0", "body": "Release notes", "draft": False,
               "prerelease": False, "assets": [{"name": name, "size": len(data),
               "digest": "sha256:" + hashlib.sha256(data).hexdigest()} for name, data in files.items()]}
    return release, files, receipt


class IntegrityTests(unittest.TestCase):
    def test_matching_release_and_all_assets(self):
        mirror.verify_assets(*fixture())

    def test_reject_corruption_missing_source_and_wrong_receipt(self):
        for mutation in (lambda f: f.update({"OpenDS-0.1.0-windows-x64.zip": b"corrupt"}),
                         lambda f: f.pop("OpenDS-0.1.0-source.zip"),
                         lambda f: f.update({"TAG-SIGNATURE.json": b"{}"}),
                         lambda f: f.update({"SHA256SUMS.txt": b"bad\n"})):
            release, files, receipt = fixture()
            mutation(files)
            with self.assertRaises(ValueError):
                mirror.verify_assets(release, files, receipt)

    def test_reject_draft_digest_mismatch_and_duplicate_asset(self):
        for change in ("draft", "digest", "duplicate"):
            release, files, receipt = fixture()
            if change == "draft":
                release["draft"] = True
            elif change == "digest":
                release["assets"][0]["digest"] = "sha256:" + "0" * 64
            else:
                release["assets"].append(copy.deepcopy(release["assets"][0]))
            with self.assertRaises(ValueError):
                mirror.verify_assets(release, files, receipt)

    def test_no_credentials_outside_gitee_or_redirects(self):
        with self.assertRaisesRegex(ValueError, "restricted"):
            mirror.request("https://example.test", b"token")
        self.assertIsNone(mirror.NoRedirect().redirect_request(None, None, 302, "", {}, "https://example.test"))

    def test_wait_for_github_release_and_fail_on_timeout_or_auth_error(self):
        release, _, _ = fixture()
        with patch.object(mirror, "get_json", side_effect=[mirror.ApiError(404), release]), \
             patch.object(mirror.time, "sleep"):
            self.assertEqual(mirror.published_releases("v0.1.0", 1), [release])
        for status in (404, 401, 429):
            with patch.object(mirror, "get_json", side_effect=mirror.ApiError(status)), self.assertRaises(ValueError):
                mirror.published_releases("v0.1.0", 0)


class PublishTests(unittest.TestCase):
    def setUp(self):
        self.release, self.files, self.receipt = fixture()
        self.assets = []
        self.stored = {}
        self.writes = []
        self.exists = True

    def json_api(self, url):
        if "/tags/" in url:
            if not self.exists:
                raise mirror.ApiError(404)
            return {"id": 123, "tag_name": "v0.1.0", "prerelease": False}
        return copy.deepcopy(self.assets)

    def api(self, url, payload=None, content_type="application/json"):
        if payload is None:
            return self.stored[int(url.split("/")[-2])]
        self.writes.append((url, payload))
        if content_type == "application/json":
            self.assertEqual(json.loads(payload)["target_commitish"], self.receipt["commit"])
            self.exists = True
            return json.dumps(self.json_api(url + "/tags/v0.1.0")).encode()
        name = payload.split(b'filename="', 1)[1].split(b'"', 1)[0].decode()
        self.assertIn(self.files[name], payload)
        asset = {"id": len(self.assets) + 1, "name": name}
        self.assets.append(asset)
        self.stored[asset["id"]] = self.files[name]
        return json.dumps(asset).encode()

    def run_publish(self):
        with patch.object(mirror, "remote_tag", return_value=self.receipt["tag_object"]), \
             patch.object(mirror, "get_json", side_effect=self.json_api), \
             patch.object(mirror, "request", side_effect=self.api):
            mirror.publish(self.release, self.files, self.receipt, "fixture-token")

    def test_create_read_back_and_repeat_without_writes(self):
        self.exists = False
        self.run_publish()
        self.assertEqual(len(self.writes), 5)
        self.assertIn(b'filename="SHA256SUMS.txt"', self.writes[-1][1])
        self.writes.clear()
        self.run_publish()
        self.assertEqual(self.writes, [])

    def test_resume_only_missing_attachments(self):
        name = "OpenDS-0.1.0-source.zip"
        self.assets = [{"id": 1, "name": name}]
        self.stored = {1: self.files[name]}
        self.run_publish()
        self.assertEqual(len(self.writes), 3)

    def test_existing_mismatch_stops_before_any_write(self):
        self.assets = [{"id": 1, "name": "OpenDS-0.1.0-source.zip"}]
        self.stored = {1: b"different"}
        with self.assertRaisesRegex(ValueError, "differs"):
            self.run_publish()
        self.assertEqual(self.writes, [])

    def test_moved_tag_stops_before_write(self):
        with patch.object(mirror, "remote_tag", return_value="c" * 40), \
             patch.object(mirror, "request") as api, self.assertRaisesRegex(ValueError, "moved"):
            mirror.publish(self.release, self.files, self.receipt, "fixture-token")
        api.assert_not_called()


# Reuse only the disposable test repository/key setup, never the user's key.
tag_spec = importlib.util.spec_from_file_location("tag_tests", ROOT / "scripts/tests/test_release_tag.py")
tag_tests = importlib.util.module_from_spec(tag_spec)
tag_spec.loader.exec_module(tag_tests)


class SignatureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture = tag_tests.ReleaseTagTests
        cls.fixture.setUpClass()

    @classmethod
    def tearDownClass(cls):
        cls.fixture.tearDownClass()

    def verify(self, owner):
        real_git = mirror.git

        def local_git(*args, **kwargs):
            if args[0] == "fetch":
                args = (*args[:2], str(self.fixture.repo), *args[3:])
            return real_git(*args, **kwargs)

        with tempfile.TemporaryDirectory(dir=self.fixture.root) as directory, \
             patch.object(mirror, "remote_tag", return_value=self.fixture.tag_object), \
             patch.object(mirror, "git", side_effect=local_git), \
             patch.object(mirror, "request", return_value=self.fixture.key_file.read_bytes()), \
             patch.object(mirror, "FINGERPRINT", owner):
            return mirror.verify_tag("v0.1.0", Path(directory))

    def test_real_signed_tag_and_unauthorized_key(self):
        self.assertEqual(self.verify(self.fixture.owner)["commit"], self.fixture.commit)
        with self.assertRaisesRegex(ValueError, "authorized"):
            self.verify(self.fixture.other)


if __name__ == "__main__":
    unittest.main()
