# Remaining work

| Area | Current state | Next verification or implementation |
| --- | --- | --- |
| Multi-account Claude sessions | Implemented, offline tested | Manual A → B → A acceptance test on real accounts |
| Existing conversations and personal configuration | Implemented, offline tested | Resume a synthetic native transcript saved before aghandoff; confirm instructions and skills load |
| Account usage | Opt-in local status-line snapshots implemented | Confirm live payload availability, profile attribution, and unchanged normal Claude setup |
| Installation | Source installation and checked local-archive installer | Verify on the user's terminal; keep the executable path stable for usage hooks |
| Binary distribution | Four native CI packages with checksums and install smoke tests | Approve the job-scoped publication permission, activate the release workflow, and publish v0.3.1 |
| Codex | Not implemented | Add a provider boundary, isolated account configuration, native-session persistence, and synthetic tests before live verification |
| Imported configuration updates | Snapshots only | Design explicit conflict handling and update previews |
| Crash recovery | Foreground locks and signal forwarding | Track orphaned processes after wrapper crashes and recover interrupted imports |

The manual steps use synthetic projects and messages: [acceptance procedure](manual-test.md). Do not include real conversation contents, account emails, or credentials in a report. Free accounts and fake CLI tests cannot establish live paid-session compatibility.
