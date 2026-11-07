//! Receive only Claude's documented status-line counters; never query credential stores.
use super::*;
use serde_json::{json, Value};
use std::io::Read;

const MAX_INPUT: u64 = 1024 * 1024;
const FRESH_SECONDS: u64 = 300;

pub(crate) fn invalidate(store: &Store, account: &str) -> Result<()> {
    let _lock = lock(&store.root.join("usage").join(format!("{account}.lock")))?;
    let path = store.usage(account);
    if regular_file(&path)? {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[derive(Subcommand)]
pub(crate) enum UsageAction {
    /// Preview a status-line command for one profile; --apply enables collection.
    Setup {
        account: String,
        #[arg(long)]
        apply: bool,
    },
    /// Preview removal of aghandoff's own status line; --apply disables collection.
    Disable {
        account: String,
        #[arg(long)]
        apply: bool,
    },
    /// Display the latest local observations; this does not poll Claude's servers.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Internal status-line callback. Reads JSON on stdin in the selected profile.
    #[command(hide = true)]
    Record { account: String },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Window {
    used_percentage: f64,
    resets_at: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    observed_at: u64,
    five_hour: Option<Window>,
    seven_day: Option<Window>,
}

pub(crate) fn execute(store: &Store, registry: &Registry, action: UsageAction) -> Result<()> {
    match action {
        UsageAction::Setup { account, apply } => configure(store, registry, &account, apply, false),
        UsageAction::Disable { account, apply } => {
            configure(store, registry, &account, apply, true)
        }
        UsageAction::Record { account } => record(store, registry, &account),
        UsageAction::List { json: as_json } => list(store, registry, as_json),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn status_line(store: &Store, account: &str) -> Result<Value> {
    let executable = env::current_exe()?;
    let executable = executable
        .to_str()
        .context("executable path must be UTF-8")?;
    let root = store
        .root
        .to_str()
        .context("data directory must be UTF-8")?;
    Ok(json!({
        "type": "command",
        "command": format!("{} --data-dir {} usage record {}",
            shell_quote(executable), shell_quote(root), shell_quote(account))
    }))
}

fn configure(
    store: &Store,
    registry: &Registry,
    account: &str,
    apply: bool,
    disable: bool,
) -> Result<()> {
    let profile = store.check_account(registry, account)?;
    let _lock = lock(&profile.join("active.lock"))?;
    let path = profile.join("settings.json");
    let mut settings: Value = if regular_file(&path)? {
        serde_json::from_slice(&fs::read(&path)?)
            .map_err(|_| anyhow::anyhow!("invalid profile settings JSON; no changes made"))?
    } else {
        json!({})
    };
    let object = settings
        .as_object_mut()
        .context("profile settings must be a JSON object")?;
    let expected = status_line(store, account)?;
    let current = object.get("statusLine").filter(|v| !v.is_null());
    if current.is_some_and(|value| value != &expected) {
        bail!("profile already has a different status line; existing settings were left unchanged");
    }
    if disable && current.is_none() {
        println!("{account}: aghandoff usage collection is already disabled");
        return Ok(());
    }
    if !disable && current == Some(&expected) {
        println!("{account}: aghandoff usage collection is already configured");
        return Ok(());
    }
    if disable {
        if apply {
            object.remove("statusLine");
        }
        println!("{account}: remove aghandoff's statusLine entry; retain all other settings and snapshots");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({"statusLine": expected}))?
        );
        if apply {
            object.insert("statusLine".to_owned(), expected);
        }
    }
    if apply {
        write_json(&path, &settings)?;
        println!(
            "{account}: usage collection {}; restart managed Claude to apply",
            if disable { "disabled" } else { "enabled" }
        );
    } else {
        println!("Preview only. Repeat with --apply to change this profile's status line.");
    }
    Ok(())
}

fn parse_window(value: Option<&Value>) -> Result<Option<Window>> {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let object = value
        .as_object()
        .context("invalid rate limit window (values hidden)")?;
    let (Some(used), Some(reset)) = (object.get("used_percentage"), object.get("resets_at")) else {
        return Ok(None);
    };
    if used.is_null() || reset.is_null() {
        return Ok(None);
    }
    let used = used
        .as_f64()
        .filter(|p| p.is_finite() && (0.0..=100.0).contains(p))
        .context("invalid rate limit percentage (values hidden)")?;
    let reset = reset
        .as_u64()
        .context("invalid rate limit reset time (values hidden)")?;
    Ok(Some(Window {
        used_percentage: used,
        resets_at: reset,
    }))
}

fn record(store: &Store, registry: &Registry, account: &str) -> Result<()> {
    let profile = store.check_account(registry, account)?;
    let actual = env::var_os("CLAUDE_CONFIG_DIR").context("callback requires CLAUDE_CONFIG_DIR")?;
    if fs::canonicalize(actual)? != profile {
        bail!("callback profile does not match CLAUDE_CONFIG_DIR; no snapshot saved");
    }
    let mut input = Vec::new();
    std::io::stdin()
        .take(MAX_INPUT + 1)
        .read_to_end(&mut input)?;
    if input.len() as u64 > MAX_INPUT {
        bail!("status-line input exceeds 1 MiB; no snapshot saved");
    }
    let value: Value = serde_json::from_slice(&input).map_err(|_| {
        anyhow::anyhow!("invalid status-line JSON; no snapshot saved (contents hidden)")
    })?;
    let object = value
        .as_object()
        .context("status-line input must be a JSON object")?;
    let limits = object.get("rate_limits").filter(|v| !v.is_null());
    if limits.is_some_and(|v| !v.is_object()) {
        bail!("invalid rate_limits object (values hidden)");
    }
    let snapshot = Snapshot {
        observed_at: now(),
        five_hour: parse_window(limits.and_then(|v| v.get("five_hour")))?,
        seven_day: parse_window(limits.and_then(|v| v.get("seven_day")))?,
    };
    // A separate lock lets the callback run while the managed foreground session
    // holds active.lock. Atomic writes let readers observe a complete snapshot.
    let _lock = lock(&store.root.join("usage").join(format!("{account}.lock")))?;
    write_json(&store.usage(account), &snapshot)?;
    println!(
        "{account} | 5h {} | 7d {}",
        window_text(&window_view(
            snapshot.five_hour.as_ref(),
            snapshot.observed_at,
            now()
        )),
        window_text(&window_view(
            snapshot.seven_day.as_ref(),
            snapshot.observed_at,
            now()
        ))
    );
    Ok(())
}

fn window_view(window: Option<&Window>, observed: u64, current: u64) -> Value {
    let Some(window) = window else {
        return json!({"state": "unknown", "used_percentage": null, "resets_at": null});
    };
    let state = if observed > current {
        "clock-skew"
    } else if window.resets_at <= current {
        "expired"
    } else if current - observed > FRESH_SECONDS {
        "stale"
    } else {
        "fresh"
    };
    json!({"state":state, "used_percentage":window.used_percentage, "resets_at":window.resets_at})
}

fn window_text(window: &Value) -> String {
    match window["used_percentage"].as_f64() {
        Some(used) => format!(
            "{used:.1}% ({})",
            window["state"].as_str().unwrap_or("unknown")
        ),
        None => "unknown".to_owned(),
    }
}

fn list(store: &Store, registry: &Registry, as_json: bool) -> Result<()> {
    let current = now();
    let mut rows = Vec::new();
    for account in &registry.accounts {
        store.check_account(registry, account)?;
        let path = store.usage(account);
        let snapshot: Option<Snapshot> = if regular_file(&path)? {
            let snapshot: Snapshot = serde_json::from_slice(&fs::read(path)?).map_err(|_| {
                anyhow::anyhow!("invalid usage metadata for {account}; contents hidden")
            })?;
            for window in [&snapshot.five_hour, &snapshot.seven_day]
                .into_iter()
                .flatten()
            {
                if !window.used_percentage.is_finite()
                    || !(0.0..=100.0).contains(&window.used_percentage)
                {
                    bail!("invalid stored percentage for {account}");
                }
            }
            Some(snapshot)
        } else {
            None
        };
        let observed = snapshot.as_ref().map(|s| s.observed_at);
        rows.push(json!({
            "account": account,
            "observed_at": observed,
            "age_seconds": observed.and_then(|time| current.checked_sub(time)),
            "five_hour": window_view(snapshot.as_ref().and_then(|s| s.five_hour.as_ref()), observed.unwrap_or(0), current),
            "seven_day": window_view(snapshot.as_ref().and_then(|s| s.seven_day.as_ref()), observed.unwrap_or(0), current)
        }));
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!("No profiles. Create one with aghandoff account add <name>.");
    } else {
        for row in rows {
            let age = row["age_seconds"]
                .as_u64()
                .map_or_else(|| "unknown".to_owned(), |s| format!("{s}s"));
            println!(
                "{}\t5h={}\t7d={}\tage={age}\t5h-reset={}\t7d-reset={}",
                row["account"].as_str().unwrap_or("unknown"),
                window_text(&row["five_hour"]),
                window_text(&row["seven_day"]),
                row["five_hour"]["resets_at"],
                row["seven_day"]["resets_at"]
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_age_reset_and_clock_skew_have_distinct_states() {
        let window = Window {
            used_percentage: 25.0,
            resets_at: 2000,
        };
        assert_eq!(window_view(Some(&window), 1000, 1300)["state"], "fresh");
        assert_eq!(window_view(Some(&window), 1000, 1301)["state"], "stale");
        assert_eq!(window_view(Some(&window), 1000, 2000)["state"], "expired");
        assert_eq!(
            window_view(Some(&window), 2000, 1999)["state"],
            "clock-skew"
        );
        assert_eq!(window_view(None, 1000, 1000)["state"], "unknown");
    }

    #[test]
    fn absent_windows_stay_unknown_and_invalid_numbers_are_rejected() {
        for value in [
            Value::Null,
            json!({}),
            json!({"used_percentage":null,"resets_at":10}),
        ] {
            assert!(parse_window(Some(&value)).unwrap().is_none());
        }
        for value in [
            json!({"used_percentage":-1,"resets_at":10}),
            json!({"used_percentage":10,"resets_at":-1}),
        ] {
            assert!(parse_window(Some(&value)).is_err());
        }
    }
}
