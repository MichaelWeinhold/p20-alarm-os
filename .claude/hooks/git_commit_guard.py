#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

SEPARATORS = frozenset({"&&", "||", ";", "|", "\n"})
VALUED_GLOBALS = frozenset(
    {"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--exec-path", "--super-prefix"}
)
PROTECTED = frozenset({"main", "production"})
CONFORMING = re.compile(r"(feat|fix|chore)/[A-Za-z0-9][A-Za-z0-9._-]*")
GIT = frozenset({"git", "git.exe"})
SWITCHES = frozenset({"checkout", "switch"})
NEW_BRANCH = frozenset({"-b", "-B", "-c", "-C"})

INTENT_DIRECTORY = "branch-intent"
INTENT_MAX_AGE = 14 * 24 * 60 * 60
SESSION = re.compile(r"[A-Za-z0-9._-]{1,128}")
RELEVANT = re.compile(r"commit|checkout|switch")

PREFIXED = re.compile(r"^(Feat|Fix|Chore)\(#\d+\): \S")
CLAIMS_PREFIX = re.compile(r"^[A-Za-z]+\s*[(:]")
MESSAGE_FLAG = re.compile(r"^-[A-Za-z]*m(.*)$")

FORBIDDEN = (
    ("co-authored-by", "a Co-Authored-By trailer"),
    ("generated with", 'a "generated with" footer'),
    ("claude-session", "a session trailer"),
    ("claude.ai/", "a session link"),
)

USAGE = (
    "Feat(#n): …, Fix(#n): … or Chore(#n): … on one line, "
    "or a plain one-liner with no prefix for an incidental commit. "
    "See the Definition of Done."
)


def segments(command: str) -> list[list[str]]:
    try:
        words = shlex.split(command, comments=False, posix=True)
    except ValueError:
        return []
    found: list[list[str]] = []
    current: list[str] = []
    for word in words:
        if word in SEPARATORS:
            found.append(current)
            current = []
        else:
            current.append(word)
    found.append(current)
    return [segment for segment in found if segment]


def subcommand(segment: list[str]) -> tuple[str, list[str]] | None:
    if not segment or Path(segment[0]).name not in GIT:
        return None
    index = 1
    while index < len(segment):
        word = segment[index]
        if word in VALUED_GLOBALS:
            index += 2
            continue
        if word.startswith("-"):
            index += 1
            continue
        return word, segment[index + 1 :]
    return None


def commit_arguments(segment: list[str]) -> list[str] | None:
    found = subcommand(segment)
    if found is None:
        return None
    name, arguments = found
    return arguments if name == "commit" else None


def switch_target(segment: list[str], cwd: str) -> str | None:
    found = subcommand(segment)
    if found is None:
        return None
    name, arguments = found
    if name not in SWITCHES or "--" in arguments:
        return None
    index = 0
    while index < len(arguments):
        word = arguments[index]
        if word in NEW_BRANCH:
            return arguments[index + 1] if index + 1 < len(arguments) else None
        if word.startswith("-"):
            index += 1
            continue
        return word if branch_exists(word, cwd) else None
    return None


def messages(arguments: list[str]) -> list[str]:
    found: list[str] = []
    index = 0
    while index < len(arguments):
        word = arguments[index]
        if word in {"--message", "-m"}:
            if index + 1 < len(arguments):
                found.append(arguments[index + 1])
            index += 2
            continue
        if word.startswith("--message="):
            found.append(word.removeprefix("--message="))
            index += 1
            continue
        inline = MESSAGE_FLAG.match(word) if not word.startswith("--") else None
        if inline is not None:
            if inline.group(1):
                found.append(inline.group(1))
            elif index + 1 < len(arguments):
                found.append(arguments[index + 1])
                index += 1
        index += 1
    return found


def message_problem(message: str) -> str | None:
    if "\n" in message.strip():
        return (
            "the message has more than one line, and a commit message here is the subject line only"
        )
    lowered = message.lower()
    for needle, name in FORBIDDEN:
        if needle in lowered:
            return f"the message carries {name}, and no AI is an author or co-author here"
    if CLAIMS_PREFIX.match(message) and not PREFIXED.match(message):
        return f"the prefix is malformed: {USAGE}"
    return None


