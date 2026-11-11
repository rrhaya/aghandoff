#!/usr/bin/env python3
"""Create bounded, deterministic release archives from an explicitly built binary."""
import argparse
import gzip
import hashlib
import io
from pathlib import Path
import re
import subprocess
import tarfile

TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
)
MAX_BINARY = 64 * 1024 * 1024


def version(root):
    match = re.search(r'^version = "([0-9]+\.[0-9]+\.[0-9]+)"$',
                      (root / "Cargo.toml").read_text(), re.MULTILINE)
    if not match:
        raise ValueError("expected a stable package version in Cargo.toml")
    return match.group(1)


def create(root, binary, target, commit, output):
    if target not in TARGETS or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("unsupported target or commit SHA")
    if binary.is_symlink() or not binary.is_file() or not 0 < binary.stat().st_size <= MAX_BINARY:
        raise ValueError("binary must be a regular file of at most 64 MiB")
    expected = "aghandoff " + version(root)
    result = subprocess.run([str(binary.resolve()), "--version"], check=True,
                            capture_output=True, text=True)
    if result.stdout.strip() != expected:
        raise ValueError("binary version differs from Cargo.toml")
    files = {
        "aghandoff": (binary.read_bytes(), 0o755),
        "LICENSE": ((root / "LICENSE").read_bytes(), 0o644),
        "BUILD-INFO": (("version=" + version(root) + "\ntarget=" + target
                        + "\ncommit=" + commit + "\n").encode(), 0o644),
    }
    output.mkdir(parents=True, exist_ok=True)
    name = "aghandoff-" + target + ".tar.gz"
    archive = output / name
    checksum = output / (name + ".sha256")
    if archive.exists() or archive.is_symlink() or checksum.exists() or checksum.is_symlink():
        raise ValueError("package destination already exists")
    created = False
    try:
        with archive.open("xb") as raw:
            created = True
            with gzip.GzipFile(filename="", fileobj=raw, mode="wb", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as tar:
                    for name, (data, mode) in sorted(files.items()):
                        entry = tarfile.TarInfo(name)
                        entry.size, entry.mode, entry.mtime = len(data), mode, 0
                        entry.uid = entry.gid = 0
                        entry.uname = entry.gname = ""
                        tar.addfile(entry, io.BytesIO(data))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        with checksum.open("x") as stream:
            stream.write(digest + "  " + archive.name + "\n")
    except BaseException:
        if created:
            archive.unlink(missing_ok=True)
        raise
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--output-dir", type=Path, default=Path("dist"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    print(create(root, args.binary, args.target, args.commit, args.output_dir))


if __name__ == "__main__":
    main()
