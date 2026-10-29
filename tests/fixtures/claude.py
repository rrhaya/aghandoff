#!/usr/bin/python3
"""Offline fake: synthetic profiles and transcripts only; never opens real CLI state."""
import json
import os
from pathlib import Path
import sys
import time

args = sys.argv[1:]
if args == ["--version"]:
    print(os.environ.get("FAKE_VERSION", "2.1.268") + " (Claude Code)")
    sys.exit(0)

profile = Path(os.environ["CLAUDE_CONFIG_DIR"])
if args == ["auth", "login"]:
    if os.environ.get("FAKE_LOGIN_FAIL"):
        sys.exit(9)
    (profile / "fake-login").write_text(profile.name)
    sys.exit(0)
if args == ["auth", "status"]:
    if os.environ.get("FAKE_AUTH_FAIL"):
        sys.exit(2)
    if os.environ.get("FAKE_BAD_AUTH_JSON"):
        print("not-json")
        sys.exit(0)
    logged_in = (profile / "fake-login").exists()
    print(json.dumps({
        "loggedIn": logged_in,
        "authMethod": os.environ.get("FAKE_AUTH_METHOD", "claude.ai"),
        "configDirectory": os.environ.get("FAKE_CONFIG_DIR", str(profile)),
        "email": "synthetic@example.invalid",
        "unknownFutureField": "synthetic-value-not-for-storage",
    }))
    sys.exit(0 if logged_in else 1)

if os.environ.get("FAKE_FAIL"):
    sys.exit(7)
if os.environ.get("FAKE_NO_TRANSCRIPT"):
    sys.exit(0)

assert len(args) == 2 and args[0] in ("--session-id", "--resume"), args
storage = profile / "projects" / os.environ["CLAUDE_CODE_PROJECT_DIR_NAME"]
assert storage.is_symlink(), storage
transcript = storage / (args[1] + ".jsonl")
if args[0] == "--resume":
    assert transcript.is_file(), "must not create a missing resumed transcript"
previous = transcript.read_text().splitlines() if transcript.exists() else []
with transcript.open("a") as out:
    out.write(json.dumps({"profile": profile.name, "previous_turns": len(previous),
                          "message": "synthetic context"}) + "\n")
memory = storage / "memory"
memory.mkdir(exist_ok=True)
(memory / "MEMORY.md").write_text("synthetic memory")

if os.environ.get("FAKE_WAIT"):
    Path(os.environ["FAKE_WAIT"]).write_text("ready")
    while True:
        time.sleep(1)
