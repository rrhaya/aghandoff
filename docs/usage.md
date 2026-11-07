# Account usage snapshots

aghandoff collects local observations from [Claude's documented status-line input](https://code.claude.com/docs/en/statusline#available-data). It does not read credentials, scrape `/usage`, call private endpoints, or refresh account tokens.

## Enable and inspect

```sh
aghandoff usage setup personal
aghandoff usage setup personal --apply
aghandoff usage setup second --apply
```

The first command previews a `statusLine` entry. `--apply` updates only that entry in the selected account profile's `settings.json`, preserving other settings. Existing custom status lines are refused rather than overwritten. Setup is idempotent for the same executable and data directory, and is blocked while a managed session uses the profile.

Restart managed Claude and send a message. When Claude invokes its status line, aghandoff saves only the observation time and the documented `five_hour` and `seven_day` percentages and reset timestamps. Account emails, transcript paths, costs, prompts, and unknown payload fields are not stored. Input is limited to 1 MiB; invalid payloads do not replace the last valid observation.

```sh
aghandoff usage list
aghandoff usage list --json
```

The list includes every registered profile. It reads local metadata and performs no network requests. JSON output includes `account`, `observed_at`, `age_seconds`, and the two windows. Each window has a `state`, `used_percentage`, and `resets_at`; times are Unix epoch seconds.

| State | Meaning |
| --- | --- |
| `unknown` | No observation, or Claude omitted this window. The percentage is null. |
| `fresh` | Observed within 5 minutes, with a reset time still in the future. |
| `stale` | Last observed more than 5 minutes ago. The value is historical. |
| `expired` | The reported reset time passed. A new response is needed. |
| `clock-skew` | The stored observation appears to be in the future. |

Missing windows replace old values with unknown. A passed reset time never implies a known 0% usage. Corrupt local metadata is an error, not an unknown observation. Raw payload and settings values are hidden in parse errors.

## Availability and attribution

Claude supplies rate-limit windows only for supported subscription sessions after a model response. Windows can be absent independently. Initial sessions and accounts without these fields show unknown; collection itself does not send a model request. See [Claude's availability notes](https://code.claude.com/docs/en/statusline#available-data).

Snapshots belong to local profile labels, not verified account identities. The callback checks that `CLAUDE_CONFIG_DIR` resolves to the selected profile, preventing accidental writes from a different profile. It uses a separate usage lock so it can run while the managed session holds its account lock. Login attempts clear the old observation because the same label may be used for another account.

An account can be active elsewhere after the last observation. Check `/status` and `/usage` inside Claude for the current account and limits. These snapshots do not authorize automatic account rotation, predict capacity, or represent a billing balance.

## Disable and customize

```sh
aghandoff usage disable personal
aghandoff usage disable personal --apply
```

This removes only the exact status-line command generated for the current executable and store. It leaves other settings and saved observations in place. If a custom status line replaced the generated entry, disable refuses to remove it. If you move the aghandoff executable, update the old command through Claude's settings before setup; use a stable installation path such as Cargo's binary directory.

The generated command occupies the profile's status line and displays the profile label and observed percentages. Existing status-line commands are not automatically chained. Project, local, or managed settings can override the user setting; if no snapshots appear, inspect the active status line through Claude and confirm it invokes the command setup previewed.

Storage is `~/.local/share/aghandoff/usage/<profile>.json`, or the corresponding directory under `--data-dir`. The directory is private and JSON snapshots use mode 0600. It is separate from Claude's configuration and authentication storage. There is no background service or telemetry.
