#!/usr/bin/env python3
"""Validate the exact four build outputs before a release can be published."""
import argparse
import hashlib
from pathlib import Path
import re
import tarfile

from package import TARGETS, MAX_BINARY, version


def prepare(root, directory, commit):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("invalid commit SHA")
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("artifact directory must be a real directory")
    allowed = {"aghandoff-" + target + ".tar.gz" for target in TARGETS}
    allowed |= {name + ".sha256" for name in allowed}
    actual = {path.name for path in directory.iterdir()}
    if actual != allowed:
        raise ValueError("release requires exactly four archives and four checksum files")
    checksums = []
    for target in sorted(TARGETS):
        name = "aghandoff-" + target + ".tar.gz"
        archive, checksum = directory / name, directory / (name + ".sha256")
        if (archive.is_symlink() or checksum.is_symlink()
                or not archive.is_file() or not checksum.is_file()
                or archive.stat().st_size > 128 * 1024 * 1024
                or checksum.stat().st_size > 512):
            raise ValueError("unexpected artifact type or size")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        line = digest + "  " + name + "\n"
        if checksum.read_text() != line:
            raise ValueError("artifact checksum mismatch")
        with tarfile.open(archive, "r:gz") as tar:
            members = tar.getmembers()
            if ({member.name for member in members} != {"aghandoff", "LICENSE", "BUILD-INFO"}
                    or len(members) != 3 or not all(member.isfile() for member in members)):
                raise ValueError("unexpected archive members or types")
            if any(member.size > MAX_BINARY for member in members):
                raise ValueError("archive member exceeds the size limit")
            info = tar.getmember("BUILD-INFO")
            if info.size > 1024:
                raise ValueError("oversized build information")
            expected = ("version=" + version(root) + "\ntarget=" + target
                        + "\ncommit=" + commit + "\n").encode()
            if tar.extractfile(info).read() != expected:
                raise ValueError("artifact source, version, or target differs from this release")
        checksums.append(line)
    # Never overwrite a prior manifest or write output before every archive passes.
    with (directory / "SHA256SUMS").open("x") as stream:
        stream.write("".join(checksums))
    return "v" + version(root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    print(prepare(Path(__file__).resolve().parent.parent, args.directory, args.commit))


if __name__ == "__main__":
    main()
