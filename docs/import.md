# Import existing Claude data

Imports are local copies. They do not contact Claude or require a paid account. The original files remain unchanged. Every command previews its selection unless you add `--apply`.

First create an account profile and register the original project directory. You can import before signing in:

```sh
aghandoff account add personal
aghandoff workspace add myapp /path/to/myapp
```

## Conversations saved before aghandoff

Claude normally saves native transcripts in `~/.claude/projects/<project>/<session-id>.jsonl`. Use the project directory containing the sessions you want, rather than the entire `~/.claude` directory. A custom `CLAUDE_CONFIG_DIR` uses its own `projects/` directory. To locate a particular session, Claude's hook and status-line inputs expose `transcript_path`; see [session storage](https://code.claude.com/docs/en/sessions#where-transcripts-are-stored).

Exit Claude sessions writing to the source project before importing. Preview all saved sessions in that project:

```sh
aghandoff import sessions myapp personal --from /path/to/old/project-history
```

After checking the listed destinations, repeat with `--apply`:

```sh
aghandoff import sessions myapp personal --from /path/to/old/project-history --apply
aghandoff sessions myapp
```

The importer copies each native `<uuid>.jsonl` and its matching `<uuid>/` directory, including related subagent files. It preserves the bytes and IDs; it does not convert text exported with `/export` into a resumable session. Add `--session <uuid>` to select one session, or repeat that option for several. Without it, all canonical UUID-named transcripts in this directory are selected.

```sh
aghandoff import sessions myapp personal --from /path/to/old/project-history --session 74af620e-0a44-4a9a-830e-a33475c013aa --with-memory
```

`--with-memory` also copies the selected project's `memory/` directory. It is project-wide memory, not memory belonging only to the selected session. It is opt-in and is never merged over existing files.

An empty workspace sets its `continue` pointer to the most recently modified imported transcript. An existing pointer stays unchanged. The apply command prints an explicit `resume` command for the imported selection:

```sh
aghandoff login personal
aghandoff resume myapp personal 74af620e-0a44-4a9a-830e-a33475c013aa
# After /exit, use another authenticated profile.
aghandoff continue myapp second
```

The account argument records your intended initial profile; it does not claim that profile originally created the conversation. Imported entries have no previous process exit code. As with other handoffs, the conversation may be sent under the account you resume with. Original working files, external file-history checkpoints, running processes, and server-side state are not imported.

## Personal instructions and skills

Project files such as `CLAUDE.md`, `CLAUDE.local.md`, `.claude/settings.local.json`, and `.claude/skills/` remain in the original project. Claude loads them according to its own rules; importing them is unnecessary.

Personal files under your normal `~/.claude` are separate from aghandoff's account profiles. Select the parts to copy:

```sh
aghandoff import config personal --instructions ~/.claude/CLAUDE.md
aghandoff import config personal --skill ~/.claude/skills/review
aghandoff import config personal --rules ~/.claude/rules
aghandoff import config personal --settings ~/.claude/settings.json
```

Add `--apply` after reviewing each preview. Options can also be combined into one import; repeat `--skill` for several directories. A skill directory must contain `SKILL.md`; its supporting files and executable permissions are preserved, with access restricted to your OS user. Skill directory names use 1–64 lowercase letters, digits, hyphens, or underscores and start with a letter or digit. Claude's reserved `synced` and `anthropic-skills` names are excluded.

An instruction file is copied to the selected profile's `CLAUDE.md`. You can use a personal `AGENTS.md` this way, or select skills stored outside `~/.claude`:

```sh
aghandoff import config personal --instructions /path/to/personal/AGENTS.md --skill /path/to/local-skills/review
```

This changes neither the source filename nor its content. Relative `@imports` and references in an instruction file keep their original text, so review them if the destination directory changes their meaning. Additional referenced files are not followed or imported automatically.

Settings import retains only `model`, `language`, `outputStyle`, `alwaysThinkingEnabled`, and `effortLevel`. Other settings, including `env`, hooks, permissions, MCP configuration, endpoints, and plugin enablement, are excluded; values are not printed. Claude may already have written a profile's `settings.json`, in which case this import refuses to replace it. Configure those preferences through Claude or import into a new profile instead.

Repeat the import for each profile that should receive the same personal configuration. These are independent snapshots, not live synchronization. Updating the source later does not change an imported copy.

## Safety and limits

- No existing file is overwritten, including an identical file. Every destination is checked before any import file is installed. A conflict stops the whole selection; it does not silently skip files. Already registered session IDs are rejected.
- Imports hold the same account and workspace locks as managed sessions. Direct Claude launches do not use these locks; exit source sessions yourself.
- The explicitly selected source root can be a symlink, which supports dotfiles-managed instructions and skills. Symlinks inside a selected directory and special files are refused. Nested skill dependencies reached through symlinks must be made self-contained first.
- Known credential and secret filenames, including `.credentials.json`, `auth.json`, `.claude.json`, `.env`, `.npmrc`, private-key extensions, and `secrets/`, are refused. This is not a content scanner: instructions, transcripts, or supporting files can themselves contain sensitive text. Select only data the target profile may access.
- One import is limited to 512 MiB of source data, 10,000 files, 10,000 directories, and 32 nested directory levels. Transcript JSON lines are limited to 16 MiB. Main transcripts are checked for JSON objects and matching `sessionId` values when present; their internal message schema is not interpreted.
- Files are staged privately before installation. Interrupted staging may leave a private `.import-*` temporary directory. A crash after file installation and before session metadata is saved can leave unregistered files; the CLI reports metadata-save failures and retains copies for recovery. Do not delete or overwrite the originals to resolve this. There is no automatic rollback across process crashes or disk failures.
- Importing does not execute a skill, hook, or instruction. Claude may execute an imported skill's commands when you use it later; review skills from other authors first.

## Design references

[CC Switch's local skill import and copy/symlink modes](https://github.com/farion1231/cc-switch/blob/main/docs/user-manual/en/3-extensions/3.3-skills.md) informed explicit selection and keeping unmanaged files untouched. The [Agent Skills directory format](https://agentskills.io/specification) informed copying a whole selected skill with its supporting files. These are design references; no framework or upstream source code is bundled. The implementation uses aghandoff's existing Rust dependencies and Claude's native transcript and configuration formats.
