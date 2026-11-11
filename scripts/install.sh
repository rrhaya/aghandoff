#!/bin/sh
# Install only a checksummed native binary. Never changes shell configuration.
set -eu
umask 077

fail() { printf '%s\n' "aghandoff installer: $*" >&2; exit 1; }
usage() {
    cat <<'USAGE'
Usage: sh install.sh [--version VERSION] [--prefix DIRECTORY] [--force]
       sh install.sh --archive FILE --checksum FILE [--prefix DIRECTORY] [--force]

The default prefix is ~/.local; the binary is installed as PREFIX/bin/aghandoff.
Download mode uses only github.com/rrhaya/aghandoff Releases. Local archive mode
requires both files from the same build. Existing binaries require --force.
USAGE
}

prefix=${HOME:+$HOME/.local}
version=latest
version_set=no
archive=
checksum=
force=no
while [ "$#" -gt 0 ]; do
    case "$1" in
        --prefix|--version|--archive|--checksum)
            [ "$#" -ge 2 ] && [ -n "$2" ] || fail "missing value for $1"
            case "$1" in
                --prefix) prefix=$2 ;;
                --version) version=$2; version_set=yes ;;
                --archive) archive=$2 ;;
                --checksum) checksum=$2 ;;
            esac
            shift 2 ;;
        --force) force=yes; shift ;;
        --help|-h) usage; exit 0 ;;
        *) fail "unknown option; use --help" ;;
    esac
done

