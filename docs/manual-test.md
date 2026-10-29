# Manual acceptance test

Status: not yet performed.

Use your own accounts and a small synthetic project. Offline integration tests cannot verify Claude's credential isolation or live transcript compatibility.

## Prerequisites

- Run `cargo fmt`, then pass `cargo fmt --check`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.
- Use the latest stable Claude Code.
- Prepare a dedicated empty data directory and a synthetic project directory.
- Keep existing Claude configuration, history, and credentials in place.

If using `--data-dir`, pass the same path on every command below.

## Procedure

1. Create profiles with `account add a` and `account add b`.
2. Run `login` for each profile, choosing a different Claude account in the browser. Confirm that both `status` commands succeed.
3. Register the synthetic project with `workspace add demo PATH`.
4. Start `run demo a`. Confirm account A with Claude's `/status` before sending a message. Ask Claude to remember a unique synthetic phrase and exchange a few short messages. Optionally create synthetic auto memory.
5. Use `/exit`. Check that `sessions demo` lists the UUID with `transcript=true` and `last`.
6. Start `continue demo b`. Confirm account B with `/status` before sending a message. Check the restored conversation and ask for the synthetic phrase.
7. Add a message as B and exit. Run `continue demo a` and confirm that B's addition is present.
8. While A is running, try to launch B in the same workspace from another terminal. Expect a busy error.
9. Use terminal Ctrl+C. Confirm that the lock remains held while Claude continues running and is released after it exits.
10. If you also use a normal Claude installation, confirm through its own status display that its account and settings are unchanged. Do not display credential values.

## Failure criteria

- If the wrong account appears, exit before sending a message. Authentication isolation has not passed.
- If Claude rejects the transcript because of organization boundaries or thinking signatures, resume with the original account. Do not modify history or disable validation to bypass the rejection.
- If an empty conversation opens, exit without continuing. Session continuity has not passed.

Record the Claude version, OS, step number, result, and redacted errors. Exclude account emails, tokens, and real conversation content.
