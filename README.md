# aghandoff

Switch Claude Code accounts. Keep the conversation.

English · [日本語](README.ja.md)

A Rust CLI for using multiple Claude Code subscription accounts with separate logins and shared workspace conversations. Choose an account when you start, then resume the same session with another account after exiting Claude.

## Requirements

- macOS or Linux
- Git
- Rust 1.85+ with Cargo
- Claude Code 2.1.268+ (stable), with access to a Claude subscription

Rust is needed to build and install aghandoff. The installed binary runs without the Rust toolchain. Use the latest stable Claude Code for initial testing.

<details>
<summary>Install Rust with rustup</summary>

[rustup](https://rust-lang.org/tools/install/) is Rust's official recommended installer.

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the installer prompts, then reopen your terminal and check:

```sh
rustc --version
cargo --version
```

On macOS, if the build reports a missing linker, install the [Command Line Tools](https://developer.apple.com/documentation/xcode/installing-the-command-line-tools/) with `xcode-select --install`, then retry.

</details>

<details>
<summary>Use mise instead (optional)</summary>

If you already use [mise](https://mise.jdx.dev/lang/rust.html), you can manage Rust with it. mise uses rustup internally and installs it if needed, so a separate rustup installation is unnecessary.

```sh
mise use -g rust@stable
mise exec -- rustc --version
mise exec -- cargo --version
```

`-g` sets your personal default Rust version. After cloning the repository and entering its directory as shown below, install and check aghandoff with:

```sh
mise exec -- cargo install --path . --locked
mise exec -- aghandoff --help
```

If mise is not activated in your shell, prefix subsequent `aghandoff` commands with `mise exec --` as well. mise is optional; rustup alone is sufficient.

</details>

## Quick start

```sh
git clone https://github.com/rrhaya/aghandoff.git
cd aghandoff
cargo install --path . --locked
```

This installs `aghandoff` in Cargo's binary directory, normally `~/.cargo/bin`. Make sure that directory is on your `PATH`, or use `mise exec --` as described above.

```sh
aghandoff --help
```

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

## Bring your existing setup

Preview conversations saved before aghandoff, then copy them with `--apply`:

```sh
aghandoff import sessions myapp personal --from /path/to/old/project-history
aghandoff import sessions myapp personal --from /path/to/old/project-history --apply
```

Select one or more IDs with `--session <uuid>`, and add `--with-memory` to include the project's auto memory. The source is one directory under your normal `~/.claude/projects/`. The command prints how to resume the imported session.

Personal instructions and skills can be copied into each account profile:

```sh
aghandoff import config personal --instructions ~/.claude/CLAUDE.md --skill ~/.claude/skills/review
# After reviewing the preview, repeat with --apply.
```

You can select a personal `AGENTS.md` or a skill in another local directory instead. `--rules <dir>` copies personal rules; `--settings <file>` copies only five basic preferences. Authentication, hooks, environment variables, MCP configuration, and permission settings are excluded. Existing files are never overwritten. These are snapshots, not live synchronization; repeat for other profiles as needed.

Project instructions and `.claude/skills/` continue to load through Claude's normal rules. [Import guide](docs/import.md) covers selection, conflicts, and limits.

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
| `import sessions <workspace> <account> --from <dir>` | Preview existing native conversations; add `--apply` to copy |
| `import config <account> [options]` | Preview selected personal instructions, skills, rules, and preferences |

Names use lowercase letters, digits, `-`, or `_`, begin with a letter or digit, and contain up to 48 characters.

`continue` selects the last launch that exited successfully and left a transcript. After an interrupted or failed launch, use the session ID printed at startup with `resume`.

## How it works

Each profile has its own `CLAUDE_CONFIG_DIR`. Registered workspaces share conversation files and auto memory through a workspace-specific link, and Claude resumes them using the original session ID. Authentication, MCP credentials, extensions, and prompt history remain separate. Personal settings remain separate unless explicitly copied with `import config`.

Claude handles login, credential storage, and refresh. aghandoff checks Claude's authentication status without opening credential files or the OS keychain. It adds no proxy and holds account and workspace locks while a managed foreground session runs.

**Shared history may be sent to the account you resume with.** Use accounts that are permitted to access the same project. Profile directories provide storage separation, not a security boundary against other processes running as your OS user.

## Configuration

State is stored in `~/.local/share/aghandoff`. Use a new, private directory when overriding it, and keep its location fixed.

```sh
aghandoff --data-dir /private/path/aghandoff account list
aghandoff --claude-bin /path/to/claude status personal
```

## Current scope

- Claude subscription logins, sessions created through aghandoff, and imported native transcripts.
- Account changes after exiting Claude; managed foreground sessions only.
- Selected local history and personal configuration import. Codex support and account usage dashboards are planned.

Use aghandoff's `run` and `resume` to change conversations. Session changes made with Claude's `/clear` or `/resume` are not tracked. Check usage with Claude's `/usage` for now.

Restored context is subject to Claude's session retention and account or organization restrictions. Running tools, processes, and server-side caches do not transfer. Sharing workspace storage across profiles is an integration approach that still needs validation against the live CLI.

## Development

```sh
cargo fmt
cargo fmt --check
cargo test --all-targets --locked
cargo clippy --all-targets --locked -- -D warnings
```

The suite includes 22 offline integration tests using a fake Claude CLI and synthetic data. It covers profile separation, shared transcripts and memory, resume failures, process locking, history migration, configuration selection, and import conflicts.

`Cargo.lock` pins the dependency versions used in CI.

CI checks release builds, formatting, Clippy, and all offline integration tests on Ubuntu 24.04 and macOS 14 with Rust 1.85.1. Live account handoff and imported transcript compatibility still need manual verification.

[Design notes](docs/design.md) · [Manual acceptance test](docs/manual-test.md)

## License

[MIT](LICENSE)
