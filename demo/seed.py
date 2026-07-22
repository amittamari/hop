#!/usr/bin/env python3
"""Seed a throwaway $HOME with synthetic Claude Code + Codex sessions.

The VHS demo records `hop` against this sandbox so the recording never touches
real session data and is fully reproducible. Every path `hop` reads (the agent
session dirs, the Tantivy cache, the config dir) derives from $HOME via the
`directories` crate, so isolating $HOME isolates everything.

It also creates a real git repo at <sandbox>/work/hop with an `origin` remote.
The demo `cd`s into it and runs bare `hop`, which auto-scopes to the current
repo (`repo:amittamari/hop`) — the default behavior. Sessions whose cwd is that
repo show up; the other-repo sessions are correctly filtered out.

Usage: seed.py <sandbox-home-dir>
"""
import json
import os
import shutil
import subprocess
import sys
import uuid
from datetime import datetime, timedelta, timezone

REPO_REMOTE = "https://github.com/amittamari/hop.git"


def rfc3339(dt: datetime) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def ago(**kw) -> datetime:
    return datetime.now(timezone.utc) - timedelta(**kw)


def write_lines(path: str, lines: list[dict]) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        for obj in lines:
            f.write(json.dumps(obj) + "\n")


def make_git_repo(path: str, remote: str) -> None:
    """A real repo (init + origin remote) so `git remote get-url origin`
    resolves — that's how hop derives the repo slug for auto-scoping and the
    REPO column. No commits needed."""
    os.makedirs(path, exist_ok=True)
    subprocess.run(["git", "init", "-q", "-b", "main", path], check=True)
    subprocess.run(["git", "-C", path, "remote", "add", "origin", remote], check=True)


# --- Claude sessions -------------------------------------------------------
# Layout: ~/.claude/projects/<encoded-cwd>/<session-uuid>.jsonl
# Title comes from `aiTitle`; repo column + repo: filter resolve from the cwd's
# git remote; branch comes from `gitBranch`.

def claude_session(root, cwd, branch, title, ts, turns):
    sid = str(uuid.uuid4())
    enc = cwd.replace("/", "-")
    path = os.path.join(root, ".claude", "projects", enc, f"{sid}.jsonl")
    common = {"cwd": cwd, "gitBranch": branch, "timestamp": rfc3339(ts)}
    lines = [{"type": "summary", "aiTitle": title, **common}]
    for role, text in turns:
        if role == "user":
            msg = {"role": "user", "content": text}
        else:
            msg = {"role": "assistant", "content": [{"type": "text", "text": text}]}
        lines.append({"type": role, "message": msg, **common})
    write_lines(path, lines)


# --- Codex sessions --------------------------------------------------------
# Layout: ~/.codex/sessions/<...>/rollout-<ts>-<uuid>.jsonl
# session_meta carries cwd + git.repository_url + git.branch directly; a
# turn_context with approval=never + danger-full-access marks the session yolo.

def codex_session(root, cwd, branch, repo_url, ts, turns, yolo=False):
    sid = str(uuid.uuid4())
    stamp = ts.strftime("%Y-%m-%dT%H-%M-%S")
    path = os.path.join(root, ".codex", "sessions", f"rollout-{stamp}-{sid}.jsonl")
    t = rfc3339(ts)
    lines = [{
        "type": "session_meta",
        "timestamp": t,
        "payload": {"cwd": cwd, "git": {"branch": branch, "repository_url": repo_url}},
    }]
    if yolo:
        lines.append({
            "type": "turn_context",
            "timestamp": t,
            "payload": {"approval_policy": "never",
                        "sandbox_policy": {"type": "danger-full-access"}},
        })
    for role, text in turns:
        sub = "user_message" if role == "user" else "agent_message"
        lines.append({"type": "event_msg", "timestamp": t,
                       "payload": {"type": sub, "message": text}})
    write_lines(path, lines)


