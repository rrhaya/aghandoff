# aghandoff

Switch Claude Code accounts. Keep the conversation.

English · [日本語](README.ja.md)

A Rust CLI for using multiple Claude Code subscription accounts with separate logins and shared workspace conversations. Choose an account when you start, then resume the same session with another account after exiting Claude.

## Quick start

Requires macOS or Linux, Rust 1.85+, and Claude Code 2.1.268+ (stable). Use the latest stable Claude Code for initial testing.

```sh
git clone https://github.com/rrhaya/aghandoff.git
cd aghandoff
cargo install --path . --locked
```

This installs `aghandoff` in Cargo's binary directory. Make sure that directory is on your `PATH`.

```sh
# Create two profiles and sign in through Claude's own login flow.
aghandoff account add personal
aghandoff account add second
aghandoff login personal
aghandoff login second

# Register the project whose conversations you want to share.
aghandoff workspace add myapp /path/to/myapp

# Start with one account.
aghandoff run myapp personal

# After /exit, resume the same conversation with the other account.
aghandoff continue myapp second
```

Inside Claude, use `/status` to confirm the account before sending a message. Profile names are local labels; signing into the same account under two labels does not create separate accounts.

## Commands

| Command | Purpose |
| --- | --- |
| `account add <name>` / `account list` | Create or list account profiles |
| `login <account>` | Sign in with the selected profile |
| `status <account>` | Check subscription authentication and the profile directory |
| `workspace add <name> <path>` / `workspace list` | Register or list shared workspaces |
| `run <workspace> <account>` | Start a new conversation |
| `continue <workspace> <account>` | Resume the last successfully saved session |
| `resume <workspace> <account> <session-id>` | Resume a specific registered session |
| `sessions <workspace>` | List session IDs and launch metadata |

Names use lowercase letters, digits, `-`, or `_`, begin with a letter or digit, and contain up to 48 characters.

`continue` selects the last launch that exited successfully and left a transcript. After an interrupted or failed launch, use the session ID printed at startup with `resume`.

## How it works

Each profile has its own `CLAUDE_CONFIG_DIR`. Registered workspaces share conversation files and auto memory through a workspace-specific link, and Claude resumes them using the original session ID. Authentication, user settings, MCP credentials, extensions, and prompt history remain separate.

Claude handles login, credential storage, and refresh. aghandoff checks Claude's authentication status without opening credential files or the OS keychain. It adds no proxy and holds account and workspace locks while a managed foreground session runs.

**Shared history may be sent to the account you resume with.** Use accounts that are permitted to access the same project. Profile directories provide storage separation, not a security boundary against other processes running as your OS user.

## Configuration

State is stored in `~/.local/share/aghandoff`. Use a new, private directory when overriding it, and keep its location fixed.

```sh
aghandoff --data-dir /private/path/aghandoff account list
aghandoff --claude-bin /path/to/claude status personal
```

## Current scope

- Claude subscription logins and sessions created through aghandoff.
- Account changes after exiting Claude; managed foreground sessions only.
- Existing history import, Codex support, and account usage dashboards are planned.

Use aghandoff's `run` and `resume` to change conversations. Session changes made with Claude's `/clear` or `/resume` are not tracked. Check usage with Claude's `/usage` for now.

Restored context is subject to Claude's session retention and account or organization restrictions. Running tools, processes, and server-side caches do not transfer. Sharing workspace storage across profiles is an integration approach that still needs validation against the live CLI.

## Development

```sh
cargo fmt
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```

The suite includes 13 offline integration tests using a fake Claude CLI and synthetic data. It covers profile separation, shared transcripts and memory, resume failures, and process locking.

`Cargo.lock` pins the dependency versions used in CI.

Release builds, Clippy, and all 13 offline integration tests pass on Ubuntu 24.04 and macOS 14 with Rust 1.85.1. Live account handoff still needs manual verification.

[Design notes](docs/design.md) · [Manual acceptance test](docs/manual-test.md)

## License

[MIT](LICENSE)