def git_output(arguments: list[str], cwd: str) -> str | None:
    try:
        result = subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def current_branch(cwd: str) -> str | None:
    return git_output(["branch", "--show-current"], cwd) or None


def branch_exists(branch: str, cwd: str) -> bool:
    return git_output(["rev-parse", "--verify", "--quiet", f"refs/heads/{branch}"], cwd) is not None


def intent_file(cwd: str, session: str | None) -> Path | None:
    if session is None or SESSION.fullmatch(session) is None:
        return None
    common = git_output(["rev-parse", "--git-common-dir"], cwd)
    if not common:
        return None
    return Path(cwd) / common / INTENT_DIRECTORY / f"{session}.txt"


def prune(directory: Path) -> None:
    cutoff = time.time() - INTENT_MAX_AGE
    for record in directory.glob("*.txt"):
        try:
            if record.stat().st_mtime < cutoff:
                record.unlink(missing_ok=True)
        except OSError:
            continue


def write_intent(path: Path, branch: str) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        prune(path.parent)
        path.write_text(f"{branch}\n", encoding="utf-8")
    except OSError:
        return


def read_intent(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8").strip() or None
    except OSError:
        return None


def record_intent(command: str, cwd: str, session: str | None) -> None:
    path = intent_file(cwd, session)
    if path is None:
        return
    for segment in segments(command):
        branch = switch_target(segment, cwd)
        if branch is not None:
            write_intent(path, branch)


def naming_problem(branch: str | None) -> str | None:
    if branch is None:
        return None
    if CONFORMING.fullmatch(branch) is not None:
        return None
    return (
        f"Refusing this commit: {branch} is not a feature branch. Rename it with "
        "git branch -m <type>/<shortname> — feat/ for a capability, fix/ for a bug, "
        "chore/ for the rest — and commit again."
    )


def intent_problem(cwd: str, session: str | None, branch: str | None) -> str | None:
    path = intent_file(cwd, session)
    if path is None:
        return None
    intended = read_intent(path)
    if intended is None or intended == branch:
        return None
    return (
        f"Refusing this commit: this session switched to {intended}, but HEAD is on "
        f"{branch or 'a detached HEAD'} — another session has probably moved this working tree. "
        f"Check out {intended} again, or delete {path} to drop the record."
    )


def violation(command: str, cwd: str, session: str | None) -> str | None:
    for segment in segments(command):
        arguments = commit_arguments(segment)
        if arguments is None:
            continue
        branch = current_branch(cwd)
        if branch in PROTECTED:
            return (
                f"Refusing to commit on {branch}. Every change goes on a feature branch "
                "cut from main — feat/<shortname>, fix/<shortname> or chore/<shortname>."
            )
        unnamed = naming_problem(branch)
        if unnamed is not None:
            return unnamed
        stray = intent_problem(cwd, session, branch)
        if stray is not None:
            return stray
        written = messages(arguments)
        if len(written) > 1:
            return f"Refusing this commit: several -m arguments make a message body. {USAGE}"
        for message in written:
            problem = message_problem(message)
            if problem is not None:
                return f"Refusing this commit: {problem}"
    return None


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except ValueError:
        return 0
    if not isinstance(payload, dict) or payload.get("tool_name") != "Bash":
        return 0
    tool_input = payload.get("tool_input")
    command = tool_input.get("command") if isinstance(tool_input, dict) else None
    if not isinstance(command, str) or RELEVANT.search(command) is None:
        return 0
    cwd = payload.get("cwd")
    directory = cwd if isinstance(cwd, str) else "."
    identifier = payload.get("session_id")
    session = identifier if isinstance(identifier, str) else None
    problem = violation(command, directory, session)
    if problem is not None:
        print(problem, file=sys.stderr)
        return 2
    record_intent(command, directory, session)
    return 0


if __name__ == "__main__":
    sys.exit(main())
