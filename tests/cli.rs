#![cfg(unix)]

use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::{Child, Command, Output},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;

struct Fixture { temp: TempDir, data: PathBuf, project: PathBuf, fake: PathBuf }

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        // A space exercises argv handling without invoking a shell.
        let fake = temp.path().join("fake claude");
        fs::write(&fake, include_str!("fixtures/claude.py")).unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o700)).unwrap();
        let data = temp.path().join("data");
        let project = temp.path().join("project with spaces");
        fs::create_dir(&project).unwrap();
        Self { temp, data, project, fake }
    }

    fn command(&self, args: &[&str]) -> Command {
        self.command_in(&self.data, args)
    }

    fn command_in(&self, data: &Path, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_aghandoff"));
        c.env_clear().env("HOME", self.temp.path()).env("PATH", "/usr/bin:/bin")
            .arg("--data-dir").arg(data).arg("--claude-bin").arg(&self.fake).args(args);
        c
    }

    fn ok(&self, args: &[&str]) -> Output {
        let out = self.command(args).output().unwrap();
        assert!(out.status.success(), "args={args:?}, stderr={}", String::from_utf8_lossy(&out.stderr));
        out
    }

    fn setup(&self) {
        for name in ["one", "two", "three"] {
            self.ok(&["account", "add", name]);
            self.ok(&["login", name]);
        }
        self.ok(&["workspace", "add", "demo", self.project.to_str().unwrap()]);
    }

    fn history(&self) -> Value {
        serde_json::from_slice(&fs::read(self.data.join("workspaces/demo/sessions.json")).unwrap()).unwrap()
    }
}

#[test]
fn conversation_and_memory_survive_three_profiles_without_copying_auth() {
    let f = Fixture::new();
    f.setup();
    f.ok(&["run", "demo", "one"]);
    let id = f.history()["last"].as_str().unwrap().to_owned();
    f.ok(&["continue", "demo", "two"]);
    f.ok(&["resume", "demo", "three", &id]);
    let transcript = f.data.join(format!("workspaces/demo/transcripts/{id}.jsonl"));
    let lines: Vec<Value> = fs::read_to_string(transcript).unwrap().lines()
        .map(|s| serde_json::from_str(s).unwrap()).collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["profile"], "one");
    assert_eq!(lines[1]["profile"], "two");
    assert_eq!(lines[2]["profile"], "three");
    assert_eq!(lines[2]["previous_turns"], 2);
    for name in ["one", "two", "three"] {
        let profile = f.data.join("accounts").join(name);
        assert_eq!(fs::read_to_string(profile.join("fake-login")).unwrap(), name);
        assert_eq!(fs::read_to_string(profile.join("projects/aghandoff-demo/memory/MEMORY.md")).unwrap(), "synthetic memory");
        assert!(fs::symlink_metadata(profile.join("projects/aghandoff-demo")).unwrap().file_type().is_symlink());
    }
    assert_eq!(f.history()["last"], id);
    assert_eq!(f.history()["sessions"][&id]["account"], "three");
}

#[test]
fn missing_history_and_failed_launch_never_start_a_replacement_conversation() {
    let f = Fixture::new();
    f.setup();
    f.ok(&["run", "demo", "one"]);
    let id = f.history()["last"].as_str().unwrap().to_owned();
    let out = f.command(&["continue", "demo", "two"]).env("FAKE_FAIL", "1").output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(f.history()["last"], id);
    // Failed new session cannot hide the preceding resumable one.
    let out = f.command(&["run", "demo", "two"]).env("FAKE_FAIL", "1").output().unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(f.history()["last"], id);
    fs::remove_file(f.data.join(format!("workspaces/demo/transcripts/{id}.jsonl"))).unwrap();
    let out = f.command(&["continue", "demo", "two"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("transcript is missing"));
    assert_eq!(f.history()["last"], id);
}

#[test]
fn refuses_override_without_printing_its_value() {
    let f = Fixture::new();
    f.setup();
    let out = f.command(&["run", "demo", "one"])
        .env("ANTHROPIC_API_KEY", "synthetic-secret-must-not-appear").output().unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ANTHROPIC_API_KEY"));
    assert!(!stderr.contains("synthetic-secret-must-not-appear"));
    assert!(!f.data.join("workspaces/demo/sessions.json").exists());
}

#[test]
fn refuses_wrong_auth_directory_api_credentials_and_old_cli() {
    let f = Fixture::new();
    f.setup();
    for (key, value, expected) in [
        ("FAKE_CONFIG_DIR", "/tmp", "different config directory"),
        ("FAKE_AUTH_METHOD", "api_key", "subscription login"),
        ("FAKE_VERSION", "2.1.233", ">= 2.1.268"),
        ("FAKE_BAD_AUTH_JSON", "1", "did not return JSON"),
        ("FAKE_AUTH_FAIL", "1", "authentication check failed"),
    ] {
        let out = f.command(&["run", "demo", "one"]).env(key, value).output().unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains(expected));
    }
    assert!(!f.data.join("workspaces/demo/sessions.json").exists());
}

