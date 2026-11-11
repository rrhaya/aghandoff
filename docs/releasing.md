# Build and publish packages

## Active build workflow

`Package binaries` builds and tests macOS Intel/ARM64 and Linux GNU Intel/ARM64 natively. It runs the Rust and distribution tests and installs each actual release binary with `PATH=/usr/bin:/bin`. It uploads only the archive and SHA-256 sidecar. Active jobs retain `contents: read`; checkout does not persist credentials.

Packages contain exactly `aghandoff`, `LICENSE`, and `BUILD-INFO`. Archive owner names and timestamps are fixed. Package, Cargo, and executable versions must match; the recorded source is the workflow's exact `github.sha`. This normalizes archive metadata, without claiming independently reproducible compiler output.

The pinned official artifact actions are MIT-licensed: [upload-artifact v7.0.0](https://github.com/actions/upload-artifact/tree/bbbca2ddaa5d8feaa63e36b76fdaad77386f024f), [download-artifact v8.0.0](https://github.com/actions/download-artifact/tree/70fc10c6e5e1ce46ad2ea6f2b72d43f7d47b13c3).

## Proposed publication job

[`docs/proposals/release-workflow.yml`](proposals/release-workflow.yml) is inactive at that location. Activation moves it to `.github/workflows/release.yml`, after approval of this job-scoped permission:

```yaml
publish:
  permissions:
    contents: write
```

Global and build-job permissions remain `contents: read`. The publish job is restricted to `rrhaya/aghandoff` on `refs/heads/main`. GitHub provides its short-lived job token only to the publish step; no personal token, connector, or stored credential is added.

After activation, a main push changing `Cargo.toml` or the release workflow, or a manual dispatch on main, builds all four packages. Publication then:

1. Downloads only this run's artifacts, treating digest mismatches as errors.
2. Requires exactly four archives and four sidecars; validates all hashes, member types, versions, targets, and source SHAs before writing `SHA256SUMS`.
3. Creates a **draft** `v<package-version>` Release targeting that source SHA and uploads all assets.
4. Publishes the draft only when every upload succeeds.

The first planned release is `v0.3.1`. Existing releases and tags are not overwritten or force-updated. Upload failures leave a draft for inspection; verify its source and assets before retrying. Alter or delete existing releases/tags only with explicit authorization.

Reverting the activation commit stops automatic publication and removes its write permission. Published assets and tags remain until separately handled.

## Verification

```sh
python3 -B -m unittest discover -s tests -p test_distribution.py -v
sh -n scripts/install.sh scripts/publish_release.sh
cargo test --all-targets --locked
```

Distribution tests use synthetic binaries, local archives, a stub downloader, and a stub `gh`. They do not contact GitHub or use account credentials. Native CI separately installs and starts each actual compiled executable. Real Release API calls remain unexercised until publication is approved and activated.
