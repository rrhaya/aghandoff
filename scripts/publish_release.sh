#!/bin/sh
# Used only by the separately approved GitHub Actions publishing job.
set -eu
[ "${GITHUB_ACTIONS:-}" = true ] || { echo 'publishing requires GitHub Actions' >&2; exit 1; }
[ "${GITHUB_REPOSITORY:-}" = rrhaya/aghandoff ] || { echo 'unexpected repository' >&2; exit 1; }
[ "${GITHUB_REF:-}" = refs/heads/main ] || { echo 'publishing requires main' >&2; exit 1; }
[ "$#" -eq 1 ] || { echo 'pass the artifact directory' >&2; exit 1; }
directory=$(cd "$1" && pwd -P)
root=$(CDPATH= cd "$(dirname "$0")/.." && pwd -P)
tag=$(python3 -B "$root/scripts/prepare_release.py" --directory "$directory" --commit "${GITHUB_SHA:?}")
notes=$(mktemp "${RUNNER_TEMP:-/tmp}/aghandoff-release.XXXXXX")
trap 'rm -f "$notes"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP
cat > "$notes" <<NOTES
Native macOS and Linux binaries for Intel and ARM64. Rust is not needed to run them.

Each archive has a SHA-256 sidecar; SHA256SUMS covers all four archives. See the
binary installation guide: https://github.com/rrhaya/aghandoff/blob/$tag/docs/install-binary.md

These builds pass synthetic tests and native install smoke checks. Live Claude
account handoff, imported transcript compatibility, and usage attribution still
require manual verification. macOS binaries are not Developer ID notarized.
NOTES
# An existing release or tag is never force-updated. A failed upload leaves a
# draft for inspection; it never exposes an incomplete public release.
gh release create "$tag" --repo rrhaya/aghandoff --target "$GITHUB_SHA" --draft \
    --title "aghandoff $tag" --notes-file "$notes" \
    "$directory"/*.tar.gz "$directory"/*.sha256 "$directory/SHA256SUMS"
gh release edit "$tag" --repo rrhaya/aghandoff --draft=false --latest