#[test]
fn rejects_path_traversal_duplicate_workspace_and_unmanaged_data() {
    let f = Fixture::new();
    assert!(!f.command(&["account", "add", "../escape"]).output().unwrap().status.success());
    assert!(!f.temp.path().join("escape").exists());
    f.setup();
    assert!(!f.command(&["workspace", "add", "other", f.project.to_str().unwrap()])
        .output().unwrap().status.success());
    let unmanaged = f.temp.path().join("unmanaged");
    fs::create_dir(&unmanaged).unwrap();
    fs::set_permissions(&unmanaged, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(unmanaged.join("keep"), "synthetic").unwrap();
    let out = f.command_in(&unmanaged, &["account", "list"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not empty"));
    assert_eq!(fs::read_to_string(unmanaged.join("keep")).unwrap(), "synthetic");
    assert!(!unmanaged.join("accounts").exists());
}

#[test]
fn refuses_to_replace_existing_history_and_metadata_symlinks() {
    let f = Fixture::new();
    f.setup();
    let projects = f.data.join("accounts/one/projects");
    fs::create_dir(&projects).unwrap();
    fs::set_permissions(&projects, fs::Permissions::from_mode(0o700)).unwrap();
    let unrelated = f.temp.path().join("unrelated");
    fs::create_dir(&unrelated).unwrap();
    fs::write(unrelated.join("keep"), "synthetic").unwrap();
    symlink(&unrelated, projects.join("aghandoff-demo")).unwrap();
    assert!(!f.command(&["run", "demo", "one"]).output().unwrap().status.success());
    assert_eq!(fs::read_to_string(unrelated.join("keep")).unwrap(), "synthetic");
    let state = f.data.join("workspaces/demo/sessions.json");
    let other = f.temp.path().join("metadata");
    fs::write(&other, "do not read or replace").unwrap();
    symlink(&other, &state).unwrap();
    assert!(!f.command(&["sessions", "demo"]).output().unwrap().status.success());
    assert_eq!(fs::read_to_string(other).unwrap(), "do not read or replace");
}

#[test]
fn transcript_absence_is_not_reported_as_a_resumable_session() {
    let f = Fixture::new();
    f.setup();
    let out = f.command(&["run", "demo", "one"]).env("FAKE_NO_TRANSCRIPT", "1").output().unwrap();
    assert!(out.status.success());
    assert!(f.history()["last"].is_null());
    assert!(!f.command(&["continue", "demo", "two"]).output().unwrap().status.success());
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(Some(_))) {
            return;
        }
        // SAFETY: tests signal only their own child process.
        unsafe { libc::kill(self.0.id() as libc::pid_t, libc::SIGTERM); }
        let _ = self.0.wait();
    }
}

fn await_file(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(start.elapsed() < Duration::from_secs(10), "fake Claude did not start");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn blocks_workspace_and_account_overlap_and_releases_locks_on_termination() {
    let f = Fixture::new();
    f.setup();
    let ready = f.temp.path().join("ready");
    let mut child = ChildGuard(f.command(&["run", "demo", "one"])
        .env("FAKE_WAIT", &ready).spawn().unwrap());
    await_file(&ready);
    let out = f.command(&["run", "demo", "two"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("busy:"));
    assert!(!f.command(&["login", "one"]).output().unwrap().status.success());
    // SAFETY: this is our own child.
    unsafe { libc::kill(child.0.id() as libc::pid_t, libc::SIGTERM); }
    assert_eq!(child.0.wait().unwrap().code(), Some(143));
    f.ok(&["run", "demo", "two"]);
}

#[test]
fn metadata_is_private_and_corruption_is_not_treated_as_empty() {
    let f = Fixture::new();
    f.setup();
    assert_eq!(fs::metadata(&f.data).unwrap().permissions().mode() & 0o777, 0o700);
    assert_eq!(fs::metadata(f.data.join("registry.json")).unwrap().permissions().mode() & 0o777, 0o600);
    let path = f.data.join("registry.json");
    fs::write(&path, "invalid-json").unwrap();
    assert!(!f.command(&["account", "list"]).output().unwrap().status.success());
    assert_eq!(fs::read_to_string(path).unwrap(), "invalid-json");
}

#[test]
fn session_from_another_workspace_is_not_resumed() {
    let f = Fixture::new();
    f.setup();
    f.ok(&["run", "demo", "one"]);
    let id = f.history()["last"].as_str().unwrap().to_owned();
    let other = f.temp.path().join("other");
    fs::create_dir(&other).unwrap();
    f.ok(&["workspace", "add", "other", other.to_str().unwrap()]);
    let out = f.command(&["resume", "other", "two", &id]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not registered"));
}

#[test]
fn unregistered_and_unauthenticated_profiles_are_not_used() {
    let f = Fixture::new();
    f.ok(&["account", "add", "one"]);
    f.ok(&["workspace", "add", "demo", f.project.to_str().unwrap()]);
    assert!(!f.command(&["run", "demo", "missing"]).output().unwrap().status.success());
    assert!(!f.command(&["run", "demo", "one"]).output().unwrap().status.success());
    assert!(!f.data.join("workspaces/demo/sessions.json").exists());
}

#[test]
fn does_not_store_unknown_auth_status_fields() {
    let f = Fixture::new();
    f.setup();
    f.ok(&["status", "one"]);
    let registry: Value = serde_json::from_slice(&fs::read(f.data.join("registry.json")).unwrap()).unwrap();
    assert_eq!(registry, json!({"accounts": ["one", "two", "three"], "workspaces": {"demo": fs::canonicalize(&f.project).unwrap()}}));
}

#[test]
fn failed_login_propagates_exit_and_does_not_mark_profile_authenticated() {
    let f = Fixture::new();
    f.ok(&["account", "add", "one"]);
    let out = f.command(&["login", "one"]).env("FAKE_LOGIN_FAIL", "1").output().unwrap();
    assert_eq!(out.status.code(), Some(9));
    assert!(!f.data.join("accounts/one/fake-login").exists());
    assert!(!f.command(&["status", "one"]).output().unwrap().status.success());
}
