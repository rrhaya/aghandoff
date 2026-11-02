//! Explicit, local-only migration. Preview is the default; source bytes are never changed.
use super::*;
use std::{
    collections::BTreeSet,
    io::{BufRead, BufReader, Read},
};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_LINE: u64 = 16 * 1024 * 1024;

#[derive(Subcommand)]
pub(crate) enum ImportAction {
    /// Copy native UUID.jsonl transcripts and their matching UUID/ directories.
    Sessions {
        workspace: String,
        account: String,
        /// One Claude projects/<project> directory, not the entire configuration directory.
        #[arg(long)]
        from: PathBuf,
        /// Select IDs; omit to select all UUID.jsonl files in this directory.
        #[arg(long)]
        session: Vec<Uuid>,
        /// Also copy this project's memory/ directory.
        #[arg(long)]
        with_memory: bool,
        /// Save the previewed selection. Existing destinations are never overwritten.
        #[arg(long)]
        apply: bool,
    },
    /// Copy selected personal instructions, rules, skills, and basic preferences to a profile.
    Config {
        account: String,
        /// A CLAUDE.md, AGENTS.md, or other instruction file; installed as CLAUDE.md.
        #[arg(long)]
        instructions: Option<PathBuf>,
        /// A rules directory, installed as rules/.
        #[arg(long)]
        rules: Option<PathBuf>,
        /// A skill directory containing SKILL.md; repeat to select several skills.
        #[arg(long)]
        skill: Vec<PathBuf>,
        /// A settings.json; only model, language, outputStyle, alwaysThinkingEnabled,
        /// and effortLevel are imported. Hooks, env, permissions, MCP and plugins are excluded.
        #[arg(long)]
        settings: Option<PathBuf>,
        /// Save the previewed selection. Existing destinations are never overwritten.
        #[arg(long)]
        apply: bool,
    },
}

