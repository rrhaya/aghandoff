#[cfg(not(unix))]
compile_error!("aghandoff 0.1 supports macOS and Linux only");

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// State directory; defaults to ~/.local/share/aghandoff. Never use ~/.claude.
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    /// Claude executable (also useful for offline integration tests).
    #[arg(long, global = true, default_value = "claude")]
    claude_bin: PathBuf,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Create or list isolated account profiles. Names are local labels, not identities.
    Account {
        #[command(subcommand)]
        command: AccountAction,
    },
    /// Register a directory whose conversations may be shared between your profiles.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceAction,
    },
    /// Open Claude's own subscription login flow for one profile.
    Login { account: String },
    /// Ask Claude to display this profile's authentication status.
    Status { account: String },
    /// Start a new conversation in a registered workspace.
    Run { workspace: String, account: String },
    /// Resume an aghandoff conversation with the selected profile after exiting Claude.
    Resume {
        workspace: String,
        account: String,
        session: Uuid,
    },
    /// Resume the last confirmed conversation in this workspace.
    Continue { workspace: String, account: String },
    /// List session IDs and last selected profiles; never reads conversation contents.
    Sessions { workspace: String },
}

#[derive(Subcommand)]
enum AccountAction {
    Add { name: String },
    List,
}

#[derive(Subcommand)]
enum WorkspaceAction {
    Add { name: String, path: PathBuf },
    List,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    accounts: Vec<String>,
    workspaces: BTreeMap<String, PathBuf>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    id: Uuid,
    account: String,
    updated_at: u64,
    exit_code: Option<i32>,
    transcript_present: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    last: Option<Uuid>,
    sessions: BTreeMap<Uuid, Session>,
}

struct Store {
    root: PathBuf,
}

fn valid_name(name: &str) -> Result<()> {
    // Also valid for CLAUDE_CODE_PROJECT_DIR_NAME. Lowercase avoids macOS aliases.
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        || !name.as_bytes()[0].is_ascii_alphanumeric()
    {
        bail!("name must start with a-z/0-9 and contain 1-48 lowercase letters, digits, - or _");
    }
    Ok(())
}

