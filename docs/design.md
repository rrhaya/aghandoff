# Design

## Goal

Use two or more Claude subscription accounts with the same workspace's saved conversations and context. Select a profile on each launch; there is no global active account.

## Approach

1. Pass a fixed, absolute `CLAUDE_CONFIG_DIR` for each account profile.
2. Set `CLAUDE_CODE_PROJECT_DIR_NAME=aghandoff-<workspace>` to give the workspace's transcripts and auto memory a stable directory name.
3. Link each profile's `projects/aghandoff-<workspace>` to the shared storage for that workspace.
4. Generate a UUID and pass `--session-id` for a new conversation. Pass the same UUID with `--resume` to continue it.
5. Inspect `loggedIn`, `authMethod`, and `configDirectory` from `claude auth status`. Do not store the full response or account email, or open credential files.
6. Hold OS file locks for the account and workspace until the foreground child process exits.

The environment variables and resume flags are official Claude features. Sharing a symlinked project directory across profiles is aghandoff's integration approach; it is not an Anthropic compatibility guarantee. It needs acceptance testing against the live CLI.

## Storage

```text
~/.local/share/aghandoff/                  # 0700
  .aghandoff-root                        # Store format marker
  registry.json                          # Profile labels and workspace paths
  registry.lock                          # Short registration operations
  accounts/
    personal/                            # CLAUDE_CONFIG_DIR
      active.lock                        # Login and launch exclusion
      projects/
        aghandoff-myapp -> ../../../workspaces/myapp/transcripts
      ...                                # Managed by Claude; not enumerated
    second/
      projects/
        aghandoff-myapp -> ../../../workspaces/myapp/transcripts
  workspaces/
    myapp/
      active.lock                        # Foreground launch exclusion
      sessions.json                      # IDs, profiles, exit codes, timestamps
      transcripts/
        <uuid>.jsonl                     # Managed by Claude; contents not read
        <uuid>/                          # Related files managed by Claude
        memory/                          # Claude auto memory
```

The implementation uses absolute symlink targets. Moving or renaming a store is not supported. Keep its path fixed, since the configuration directory also affects credential storage identity on macOS.

aghandoff checks transcript existence and file type without parsing conversation contents. It rejects a workspace path that disappears or resolves to a different path, and refuses to overwrite corrupt metadata as if it were empty.

## Session lifecycle

```text
account add -> login through Claude -> verify authentication
workspace add
run A -> record UUID -> launch Claude -> save exit metadata
                                            |
                               success + transcript exists
                                            |
                                    update last UUID
                                            |
continue B -> --resume with the same UUID and workspace
```

Record the UUID before launch so an interrupted wrapper can be recovered with an explicit `resume`. Only a successful launch with a saved transcript advances the last-session pointer. A failed launch must not hide the preceding resumable conversation. The last selected profile is launch metadata, not usage attribution.

## Authentication and sharing

Claude owns credential storage and refresh. aghandoff uses its status command to check for a Claude subscription login and the expected configuration directory. It rejects non-empty environment overrides that could select another credential store, provider, or endpoint, reporting variable names without values.

Account labels do not identify the underlying account. `configDirectory` verification alone does not prove that two profiles use different account IDs. Users must confirm the actual login through Claude's `/status`.

Only profiles explicitly used in a registered workspace receive that workspace's storage link. The shared transcripts and memory may be sent under the selected account on resume. This layout separates storage; it does not isolate processes running as the same OS user.

User settings, MCP credentials, extensions, and prompt history are not copied. Claude continues to apply project configuration and its own permission system.

## Concurrency and recovery

The registry lock covers short metadata operations. Account and workspace locks cover foreground launches; different accounts in different workspaces can run concurrently. SIGTERM and SIGHUP are forwarded to the child while the wrapper waits. Terminal SIGINT also reaches Claude, so the wrapper waits rather than exiting immediately.

SIGKILL or a wrapper crash can leave a child running after its lock is released. Confirm the child has exited before resuming. Direct Claude launches and background sessions do not participate in these locks; use managed foreground sessions for the initial version.

## Deferred work

- Importing existing Claude settings and conversations, with explicit selection and non-destructive diffs.
- Usage snapshots from the official status line, including observation times and explicit unknown states.
- Codex support and cross-agent transcript conversion.
- Stronger process tracking and recovery after wrapper crashes.

Token management, direct keychain access, in-process account replacement, and automatic rotation are outside the initial scope.

## Acceptance questions

- Does Claude use the linked directory consistently for transcripts, auto memory, and related subagent files?
- Does the installed Claude version isolate both configuration and authentication, including macOS Keychain entries?
- Can the target account or organization resume the transcript? If Claude rejects signed thinking or other organization-bound content, resume with the original account; do not strip signatures or disable validation.
- How does Claude's own transcript cleanup affect shared storage? aghandoff does not currently create backups.

## References

Consulted on 2026-10-27. Upstream behavior may change.

- [Multiple accounts and credential storage](https://code.claude.com/docs/en/authentication#log-in-with-multiple-accounts)
- [Configuration and project directory variables](https://code.claude.com/docs/en/env-vars)
- [Session storage and resume](https://code.claude.com/docs/en/sessions)
- [CLI commands and flags](https://code.claude.com/docs/en/cli-reference)
- [Status line data for future usage tracking](https://code.claude.com/docs/en/statusline)
