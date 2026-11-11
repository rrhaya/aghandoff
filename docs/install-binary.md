# Install a prebuilt binary

Prebuilt packages need no Rust, Cargo, mise, or Python to install or run. Claude Code and access to your chosen accounts are still required.

## Download a tested build

Open the [Package binaries workflow](https://github.com/rrhaya/aghandoff/actions/workflows/package.yml), choose a successful **main** run, and download your machine's artifact. Actions artifact downloads require GitHub sign-in and expire after 14 days. Extract the ZIP to obtain its `.tar.gz` and `.tar.gz.sha256` files.

| Machine | Archive target | Native build and install test |
| --- | --- | --- |
| Apple Silicon Mac | `aarch64-apple-darwin` | macOS 15 |
| Intel Mac | `x86_64-apple-darwin` | macOS 15 |
| Intel/AMD 64-bit Linux | `x86_64-unknown-linux-gnu` | Ubuntu 22.04 |
| ARM64 Linux | `aarch64-unknown-linux-gnu` | Ubuntu 22.04 ARM |

Linux packages use GNU/glibc; musl/Alpine packages are not provided. Older macOS versions and other Linux distributions are not covered by these tests. macOS packages are not Developer ID notarized; use a source build if your machine's policy does not allow them.

Download the installer into a file so you can inspect it:

```sh
curl -q --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/rrhaya/aghandoff/main/scripts/install.sh -o aghandoff-install.sh
```

For Apple Silicon, for example:

```sh
sh aghandoff-install.sh --archive ./aghandoff-aarch64-apple-darwin.tar.gz --checksum ./aghandoff-aarch64-apple-darwin.tar.gz.sha256
"$HOME/.local/bin/aghandoff" --version
```

Use the archive and checksum from the **same build**. The installer checks the checksum, exact archive members and file types, native target, and version before installing at `~/.local/bin/aghandoff`. Add `~/.local/bin` to PATH if needed; shell configuration is not edited.

Only the executable is installed. Claude profiles, settings, credentials, histories, and aghandoff's state directory are untouched. The archive also includes the MIT license and `BUILD-INFO` with source SHA, target, and version. Checksums detect changed downloads; they are not a signing identity or independent source attestation.

## Updates and custom paths

```sh
sh aghandoff-install.sh --archive ./aghandoff-aarch64-apple-darwin.tar.gz --checksum ./aghandoff-aarch64-apple-darwin.tar.gz.sha256 --prefix "$HOME/.local" --force
```

`--prefix` selects the parent of `bin/`. Existing binaries require `--force`; symlink destinations are refused. Keep the executable path stable for configured usage status lines. An existing Cargo installation normally uses `--prefix "$HOME/.cargo"`; otherwise follow the [usage guide](usage.md) to update its command path.

The installer uses standard shell tools, including `tar`, `mktemp`, `cmp`, and `sha256sum` or `shasum`. Download mode additionally needs `curl`. No `sudo` is needed for the default user directory. A verified candidate's `--version` must succeed before an update. Cooperating installers use a directory lock; inspect a leftover `.aghandoff-install.lock` after an interrupted install before retrying.

## Public Release downloads

The installer also supports stable GitHub Releases:

```sh
sh aghandoff-install.sh --version 0.3.1
```

Omitting `--version` requests the latest published Release. These commands require an actual published Release; CI artifacts are a separate channel. Automatic publishing is not enabled yet. The activation proposal is [release-workflow.yml](proposals/release-workflow.yml); see the [maintainer guide](releasing.md).

A download or integrity failure stops installation without falling back to an unchecked binary.
