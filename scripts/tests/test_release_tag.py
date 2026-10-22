"""Exercise real GPG tag signatures in a disposable, never-pushed repository."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("gate", ROOT / "scripts/verify_release_tag.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ReleaseTagTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        (ROOT / "target").mkdir(exist_ok=True)
        cls.temp = tempfile.TemporaryDirectory(prefix="release-gate-", dir=ROOT / "target")
        cls.root = Path(cls.temp.name).resolve()
        assert cls.root.parent == (ROOT / "target").resolve()
        cls.home = cls.root / "keys"
        cls.home.mkdir(mode=0o700)
        cls.repo = cls.root / "repo"
        cls.repo.mkdir()
        cls.original_cwd = Path.cwd()
        cls.original_home = os.environ.get("GNUPGHOME")
        os.environ["GNUPGHOME"] = str(cls.home)
        os.chdir(cls.repo)
        for name in ("owner", "other"):
            cls.run_command("gpg", "--batch", "--pinentry-mode", "loopback", "--passphrase", "",
                            "--quick-generate-key", f"OpenDS {name} fixture <{name}@example.test>", "ed25519", "sign", "1d")
        key_list = cls.run_command("gpg", "--batch", "--with-colons", "--list-keys")
        cls.owner, cls.other = [line.split(":")[9] for line in key_list.splitlines() if line.startswith("fpr:")]
        cls.key_file = cls.root / "public.gpg"
        cls.run_command("gpg", "--batch", "--output", str(cls.key_file), "--export")
        cls.run_command("git", "init", "--quiet")
        cls.run_command("git", "config", "user.name", "OpenDS fixture")
        cls.run_command("git", "config", "user.email", "fixture@example.test")
        cls.run_command("git", "config", "gpg.format", "openpgp")
        cls.run_command("git", "config", "user.signingkey", cls.owner)
        cls.manifest = cls.repo / "Cargo.toml"
        cls.manifest.write_text('[package]\nname="fixture"\nversion="0.1.0"\n')
        cls.run_command("git", "add", "Cargo.toml")
        cls.run_command("git", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Local fixture only")
        cls.commit = cls.run_command("git", "rev-parse", "HEAD")
        cls.run_command("git", "update-ref", "refs/remotes/origin/main", cls.commit)
        cls.run_command("git", "tag", "-s", "v0.1.0", "-m", "Fixture release")
        cls.tag_object = cls.run_command("git", "rev-parse", "refs/tags/v0.1.0")

    @staticmethod
    def run_command(*args, input=None):
        result = subprocess.run(args, input=input, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if result.returncode:
            raise AssertionError(result.stderr)
        return result.stdout.strip()

    @classmethod
    def tearDownClass(cls):
        cls.run_command("gpgconf", "--homedir", str(cls.home), "--kill", "gpg-agent")
        if cls.original_home is None:
            os.environ.pop("GNUPGHOME", None)
        else:
            os.environ["GNUPGHOME"] = cls.original_home
        os.chdir(cls.original_cwd)
        cls.temp.cleanup()

    def check(self, tag="v0.1.0", commit=None, owner=None, event_object=None):
        return gate.verify(tag, commit or self.commit, self.key_file, owner or self.owner, event_object)

    def test_authorized_signed_tag(self):
        result = self.check(event_object=self.tag_object)
        self.assertEqual(result["commit"], self.commit)
        self.assertFalse(result["prerelease"])
        self.assertEqual(self.check(event_object=self.commit)["tag_object"], self.tag_object)

    def test_reject_lightweight_and_unsigned_annotated_tags(self):
        self.run_command("git", "-c", "tag.gpgSign=false", "tag", "v1.0.0")
        self.run_command("git", "tag", "--no-sign", "-a", "v0.1.0+unsigned", "-m", "Unsigned")
        with self.assertRaisesRegex(ValueError, "lightweight"):
            self.check("v1.0.0")
        # Use the right embedded name/version so rejection reaches GPG itself.
        self.run_command("git", "update-ref", "-d", "refs/tags/v0.1.0")
        self.run_command("git", "tag", "--no-sign", "-a", "v0.1.0", self.commit, "-m", "Unsigned")
        try:
            with self.assertRaisesRegex(ValueError, "signature verification"):
                self.check()
        finally:
            self.run_command("git", "update-ref", "refs/tags/v0.1.0", self.tag_object)

    def test_reject_valid_signature_by_other_key(self):
        with self.assertRaisesRegex(ValueError, "unauthorized"):
            self.check(owner=self.other)

    def test_reject_event_and_object_mismatches(self):
        with self.assertRaisesRegex(ValueError, "triggering"):
            self.check(commit="0" * 40)
        with self.assertRaisesRegex(ValueError, "moved"):
            self.check(event_object="0" * 40)

    def test_reject_relabelled_tag_and_version_mismatch(self):
        self.run_command("git", "update-ref", "refs/tags/v0.2.0", self.tag_object)
        with self.assertRaisesRegex(ValueError, "name"):
            self.check("v0.2.0")
        self.run_command("git", "tag", "-s", "v0.3.0", self.commit, "-m", "Wrong package version")
        with self.assertRaisesRegex(ValueError, "Cargo.toml"):
            self.check("v0.3.0")

    def test_reject_commit_outside_main(self):
        (self.repo / 'outside.txt').write_text('Unmerged fixture commit')
        self.run_command("git", "add", "outside.txt")
        self.run_command("git", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Outside main fixture")
        outside = self.run_command("git", "rev-parse", "HEAD")
        self.run_command("git", "tag", "-s", "-f", "v0.1.0", outside, "-m", "Unmerged release")
        try:
            with self.assertRaises(ValueError):
                self.check(commit=outside)
        finally:
            self.run_command("git", "update-ref", "refs/tags/v0.1.0", self.tag_object)

    def test_reject_malformed_versions(self):
        for tag in ("0.1.0", "v01.0.0", "v0.1.0-01", "v0.1.0\ninjected=x"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                self.check(tag)

    def test_valid_prerelease(self):
        self.manifest.write_text('[package]\nname="fixture"\nversion="0.1.0-rc.1"\n')
        self.run_command("git", "add", "Cargo.toml")
        self.run_command("git", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Prerelease fixture")
        commit = self.run_command("git", "rev-parse", "HEAD")
        self.run_command("git", "update-ref", "refs/remotes/origin/main", commit)
        self.run_command("git", "tag", "-s", "v0.1.0-rc.1", "-m", "Prerelease fixture")
        try:
            self.assertTrue(self.check("v0.1.0-rc.1", commit=commit)["prerelease"])
        finally:
            self.run_command("git", "update-ref", "refs/remotes/origin/main", self.commit)


if __name__ == "__main__":
    unittest.main(verbosity=2)