pub(crate) fn execute(store: &Store, registry: &Registry, action: ImportAction) -> Result<()> {
    match action {
        ImportAction::Sessions {
            workspace,
            account,
            from,
            session,
            with_memory,
            apply,
        } => {
            let profile = store.check_account(registry, &account)?;
            let (_, state) = store.check_workspace(registry, &workspace)?;
            let _account_lock = lock(&profile.join("active.lock"))?;
            let _workspace_lock = lock(&state.join("active.lock"))?;
            let source = source_root(store, &from, true)?;
            let ids = selected_sessions(&source, session)?;
            let history_path = state.join("sessions.json");
            let mut history: History = read_json(&history_path)?;
            let mut plan = Plan::new(store)?;
            let mut newest = None;
            for id in &ids {
                if history.sessions.contains_key(id) {
                    bail!("session {id} is already registered; no changes made");
                }
                let name = PathBuf::from(format!("{id}.jsonl"));
                let modified = plan.file(&source.join(&name), &name)?;
                validate_transcript(&plan.stage.path().join(&name), *id)?;
                let related = source.join(id.to_string());
                if source_exists(&related)? {
                    plan.tree(&related, Path::new(&id.to_string()), 0)?;
                }
                history.sessions.insert(
                    *id,
                    Session {
                        id: *id,
                        account: account.clone(),
                        updated_at: modified,
                        exit_code: None,
                        transcript_present: true,
                    },
                );
                if newest.is_none_or(|previous: (u64, Uuid)| (modified, *id) > previous) {
                    newest = Some((modified, *id));
                }
            }
            if with_memory {
                let memory = source.join("memory");
                if source_exists(&memory)? {
                    plan.tree(&memory, Path::new("memory"), 0)?;
                }
            }
            let target = state.join("transcripts");
            plan.preview(&target)?;
            let selected = newest.context("no sessions selected")?.1;
            if apply {
                let installed = plan.install(&target)?;
                if history.last.is_none() {
                    history.last = Some(selected);
                }
                // Files are installed first. A crash here may leave unregistered files,
                // never a registered transcript that was not copied yet.
                if write_json(&history_path, &history).is_err() {
                    installed.keep();
                    bail!(
                        "could not save session metadata; copied files were retained for recovery"
                    );
                }
                installed.keep();
                println!(
                    "Imported {} sessions. Source files were left unchanged.",
                    ids.len()
                );
                println!("Resume with: aghandoff resume {workspace} {account} {selected}");
            } else {
                println!(
                    "Preview only. Repeat with --apply to import {} sessions.",
                    ids.len()
                );
            }
        }
        ImportAction::Config {
            account,
            instructions,
            rules,
            skill,
            settings,
            apply,
        } => {
            let profile = store.check_account(registry, &account)?;
            let _account_lock = lock(&profile.join("active.lock"))?;
            let mut plan = Plan::new(store)?;
            if let Some(path) = instructions {
                let source = source_root(store, &path, false)?;
                plan.file(&source, Path::new("CLAUDE.md"))?;
            }
            if let Some(path) = rules {
                let source = source_root(store, &path, true)?;
                plan.tree(&source, Path::new("rules"), 0)?;
            }
            for path in skill {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("invalid skill name")?;
                valid_skill_name(name)?;
                if name == "synced" || name == "anthropic-skills" {
                    bail!("reserved Claude skill name");
                }
                let source = source_root(store, &path, true)?;
                if !regular_file(&source.join("SKILL.md"))? {
                    bail!("selected skill must contain a regular SKILL.md");
                }
                plan.tree(&source, &Path::new("skills").join(name), 0)?;
            }
            if let Some(path) = settings {
                let source = source_root(store, &path, false)?;
                let raw = Path::new("settings.json");
                plan.file(&source, raw)?;
                let staged = plan.stage.path().join(raw);
                let value: serde_json::Value = serde_json::from_slice(&fs::read(&staged)?)
                    .map_err(|_| anyhow::anyhow!("invalid settings JSON; no changes made"))?;
                let original = value
                    .as_object()
                    .context("settings must be a JSON object")?;
                let mut selected = serde_json::Map::new();
                for key in [
                    "model",
                    "language",
                    "outputStyle",
                    "alwaysThinkingEnabled",
                    "effortLevel",
                ] {
                    if let Some(value) = original.get(key) {
                        let valid = if key == "alwaysThinkingEnabled" {
                            value.is_boolean()
                        } else {
                            value.is_string()
                        };
                        if !valid {
                            bail!("unsupported type for preference {key}; no changes made");
                        }
                        selected.insert(key.to_owned(), value.clone());
                    }
                }
                println!("Settings: {} basic preferences selected; {} other entries excluded (values hidden).",
                    selected.len(), original.len() - selected.len());
                if selected.is_empty() {
                    bail!("no supported basic preferences in selected settings; no changes made");
                }
                // Truncate only our temporary copy; never change the source settings.
                let mut file = OpenOptions::new().write(true).truncate(true).open(staged)?;
                serde_json::to_writer_pretty(&mut file, &selected)?;
                file.write_all(b"\n")?;
                file.sync_all()?;
            }
            if plan.files.is_empty() {
                bail!("select --instructions, --rules, --skill, or --settings");
            }
            plan.preview(&profile)?;
            if apply {
                plan.install(&profile)?.keep();
                println!("Imported personal configuration into {account}. Source files were left unchanged.");
            } else {
                println!("Preview only. Repeat with --apply to import into {account}.");
            }
        }
    }
    Ok(())
}

fn source_root(store: &Store, path: &Path, directory: bool) -> Result<PathBuf> {
    allowed_name(path)?;
    // Explicitly selected roots may be symlinks (common for dotfiles). Descendant
    // links are refused, so an imported skill cannot silently reach outside itself.
    let source = fs::canonicalize(path).context("selected source does not exist")?;
    allowed_name(&source)?;
    if source.starts_with(&store.root) || (directory && store.root.starts_with(&source)) {
        bail!("source overlaps aghandoff storage; select an external file or directory");
    }
    let meta = fs::metadata(&source)?;
    if (directory && !meta.is_dir()) || (!directory && !meta.is_file()) {
        bail!("selected source has the wrong file type");
    }
    Ok(source)
}

