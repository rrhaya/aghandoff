# Remaining work

| Area | Current state | Next verification or implementation |
| --- | --- | --- |
| Multi-account Claude sessions | Implemented, offline tested | Manual A → B → A acceptance test on real accounts |
| Existing conversations and personal configuration | Implemented, offline tested | Resume a synthetic native transcript saved before aghandoff; confirm instructions and skills load |
| Account usage | Opt-in local status-line snapshots implemented | Confirm live payload availability, profile attribution, and unchanged normal Claude setup |
| Installation | Source installation with Rust; rustup and optional mise documented | Verify installation on a fresh terminal; configure a Rust version if using mise shims |
| Binary distribution | Not implemented | Build downloadable binaries with checksums and release/install smoke tests |
| Codex | Not implemented | Add a provider boundary, isolated account configuration, native-session persistence, and synthetic tests before live verification |
| Imported configuration updates | Snapshots only | Design explicit conflict handling and update previews |
| Crash recovery | Foreground locks and signal forwarding | Track orphaned processes after wrapper crashes and recover interrupted imports |

The manual steps use synthetic projects and messages: [acceptance procedure](manual-test.md). Do not include real conversation contents, account emails, or credentials in a report. Free accounts and fake CLI tests cannot establish live paid-session compatibility.