fn private_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                bail!(
                    "managed directory is not a real directory: {}",
                    path.display()
                );
            }
            if meta.permissions().mode() & 0o077 != 0 {
                bail!(
                    "managed directory must be private (mode 0700): {}",
                    path.display()
                );
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            DirBuilder::new().mode(0o700).create(path)?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => Ok(true),
        Ok(_) => bail!("expected regular file: {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn read_json<T: serde::de::DeserializeOwned + Default>(path: &Path) -> Result<T> {
    if !regular_file(path)? {
        return Ok(T::default());
    }
    serde_json::from_slice(&fs::read(path)?).context("invalid metadata; no changes made")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    regular_file(path)?;
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("missing parent")?)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    serde_json::to_writer_pretty(&mut temp, value)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

fn lock(path: &Path) -> Result<File> {
    regular_file(path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.try_lock_exclusive().with_context(|| {
        format!(
            "busy: {}; exit the running managed Claude first",
            path.display()
        )
    })?;
    Ok(file)
}

impl Store {
    fn open(path: PathBuf) -> Result<Self> {
        if path.file_name().is_none() {
            bail!("data directory cannot be a filesystem root");
        }
        // Only creates the app directory and missing parents; does not inspect ~/.claude.
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        private_dir(&path)?;
        let root = fs::canonicalize(path)?;
        let marker = root.join(".aghandoff-root");
        if !regular_file(&marker)? {
            if fs::read_dir(&root)?.next().is_some() {
                bail!("data directory is not empty and is not an aghandoff store; choose a new directory");
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&marker)?;
            file.write_all(b"aghandoff-store-v1\n")?;
            file.sync_all()?;
        } else if fs::read(&marker)? != b"aghandoff-store-v1\n" {
            bail!("unsupported aghandoff store version");
        }
        for name in ["accounts", "workspaces"] {
            private_dir(&root.join(name))?;
        }
        Ok(Self { root })
    }

    fn registry(&self) -> PathBuf {
        self.root.join("registry.json")
    }
    fn profile(&self, account: &str) -> PathBuf {
        self.root.join("accounts").join(account)
    }
    fn workspace(&self, name: &str) -> PathBuf {
        self.root.join("workspaces").join(name)
    }

    fn check_account(&self, registry: &Registry, account: &str) -> Result<PathBuf> {
        valid_name(account)?;
        if !registry.accounts.iter().any(|a| a == account) {
            bail!("unknown account profile: {account}");
        }
        let path = self.profile(account);
        private_dir(&path)?;
        Ok(path)
    }

    fn check_workspace(&self, registry: &Registry, name: &str) -> Result<(PathBuf, PathBuf)> {
        valid_name(name)?;
        let cwd = registry.workspaces.get(name).context("unknown workspace")?;
        if fs::canonicalize(cwd)? != *cwd {
            bail!("workspace path changed; refusing to launch");
        }
        let state = self.workspace(name);
        private_dir(&state)?;
        private_dir(&state.join("transcripts"))?;
        Ok((cwd.clone(), state))
    }

    fn link_history(&self, profile: &Path, workspace: &str, state: &Path) -> Result<()> {
        let projects = profile.join("projects");
        private_dir(&projects)?;
        let link = projects.join(project_name(workspace));
        let target = state.join("transcripts");
        match fs::symlink_metadata(&link) {
            Ok(meta) if meta.file_type().is_symlink() && fs::read_link(&link)? == target => (),
            Ok(_) => bail!(
                "history path already exists; refusing to replace: {}",
                link.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::os::unix::fs::symlink(target, link)?
            }
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
}

fn project_name(workspace: &str) -> String {
    format!("aghandoff-{workspace}")
}

fn check_environment() -> Result<()> {
    // Inspect names/presence only; never print credential values.
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE_DESCRIPTOR",
        "ANTHROPIC_BASE_URL",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_FOUNDRY",
        "CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST",
        "CLAUDE_SECURESTORAGE_CONFIG_DIR",
        "ANTHROPIC_PROFILE",
        "CLAUDE_CODE_PROJECT_DIR_NAME",
    ] {
        if env::var_os(key).is_some_and(|v| !v.is_empty()) {
            bail!(
                "{key} is set and may override profile isolation; launch from a shell without it"
            );
        }
    }
    Ok(())
}

fn claude(executable: &Path, profile: &Path) -> Result<Command> {
    check_environment()?;
    let mut command = Command::new(executable);
    command.env("CLAUDE_CONFIG_DIR", profile);
    Ok(command)
}

fn version_check(executable: &Path) -> Result<()> {
    let output = Command::new(executable)
        .arg("--version")
        .output()
        .context("cannot execute Claude Code; install it or set --claude-bin")?;
    if !output.status.success() {
        bail!("Claude --version failed");
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text
        .split_whitespace()
        .next()
        .context("missing Claude version")?;
    let parts: Vec<u64> = version
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .context("unrecognized Claude version (expected stable x.y.z)")?;
    if parts.len() != 3 || (parts[0], parts[1], parts[2]) < (2, 1, 268) {
        bail!("Claude Code >= 2.1.268 required for named project storage and configDirectory verification");
    }
    Ok(())
}

fn wait_child(command: &mut Command) -> Result<ExitStatus> {
    use signal_hook::{
        consts::{SIGHUP, SIGINT, SIGTERM},
        iterator::Signals,
    };
    let mut signals = Signals::new([SIGINT, SIGTERM, SIGHUP])?;
    let handle = signals.handle();
    let mut child = command.spawn().context("cannot start Claude Code")?;
    let pid = child.id() as libc::pid_t;
    let forward = std::thread::spawn(move || {
        for signal in signals.forever() {
            // SIGINT already reaches the foreground child via the terminal process group.
            // Keep the parent (and its locks) alive until Claude finishes.
            if signal != SIGINT {
                // SAFETY: child PID and signal number come from the OS.
                unsafe {
                    libc::kill(pid, signal);
                }
            }
        }
    });
    let result = child.wait();
    handle.close();
    let _ = forward.join();
    result.context("waiting for Claude failed")
}

fn code(status: ExitStatus) -> i32 {
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1))
}

fn verify_auth(executable: &Path, profile: &Path, cwd: &Path) -> Result<()> {
    // Read only the official CLI's status response. Never open credentials or Keychain.
    let output = claude(executable, profile)?
        .current_dir(cwd)
        .args(["auth", "status"])
        .output()
        .context("could not query Claude authentication status")?;
    if !output.status.success() {
        bail!("Claude authentication check failed; use login for this profile, then retry");
    }
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Claude auth status did not return JSON")?;
    if value.get("loggedIn").and_then(|v| v.as_bool()) != Some(true)
        || value.get("authMethod").and_then(|v| v.as_str()) != Some("claude.ai")
    {
        bail!("this release requires a Claude subscription login (claude.ai); API/Console/cloud credentials are unsupported");
    }
    let actual = value
        .get("configDirectory")
        .and_then(|v| v.as_str())
        .context("Claude did not report configDirectory; update Claude Code")?;
    if fs::canonicalize(actual)? != profile {
        bail!("Claude uses a different config directory; refusing an account-ambiguous launch");
    }
    Ok(())
}

fn auth(
    store: &Store,
    registry: &Registry,
    executable: &Path,
    account: &str,
    login: bool,
) -> Result<i32> {
    let profile = store.check_account(registry, account)?;
    let _lock = lock(&profile.join("active.lock"))?;
    check_environment()?;
    version_check(executable)?;
    if login {
        // Use a neutral managed cwd so project-specific settings cannot affect login.
        let mut command = claude(executable, &profile)?;
        command.current_dir(&profile).args(["auth", "login"]);
        let exit = code(wait_child(&mut command)?);
        if exit != 0 {
            return Ok(exit);
        }
    }
    verify_auth(executable, &profile, &profile)?;
    println!("{account}: authenticated Claude subscription; profile directory verified");
    Ok(0)
}

fn launch(
    store: &Store,
    registry: &Registry,
    executable: &Path,
    workspace: &str,
    account: &str,
    resume: Option<Uuid>,
    continue_last: bool,
) -> Result<i32> {
    let profile = store.check_account(registry, account)?;
    let (cwd, state) = store.check_workspace(registry, workspace)?;
    let _account_lock = lock(&profile.join("active.lock"))?;
    let _workspace_lock = lock(&state.join("active.lock"))?;
    check_environment()?;
    version_check(executable)?;
    verify_auth(executable, &profile, &cwd)?;
    let history_path = state.join("sessions.json");
    let mut history: History = read_json(&history_path)?;
    let resume = if continue_last {
        Some(
            history
                .last
                .context("no confirmed session; use run first")?,
        )
    } else {
        resume
    };
    let id = resume.unwrap_or_else(Uuid::new_v4);
    let transcript = state.join("transcripts").join(format!("{id}.jsonl"));
    if resume.is_some() {
        if !history.sessions.contains_key(&id) {
            bail!("session is not registered in this workspace");
        }
        if !regular_file(&transcript)? {
            bail!("saved transcript is missing; refusing to start a replacement conversation");
        }
    }
    store.link_history(&profile, workspace, &state)?;
    let mut command = claude(executable, &profile)?;
    command
        .current_dir(cwd)
        .env("CLAUDE_CODE_PROJECT_DIR_NAME", project_name(workspace));
    command
        .arg(if resume.is_some() {
            "--resume"
        } else {
            "--session-id"
        })
        .arg(id.to_string());
    eprintln!("aghandoff: workspace={workspace} profile={account} session={id}");
    if resume.is_some() {
        eprintln!("The saved conversation will be available to the selected account. Check /status before sending a message.");
    }
    // Persist the ID before launch so an interrupted run can be recovered explicitly.
    history.sessions.entry(id).or_insert(Session {
        id,
        account: account.to_owned(),
        updated_at: now(),
        exit_code: None,
        transcript_present: false,
    });
    write_json(&history_path, &history)?;
    let exit = code(wait_child(&mut command)?);
    let present = regular_file(&transcript)?;
    if !present {
        eprintln!("aghandoff: no saved transcript found for {id}; continue was not updated");
    }
    history.sessions.insert(
        id,
        Session {
            id,
            account: account.to_owned(),
            updated_at: now(),
            exit_code: Some(exit),
            transcript_present: present,
        },
    );
    // A failed resume must not move the last-successful pointer.
    if exit == 0 && present {
        history.last = Some(id);
    }
    write_json(&history_path, &history)?;
    Ok(exit)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn execute(cli: Cli) -> Result<i32> {
    let data_dir = match cli.data_dir {
        Some(path) => env::current_dir()?.join(path),
        None => PathBuf::from(env::var_os("HOME").context("HOME unset; pass --data-dir")?)
            .join(".local/share/aghandoff"),
    };
    let store = Store::open(data_dir)?;
    let registry_lock = lock(&store.root.join("registry.lock"))?;
    let mut registry: Registry = read_json(&store.registry())?;
    // Validate stored labels too: metadata must not turn paths into traversal.
    for account in &registry.accounts {
        valid_name(account)?;
    }
    for workspace in registry.workspaces.keys() {
        valid_name(workspace)?;
    }
    match cli.command {
        Action::Account {
            command: AccountAction::Add { name },
        } => {
            valid_name(&name)?;
            if registry.accounts.contains(&name) {
                bail!("account profile already exists");
            }
            private_dir(&store.profile(&name))?;
            registry.accounts.push(name.clone());
            write_json(&store.registry(), &registry)?;
            println!("Created {name}. Next: aghandoff login {name}");
        }
        Action::Account {
            command: AccountAction::List,
        } => {
            for account in &registry.accounts {
                println!("{account}");
            }
        }
        Action::Workspace {
            command: WorkspaceAction::Add { name, path },
        } => {
            valid_name(&name)?;
            if registry.workspaces.contains_key(&name) {
                bail!("workspace already exists");
            }
            let path = fs::canonicalize(path).context("workspace directory does not exist")?;
            if !path.is_dir() {
                bail!("workspace must be a directory");
            }
            if registry.workspaces.values().any(|p| p == &path) {
                bail!("directory already registered");
            }
            private_dir(&store.workspace(&name))?;
            private_dir(&store.workspace(&name).join("transcripts"))?;
            registry.workspaces.insert(name.clone(), path);
            write_json(&store.registry(), &registry)?;
            println!("Registered {name}. Conversations and auto memory in this workspace are shared by profiles used here.");
        }
        Action::Workspace {
            command: WorkspaceAction::List,
        } => {
            for (name, path) in &registry.workspaces {
                println!("{name}\t{}", path.display());
            }
        }
        Action::Login { account } => {
            drop(registry_lock);
            return auth(&store, &registry, &cli.claude_bin, &account, true);
        }
        Action::Status { account } => {
            drop(registry_lock);
            return auth(&store, &registry, &cli.claude_bin, &account, false);
        }
        Action::Run { workspace, account } => {
            drop(registry_lock);
            return launch(
                &store,
                &registry,
                &cli.claude_bin,
                &workspace,
                &account,
                None,
                false,
            );
        }
        Action::Resume {
            workspace,
            account,
            session,
        } => {
            drop(registry_lock);
            return launch(
                &store,
                &registry,
                &cli.claude_bin,
                &workspace,
                &account,
                Some(session),
                false,
            );
        }
        Action::Continue { workspace, account } => {
            drop(registry_lock);
            return launch(
                &store,
                &registry,
                &cli.claude_bin,
                &workspace,
                &account,
                None,
                true,
            );
        }
        Action::Sessions { workspace } => {
            let (_, state) = store.check_workspace(&registry, &workspace)?;
            let history: History = read_json(&state.join("sessions.json"))?;
            for session in history.sessions.values() {
                println!(
                    "{}\tprofile={}\texit={:?}\ttranscript={}\tupdated={}{}",
                    session.id,
                    session.account,
                    session.exit_code,
                    session.transcript_present,
                    session.updated_at,
                    if history.last == Some(session.id) {
                        "\tlast"
                    } else {
                        ""
                    }
                );
            }
        }
    }
    Ok(0)
}

fn main() {
    let cli = Cli::parse();
    let result = execute(cli);
    match result {
        Ok(exit) => std::process::exit(exit),
        Err(error) => {
            eprintln!("aghandoff: {error:#}");
            std::process::exit(1);
        }
    }
}