fn allowed_name(path: &Path) -> Result<()> {
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            continue;
        };
        let name = component
            .to_str()
            .context("unsupported source name")?
            .to_ascii_lowercase();
        if [
            ".git",
            ".ssh",
            ".aws",
            ".azure",
            ".kube",
            "keychains",
            ".credentials.json",
            "credentials.json",
            "auth.json",
            ".claude.json",
            ".netrc",
            ".npmrc",
            ".git-credentials",
            "id_rsa",
            "id_ed25519",
            "secrets",
        ]
        .contains(&name.as_str())
            || name == ".env"
            || name.starts_with(".env.")
            || [".env", ".key", ".pem", ".p12", ".pfx"]
                .iter()
                .any(|ext| name.ends_with(ext))
        {
            bail!("credential or secret file names are excluded from imports");
        }
    }
    Ok(())
}

fn valid_skill_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        || !name.as_bytes()[0].is_ascii_alphanumeric()
    {
        bail!("skill directory name must start with a-z/0-9 and contain 1-64 lowercase letters, digits, - or _");
    }
    Ok(())
}

fn selected_sessions(source: &Path, requested: Vec<Uuid>) -> Result<BTreeSet<Uuid>> {
    let ids: BTreeSet<Uuid> = if requested.is_empty() {
        fs::read_dir(source)?
            .map(|entry| {
                let path = entry?.path();
                Ok(path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_suffix(".jsonl"))
                    .and_then(|s| Uuid::parse_str(s).ok().filter(|id| id.to_string() == s)))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect()
    } else {
        requested.into_iter().collect()
    };
    if ids.is_empty() {
        bail!("no UUID.jsonl transcripts found in selected project directory");
    }
    Ok(ids)
}

fn source_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn validate_transcript(path: &Path, id: Uuid) -> Result<()> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut buffer = Vec::new();
    let mut records = 0;
    loop {
        buffer.clear();
        let bytes = (&mut reader)
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut buffer)?;
        if bytes == 0 {
            break;
        }
        if bytes as u64 > MAX_LINE {
            bail!("transcript line exceeds the 16 MiB import limit");
        }
        if buffer.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let value: serde_json::Value = serde_json::from_slice(&buffer).map_err(|_| {
            anyhow::anyhow!("invalid transcript JSON; no changes made (contents hidden)")
        })?;
        if !value.is_object() {
            bail!("transcript records must be JSON objects; no changes made");
        }
        if let Some(actual) = value.get("sessionId") {
            if actual.as_str().and_then(|s| Uuid::parse_str(s).ok()) != Some(id) {
                bail!("transcript sessionId does not match its filename; no changes made");
            }
        }
        records += 1;
    }
    if records == 0 {
        bail!("transcript is empty; no changes made");
    }
    Ok(())
}

struct Plan {
    stage: tempfile::TempDir,
    files: BTreeSet<PathBuf>,
    directories: BTreeSet<PathBuf>,
    bytes: u64,
}

impl Plan {
    fn new(store: &Store) -> Result<Self> {
        Ok(Self {
            stage: tempfile::Builder::new()
                .prefix(".import-")
                .tempdir_in(&store.root)?,
            files: BTreeSet::new(),
            directories: BTreeSet::new(),
            bytes: 0,
        })
    }

    fn directory(&mut self, relative: &Path) -> Result<()> {
        if relative.as_os_str().is_empty() {
            return Ok(());
        }
        if self.files.contains(relative) {
            bail!("duplicate import destination");
        }
        if let Some(parent) = relative.parent() {
            self.directory(parent)?;
        }
        if self.directories.insert(relative.to_path_buf()) {
            if self.directories.len() > MAX_FILES {
                bail!("import exceeds the 10,000 directory limit");
            }
            private_dir(&self.stage.path().join(relative))?;
        }
        Ok(())
    }

