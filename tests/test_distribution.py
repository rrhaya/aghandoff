"""Synthetic, offline package/installer tests. No real accounts or credentials."""
import hashlib
import io
import json
from pathlib import Path
import platform
import shlex
import subprocess
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import package
import prepare_release

COMMIT = "a" * 40
SYSTEM_PATH = "/usr/bin:/bin"


def native_target():
    arch = "aarch64" if platform.machine() in ("aarch64", "arm64") else "x86_64"
    return arch + ("-apple-darwin" if sys.platform == "darwin" else "-unknown-linux-gnu")


class Distribution(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="aghandoff-dist-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.output = self.root / "output with spaces"
        self.prefix = self.root / "prefix with spaces"
        self.version = package.version(ROOT)
        self.binary = self.root / "synthetic binary"
        self.binary.write_text("#!/bin/sh\nprintf 'aghandoff " + self.version + "\\n'\n")
        self.binary.chmod(0o700)
        self.target = native_target()

    def create(self, target=None, output=None, commit=COMMIT):
        archive = package.create(ROOT, self.binary, target or self.target, commit,
                                 output or self.output)
        return archive, Path(str(archive) + ".sha256")

    def install(self, archive, checksum, *options, env_extra=None):
        env = {"PATH": SYSTEM_PATH, "TMPDIR": str(self.root)}
        env.update(env_extra or {})
        return subprocess.run(["/bin/sh", str(ROOT / "scripts/install.sh"),
                               "--archive", str(archive), "--checksum", str(checksum),
                               "--prefix", str(self.prefix), *options],
                              env=env, capture_output=True, text=True)

    def require_success(self, result):
        self.assertEqual(result.returncode, 0, result.stderr)

    def require_failure(self, result, message=None):
        self.assertNotEqual(result.returncode, 0)
        if message:
            self.assertIn(message, result.stderr)

    def rewrite(self, archive, checksum, members):
        with tarfile.open(archive, "w:gz") as tar:
            for name, data, kind in members:
                entry = tarfile.TarInfo(name)
                if kind == "link":
                    entry.type = tarfile.SYMTYPE
                    entry.linkname = str(self.root / "unrelated")
                    tar.addfile(entry)
                else:
                    entry.size = len(data)
                    entry.mode = 0o755 if name == "aghandoff" else 0o644
                    tar.addfile(entry, io.BytesIO(data))
        checksum.write_text(hashlib.sha256(archive.read_bytes()).hexdigest()
                            + "  " + archive.name + "\n")

    def members(self):
        info = ("version=" + self.version + "\ntarget=" + self.target
                + "\ncommit=" + COMMIT + "\n").encode()
        return [("BUILD-INFO", info, "file"), ("LICENSE", b"synthetic license", "file"),
                ("aghandoff", self.binary.read_bytes(), "file")]

    def test_packages_are_deterministic_bounded_and_contain_only_public_metadata(self):
        first, checksum = self.create()
        second, _ = self.create(output=self.root / "second")
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with tarfile.open(first) as tar:
            self.assertEqual(tar.getnames(), ["BUILD-INFO", "LICENSE", "aghandoff"])
            for member in tar.getmembers():
                self.assertTrue(member.isfile())
                self.assertEqual((member.uid, member.gid, member.mtime), (0, 0, 0))
                self.assertEqual((member.uname, member.gname), ("", ""))
            self.assertNotIn(str(self.root).encode(), tar.extractfile("BUILD-INFO").read())
        self.assertEqual(checksum.read_text(), hashlib.sha256(first.read_bytes()).hexdigest()
                         + "  " + first.name + "\n")

    def test_packaging_refuses_existing_destinations_wrong_versions_and_special_sources(self):
        archive, _ = self.create()
        before = archive.read_bytes()
        with self.assertRaises(ValueError):
            self.create()
        self.assertEqual(archive.read_bytes(), before)
        self.binary.write_text("#!/bin/sh\necho 'aghandoff 0.0.0'\n")
        with self.assertRaises(ValueError):
            self.create(output=self.root / "wrong-version")
        with self.assertRaises(ValueError):
            package.create(ROOT, self.binary, self.target, "bad", self.root / "bad-commit")
        linked = self.root / "linked binary"
        linked.symlink_to(self.binary)
        with self.assertRaises(ValueError):
            package.create(ROOT, linked, self.target, COMMIT, self.root / "linked")
        with self.binary.open("wb") as stream:
            stream.truncate(package.MAX_BINARY + 1)
        with self.assertRaises(ValueError):
            self.create(output=self.root / "oversized")

    def test_install_runs_with_no_rust_or_home_and_cleans_its_temporary_files(self):
        archive, checksum = self.create()
        result = self.install(archive, checksum)
        self.require_success(result)
        installed = self.prefix / "bin/aghandoff"
        self.assertEqual(installed.stat().st_mode & 0o777, 0o755)
        self.assertEqual(subprocess.check_output([str(installed), "--version"]).strip(),
                         ("aghandoff " + self.version).encode())
        self.assertEqual(list(self.root.glob("aghandoff-install.*")), [])
        self.assertEqual(list((self.prefix / "bin").glob(".aghandoff*")), [])

    def test_checksum_mismatch_and_malformed_sidecars_do_not_create_the_prefix(self):
        archive, checksum = self.create()
        valid = checksum.read_text()
        for content, reason in [("0" * 64 + "  " + archive.name + "\n", "checksum mismatch"),
                                (valid + "unexpected\n", "one asset"),
                                (valid.replace(archive.name, "another.tar.gz"), "different asset"),
                                ("not a checksum\n", "invalid checksum")]:
            checksum.write_text(content)
            self.require_failure(self.install(archive, checksum), reason)
            self.assertFalse(self.prefix.exists())

    def test_archive_traversal_duplicate_members_and_links_are_refused_before_execution(self):
        archive, checksum = self.create()
        marker = self.root / "executed-invalid-binary"
        malicious = ("#!/bin/sh\ntouch " + shlex.quote(str(marker))
                     + "\necho 'aghandoff " + self.version + "'\n").encode()
        base = [(name, malicious if name == "aghandoff" else data, kind)
                for name, data, kind in self.members()]
        cases = [base + [("../escape", b"synthetic", "file")],
                 base + [("aghandoff", malicious, "file")],
                 [(name, data, "link" if name == "aghandoff" else kind)
                  for name, data, kind in base]]
        for index, members in enumerate(cases):
            self.rewrite(archive, checksum, members)
            self.require_failure(self.install(archive, checksum),
                                 "special files" if index == 2 else "archive members")
            self.assertFalse(marker.exists())
            self.assertFalse(self.prefix.exists())
            self.assertFalse((self.root / "escape").exists())

    def test_wrong_build_target_and_binary_version_are_refused(self):
        archive, checksum = self.create()
        entries = self.members()
        wrong = [(name, data.replace(self.target.encode(), b"unsupported-target")
                  if name == "BUILD-INFO" else data, kind) for name, data, kind in entries]
        self.rewrite(archive, checksum, wrong)
        self.require_failure(self.install(archive, checksum), "target does not match")
        wrong = [(name, b"#!/bin/sh\necho 'aghandoff 0.0.0'\n" if name == "aghandoff"
                  else data, kind) for name, data, kind in entries]
        self.rewrite(archive, checksum, wrong)
        self.require_failure(self.install(archive, checksum), "version differs")
        self.assertFalse(self.prefix.exists())

    def test_existing_binary_requires_force_and_failed_updates_preserve_it(self):
        archive, checksum = self.create()
        directory = self.prefix / "bin"
        directory.mkdir(parents=True)
        installed = directory / "aghandoff"
        installed.write_text("existing unrelated bytes")
        self.require_failure(self.install(archive, checksum), "already exists")
        self.assertEqual(installed.read_text(), "existing unrelated bytes")
        good = checksum.read_text()
        checksum.write_text("0" * 64 + "  " + archive.name + "\n")
        self.require_failure(self.install(archive, checksum, "--force"), "checksum mismatch")
        self.assertEqual(installed.read_text(), "existing unrelated bytes")
        checksum.write_text(good)
        self.require_success(self.install(archive, checksum, "--force"))
        self.assertEqual(installed.read_bytes(), self.binary.read_bytes())

    def test_destination_and_input_symlinks_are_refused(self):
        archive, checksum = self.create()
        external = self.root / "unrelated"
        external.write_text("keep me")
        directory = self.prefix / "bin"
        directory.mkdir(parents=True)
        (directory / "aghandoff").symlink_to(external)
        self.require_failure(self.install(archive, checksum, "--force"), "replace a symlink")
        self.assertEqual(external.read_text(), "keep me")
        linked = self.root / "archive link"
        linked.symlink_to(archive)
        self.require_failure(self.install(linked, checksum), "archive must be a regular file")
        linked_sum = self.root / "checksum link"
        linked_sum.symlink_to(checksum)
        self.require_failure(self.install(archive, linked_sum), "checksum must be a regular file")
        (directory / "aghandoff").unlink()
        directory.rmdir()
        external_directory = self.root / "external bin"
        external_directory.mkdir()
        directory.symlink_to(external_directory)
        self.require_failure(self.install(archive, checksum, "--force"), "installation directory")
        self.assertEqual(list(external_directory.iterdir()), [])

    def test_an_existing_installer_lock_is_preserved(self):
        archive, checksum = self.create()
        lock = self.prefix / "bin/.aghandoff-install.lock"
        lock.mkdir(parents=True)
        (lock / "synthetic-owner").write_text("keep me")
        self.require_failure(self.install(archive, checksum), "another installer")
        self.assertEqual((lock / "synthetic-owner").read_text(), "keep me")
        self.assertFalse((self.prefix / "bin/aghandoff").exists())

    def test_download_mode_uses_only_the_expected_release_urls(self):
        archive, checksum = self.create()
        fake_bin = self.root / "fake bin"
        fake_bin.mkdir()
        log = self.root / "download URLs"
        curl = fake_bin / "curl"
        curl.write_text("#!/bin/sh\nset -eu\nout=\nurl=\n"
                        "while [ $# -gt 0 ]; do case $1 in -o) out=$2; shift 2;; "
                        "https://*) url=$1; shift;; *) shift;; esac; done\n"
                        "printf '%s\\n' \"$url\" >> " + shlex.quote(str(log)) + "\n"
                        "case $url in *" + archive.name + ") cp " + shlex.quote(str(archive))
                        + " \"$out\";; *" + checksum.name + ") cp " + shlex.quote(str(checksum))
                        + " \"$out\";; *) exit 42;; esac\n")
        curl.chmod(0o700)
        env = {"PATH": str(fake_bin) + ":" + SYSTEM_PATH, "TMPDIR": str(self.root)}
        command = ["/bin/sh", str(ROOT / "scripts/install.sh"), "--prefix", str(self.prefix)]
        self.require_success(subprocess.run(command + ["--version", "v" + self.version],
                                            env=env, capture_output=True, text=True))
        base = "https://github.com/rrhaya/aghandoff/releases/download/v" + self.version + "/"
        self.assertEqual(log.read_text().splitlines(), [base + archive.name, base + checksum.name])
        self.require_success(subprocess.run(command + ["--force"], env=env,
                                            capture_output=True, text=True))
        self.assertIn("/releases/latest/download/", log.read_text())
        before = log.read_text()
        self.require_failure(subprocess.run(command + ["--version", "bad/../version"], env=env,
                                            capture_output=True, text=True))
        self.assertEqual(log.read_text(), before)

    def release_assets(self, commit=COMMIT):
        for target in package.TARGETS:
            self.create(target=target, commit=commit)

    def test_release_manifest_requires_all_four_verified_builds_from_the_same_commit(self):
        self.release_assets()
        self.assertEqual(prepare_release.prepare(ROOT, self.output, COMMIT), "v" + self.version)
        lines = (self.output / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(lines), 4)
        self.assertEqual(lines, sorted(lines, key=lambda line: line.split("  ")[1]))

    def test_release_validation_rejects_missing_extra_corrupt_or_wrong_source_artifacts(self):
        self.release_assets()
        target = self.output / ("aghandoff-" + package.TARGETS[0] + ".tar.gz")
        original = target.read_bytes()
        target.write_bytes(b"corrupt")
        with self.assertRaises(ValueError):
            prepare_release.prepare(ROOT, self.output, COMMIT)
        self.assertFalse((self.output / "SHA256SUMS").exists())
        target.write_bytes(original)
        with self.assertRaises(ValueError):
            prepare_release.prepare(ROOT, self.output, "b" * 40)
        extra = self.output / "unexpected"
        extra.write_text("synthetic")
        with self.assertRaises(ValueError):
            prepare_release.prepare(ROOT, self.output, COMMIT)
        extra.unlink()
        target.unlink()
        with self.assertRaises(ValueError):
            prepare_release.prepare(ROOT, self.output, COMMIT)

    def publish(self, fail_create=False, repository="rrhaya/aghandoff", ref="refs/heads/main",
                token="synthetic-job-token", fail_tag=False):
        fake_bin = self.root / "publish bin"
        fake_bin.mkdir(exist_ok=True)
        log = self.root / "publish calls"
        fake = fake_bin / "gh"
        fake.write_text("#!/usr/bin/env python3\nimport json,sys\n"
                        "with open(" + repr(str(log)) + ", 'a') as stream: "
                        "stream.write(json.dumps(sys.argv[1:]) + '\\n')\n"
                        + ("if sys.argv[1:3] == ['release', 'create']: sys.exit(7)\n" if fail_create else "")
                        + ("if sys.argv[1:2] == ['api']: sys.exit(7)\n" if fail_tag else ""))
        fake.chmod(0o700)
        env = {"PATH": str(fake_bin) + ":" + SYSTEM_PATH, "TMPDIR": str(self.root),
               "RUNNER_TEMP": str(self.root), "GITHUB_ACTIONS": "true",
               "GITHUB_REPOSITORY": repository, "GITHUB_REF": ref, "GITHUB_SHA": COMMIT}
        if token is not None:
            env["GH_TOKEN"] = token
        result = subprocess.run(["/bin/sh", str(ROOT / "scripts/publish_release.sh"), str(self.output)],
                                env=env, capture_output=True, text=True)
        calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
        return result, calls

    def test_publisher_creates_a_draft_then_publishes_only_after_upload_success(self):
        self.release_assets()
        result, calls = self.publish()
        self.require_success(result)
        self.assertEqual([call[:2] for call in calls],
                         [["api", "--method"], ["release", "create"], ["release", "edit"]])
        self.assertIn("POST", calls[0])
        self.assertIn("ref=refs/tags/v" + self.version, calls[0])
        self.assertIn("sha=" + COMMIT, calls[0])
        self.assertIn("--draft", calls[1])
        self.assertIn("--verify-tag", calls[1])
        self.assertIn("--draft=false", calls[2])
        self.assertIn(COMMIT, calls[1])
        self.assertIn("rrhaya/aghandoff", calls[1])
        self.assertNotIn("--clobber", " ".join(sum(calls, [])))

    def test_failed_draft_upload_never_runs_the_publish_command(self):
        self.release_assets()
        result, calls = self.publish(fail_create=True)
        self.require_failure(result)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[1][:2], ["release", "create"])

    def test_existing_or_failed_tag_creation_stops_before_release_creation(self):
        self.release_assets()
        result, calls = self.publish(fail_tag=True)
        self.require_failure(result)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][:2], ["api", "--method"])

    def test_publisher_refuses_other_repositories_and_branches_before_any_write(self):
        self.release_assets()
        for repository, ref in [("example/other", "refs/heads/main"),
                                ("rrhaya/aghandoff", "refs/heads/feature")]:
            result, calls = self.publish(repository=repository, ref=ref)
            self.require_failure(result)
            self.assertEqual(calls, [])
            self.assertFalse((self.output / "SHA256SUMS").exists())
        result, calls = self.publish(token=None)
        self.require_failure(result, "explicit job token")
        self.assertEqual(calls, [])
        self.assertFalse((self.output / "SHA256SUMS").exists())


if __name__ == "__main__":
    unittest.main()