def write_config(root):
    # macOS config dir for ProjectDirs::from("dev","hop","hop").
    cfg_dir = os.path.join(root, "Library", "Application Support", "dev.hop.hop")
    os.makedirs(cfg_dir, exist_ok=True)
    with open(os.path.join(cfg_dir, "config.toml"), "w") as f:
        # Disable background PR resolution so the demo stays deterministic and
        # makes no network/gh calls mid-recording.
        f.write("[columns]\ndisabled = [\"pr\"]\n")


def main():
    root = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else "demo/.demo-home")
    # Fresh sandbox every run.
    if os.path.isdir(root):
        shutil.rmtree(root)
    os.makedirs(root)
    # Cursor is a supported agent but we seed none; create the dir so it
    # registers as available (empty) rather than "unavailable" in the footer.
    os.makedirs(os.path.join(root, ".cursor", "projects"), exist_ok=True)

    # The repo the demo runs inside; `hop` auto-scopes to `repo:amittamari/hop`.
    repo = os.path.join(root, "work", "hop")
    make_git_repo(repo, REPO_REMOTE)

    # --- Sessions in the current repo (shown under default auto-scope) -------
    claude_session(
        root, repo, "feat/cursor-adapter",
        "Add a Cursor session adapter to the index", ago(hours=4),
        [
            ("user", "Wire up a Cursor adapter so its chats show up in the results grid alongside Claude and Codex."),
            ("assistant", "Added a CursorAdapter that scans .cursor/projects and recovers the working directory from worker.log. Branch stays blank since Cursor records none."),
            ("user", "Add a coverage row to the README support table too."),
        ],
    )
    claude_session(
        root, repo, "feat/preview-highlight",
        "Highlight query matches in the transcript preview", ago(hours=27),
        [
            ("user", "In the preview pane, highlight every occurrence of the search terms so I can scan a transcript fast."),
            ("assistant", "The preview now styles each match with a reverse-video span and exposes Ctrl+N / Ctrl+B to jump between matches in the preview."),
            ("user", "Make sure highlighting survives syntax-highlighted code blocks."),
            ("assistant", "Done — matches are layered on top of the code spans so both the syntax color and the match highlight render in the preview."),
        ],
    )
    codex_session(
        root, repo, "feat/pr-column", REPO_REMOTE, ago(days=2, hours=1),
        [
            ("user", "Resolve the GitHub PR status for each session in the background and cache it on disk."),
            ("assistant", "Added a background enrichment service that shells out to `gh` per branch and writes results to gh_pr.json. The PR column shows a spinner until it resolves."),
        ],
        yolo=True,
    )
    codex_session(
        root, repo, "feat/keymap-config", REPO_REMOTE, ago(hours=6),
        [
            ("user", "Make the Ctrl-chord keybindings configurable from config.toml, falling back to defaults on conflicts."),
            ("assistant", "Keybindings now load from a [keybindings] table; invalid or conflicting binds log to stderr and fall back. The help overlay reflects the active binds."),
        ],
    )
    claude_session(
        root, repo, "fix/preview-scroll",
        "Fix preview scroll jumping to top on resize", ago(minutes=35),
        [
            ("user", "When I resize the preview pane the scroll position snaps back to the top. It should stay put."),
            ("assistant", "The preview scroll offset was recomputed from scratch on resize. Clamped it against the new viewport height instead, so the preview keeps its position."),
        ],
    )

    # --- Other-repo sessions (auto-scope filters these out) ------------------
    codex_session(
        root, "/Users/dev/code/payments-api", "main",
        "https://github.com/acme/payments-api.git", ago(hours=3),
        [
            ("user", "Refactor the auth middleware to refresh tokens proactively before expiry."),
            ("assistant", "Added a proactive refresh scheduler keyed off the token's exp claim, with reactive 401 handling as a fallback."),
        ],
    )
    codex_session(
        root, "/Users/dev/code/web-dashboard", "feat/oauth",
        "https://github.com/acme/web-dashboard.git", ago(days=5),
        [
            ("user", "Add the OAuth login flow with PKCE and store tokens in the OS keychain."),
            ("assistant", "Implemented the authorization-code + PKCE flow; tokens live in the keychain and refresh silently on 401."),
        ],
    )

    write_config(root)
    print(f"seeded sandbox at {root} (repo: {repo})")


if __name__ == "__main__":
    main()