    fn file(&mut self, source: &Path, relative: &Path) -> Result<u64> {
        allowed_name(source)?;
        if self.files.contains(relative) || self.directories.contains(relative) {
            bail!("duplicate import destination");
        }
        if self.files.len() >= MAX_FILES {
            bail!("import exceeds the 10,000 file limit");
        }
        if !regular_file(source)? {
            bail!("selected source file is missing");
        }
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(source)?;
        let before = input.metadata()?;
        if !before.is_file() || before.len() > MAX_BYTES - self.bytes {
            bail!("import exceeds the 512 MiB limit or contains a special file");
        }
        if let Some(parent) = relative.parent() {
            self.directory(parent)?;
        }
        let mode = if before.permissions().mode() & 0o111 != 0 {
            0o700
        } else {
            0o600
        };
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(self.stage.path().join(relative))?;
        let copied = std::io::copy(
            &mut (&mut input).take(MAX_BYTES - self.bytes + 1),
            &mut output,
        )?;
        let after = input.metadata()?;
        if copied != before.len()
            || before.len() != after.len()
            || before.modified()? != after.modified()?
        {
            bail!("source changed while being copied; exit the source Claude session and retry");
        }
        output.sync_all()?;
        self.bytes += copied;
        self.files.insert(relative.to_path_buf());
        Ok(before
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs())
    }

    fn tree(&mut self, source: &Path, relative: &Path, depth: usize) -> Result<()> {
        allowed_name(source)?;
        let meta = fs::symlink_metadata(source)?;
        if !meta.is_dir() || meta.file_type().is_symlink() || depth > 32 {
            bail!("nested symlinks, special files, and directory depths over 32 are unsupported");
        }
        self.directory(relative)?;
        let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let target = relative.join(entry.file_name());
            let meta = fs::symlink_metadata(&path)?;
            if meta.is_dir() && !meta.file_type().is_symlink() {
                self.tree(&path, &target, depth + 1)?;
            } else {
                self.file(&path, &target)?;
            }
        }
        Ok(())
    }

    fn check(&self, target: &Path) -> Result<()> {
        for relative in &self.directories {
            match fs::symlink_metadata(target.join(relative)) {
                // The checked target root is private. Native Claude may create
                // readable subdirectories inside it; those remain inaccessible
                // to other users through the private parent.
                Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
                Ok(_) => {
                    bail!("destination is not a real directory: {relative:?}")
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        for relative in &self.files {
            match fs::symlink_metadata(target.join(relative)) {
                Ok(_) => bail!("destination already exists: {relative:?}; nothing was overwritten"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    fn preview(&self, target: &Path) -> Result<()> {
        self.check(target)?;
        println!("Import destination: {target:?}");
        for relative in &self.files {
            println!("  + {relative:?}");
        }
        println!(
            "{} files selected ({} source bytes).",
            self.files.len(),
            self.bytes
        );
        Ok(())
    }

    fn install(&self, target: &Path) -> Result<Installed> {
        self.check(target)?;
        let mut installed = Installed {
            files: Vec::new(),
            directories: Vec::new(),
            keep: false,
        };
        for relative in &self.directories {
            let path = target.join(relative);
            if !source_exists(&path)? {
                DirBuilder::new().mode(0o700).create(&path)?;
                installed.directories.push(path);
            }
        }
        for relative in &self.files {
            let destination = target.join(relative);
            // Atomic no-clobber publication on the same filesystem. Source is
            // our private staging file, never a link to the original source.
            fs::hard_link(self.stage.path().join(relative), &destination)?;
            installed.files.push(destination);
        }
        for relative in &self.directories {
            File::open(target.join(relative))?.sync_all()?;
        }
        File::open(target)?.sync_all()?;
        Ok(installed)
    }
}

struct Installed {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    keep: bool,
}

impl Installed {
    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for Installed {
    fn drop(&mut self) {
        if !self.keep {
            // Roll back only files and empty directories created by this attempt.
            for path in self.files.iter().rev() {
                let _ = fs::remove_file(path);
            }
            for path in self.directories.iter().rev() {
                let _ = fs::remove_dir(path);
            }
        }
    }
}