[ -n "$prefix" ] || fail "HOME is unset; pass --prefix"
while [ "$prefix" != / ] && [ "${prefix%/}" != "$prefix" ]; do prefix=${prefix%/}; done
case "$prefix" in /*) ;; *) prefix=$PWD/$prefix ;; esac
case "/$prefix/" in */../*|*/./*) fail "prefix must not contain . or .. components" ;; esac
case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin ;;
    Darwin:x86_64|Darwin:amd64) target=x86_64-apple-darwin ;;
    Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
    Linux:x86_64|Linux:amd64) target=x86_64-unknown-linux-gnu ;;
    *) fail "unsupported OS or architecture; use a source build" ;;
esac
asset=aghandoff-$target.tar.gz

work=
prepared=
lock_dir=
cleanup() {
    [ -z "$prepared" ] || rm -f "$prepared"
    [ -z "$lock_dir" ] || rmdir "$lock_dir" 2>/dev/null || :
    if [ -n "$work" ]; then
        rm -f "$work/archive" "$work/checksum" "$work/list" "$work/sorted" \
            "$work/expected" "$work/types" "$work/aghandoff" "$work/BUILD-INFO"
        rmdir "$work" 2>/dev/null || :
    fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
work=$(mktemp -d "${TMPDIR:-/tmp}/aghandoff-install.XXXXXX")

if [ -n "$archive" ] || [ -n "$checksum" ]; then
    [ -n "$archive" ] && [ -n "$checksum" ] || fail "local mode requires --archive and --checksum"
    [ "$version_set" = no ] || fail "--version cannot be combined with a local archive"
else
    if [ "$version" = latest ]; then
        base=https://github.com/rrhaya/aghandoff/releases/latest/download
    else
        version=${version#v}
        printf '%s\n' "$version" | LC_ALL=C grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
            || fail "version must have the form 0.3.1"
        base=https://github.com/rrhaya/aghandoff/releases/download/v$version
    fi
    command -v curl >/dev/null 2>&1 || fail "curl is required for download mode"
    curl -q --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
        --connect-timeout 10 --max-time 120 "$base/$asset" -o "$work/archive"
    curl -q --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
        --connect-timeout 10 --max-time 120 "$base/$asset.sha256" -o "$work/checksum"
    archive=$work/archive
    checksum=$work/checksum
fi

[ -f "$archive" ] && [ ! -L "$archive" ] || fail "archive must be a regular file"
[ -f "$checksum" ] && [ ! -L "$checksum" ] || fail "checksum must be a regular file"
[ "$(wc -c < "$archive")" -le 134217728 ] || fail "archive exceeds 128 MiB"
[ "$(wc -c < "$checksum")" -le 512 ] || fail "invalid checksum file"
IFS=' ' read -r expected listed extra < "$checksum" || fail "invalid checksum file"
[ "${#expected}" -eq 64 ] || fail "invalid checksum digest"
case "$expected" in *[!0-9a-f]*) fail "invalid checksum digest" ;; esac
[ "$listed" = "$asset" ] && [ -z "$extra" ] || fail "checksum belongs to a different asset"
[ "$(cat "$checksum")" = "$expected  $asset" ] || fail "checksum file must contain one asset"
if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum < "$archive")
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 < "$archive")
else
    fail "sha256sum or shasum is required"
fi
actual=${actual%% *}
[ "$actual" = "$expected" ] || fail "checksum mismatch; nothing installed"

# Inspect all member names and types before extracting anything. Only named
# regular members are streamed to files whose paths we control.
tar -tzf "$archive" > "$work/list" || fail "cannot list archive"
LC_ALL=C sort "$work/list" > "$work/sorted"
printf 'BUILD-INFO\nLICENSE\naghandoff\n' > "$work/expected"
cmp -s "$work/sorted" "$work/expected" || fail "unexpected, duplicate, or unsafe archive members"
tar -tvzf "$archive" > "$work/types" || fail "cannot inspect archive types"
if LC_ALL=C grep -qv '^-' "$work/types"; then
    fail "archive links and special files are refused"
fi
(ulimit -f 131072; tar -xOzf "$archive" aghandoff > "$work/aghandoff") \
    || fail "cannot extract bounded binary"
[ "$(wc -c < "$work/aghandoff")" -le 67108864 ] || fail "binary exceeds 64 MiB"
(ulimit -f 8; tar -xOzf "$archive" BUILD-INFO > "$work/BUILD-INFO") \
    || fail "invalid build information"
build_version=$(sed -n '1s/^version=//p' "$work/BUILD-INFO")
build_target=$(sed -n '2s/^target=//p' "$work/BUILD-INFO")
printf '%s\n' "$build_version" | LC_ALL=C grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' \
    || fail "invalid build version"
[ "$build_target" = "$target" ] || fail "archive target does not match this machine"
[ "$version" = latest ] || [ "$version" = "$build_version" ] || fail "unexpected binary version"
chmod 700 "$work/aghandoff"
reported=$("$work/aghandoff" --version) || fail "binary cannot run on this machine; nothing installed"
[ "$reported" = "aghandoff $build_version" ] || fail "binary version differs from build information"

[ ! -L "$prefix" ] && [ ! -L "$prefix/bin" ] || fail "installation directory must not be a symlink"
mkdir -p "$prefix/bin"
mkdir "$prefix/bin/.aghandoff-install.lock" 2>/dev/null || fail "another installer is active or its lock remains"
lock_dir=$prefix/bin/.aghandoff-install.lock
destination=$prefix/bin/aghandoff
[ ! -L "$destination" ] || fail "refusing to replace a symlink"
if [ -e "$destination" ]; then
    [ -f "$destination" ] || fail "destination is not a regular file"
    [ "$force" = yes ] || fail "binary already exists; use --force to replace it"
fi
prepared=$(mktemp "$prefix/bin/.aghandoff.XXXXXX")
cat "$work/aghandoff" > "$prepared"
chmod 755 "$prepared"
if [ "$force" = yes ]; then
    mv -f "$prepared" "$destination"
    prepared=
else
    ln "$prepared" "$destination" || fail "destination appeared during installation"
fi
printf 'Installed aghandoff %s at %s\n' "$build_version" "$destination"
printf 'Add %s/bin to PATH if needed.\n' "$prefix"
