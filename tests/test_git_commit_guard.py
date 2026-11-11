from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from pathlib import Path

import pytest

GUARD = Path(__file__).resolve().parents[1] / ".claude" / "hooks" / "git_commit_guard.py"

WELL_FORMED = 'git commit -m "Chore(#45): enforce the git rules with settings and hooks"'


SESSION = "11111111-2222-3333-4444-555555555555"
PEER = "99999999-8888-7777-6666-555555555555"


def guard(
    command: str,
    cwd: Path,
    tool: str = "Bash",
    session: str | None = None,
) -> subprocess.CompletedProcess[str]:
    payload: dict[str, object] = {
        "tool_name": tool,
        "cwd": str(cwd),
        "tool_input": {"command": command},
    }
    if session is not None:
        payload["session_id"] = session
    return subprocess.run(
        [sys.executable, str(GUARD)],
        input=json.dumps(payload),
        capture_output=True,
        text=True,
        check=False,
    )


def repository(path: Path, branch: str) -> Path:
    subprocess.run(["git", "init", "--quiet", "--initial-branch", branch, str(path)], check=True)
    return path


def git(arguments: list[str], cwd: Path) -> None:
    subprocess.run(["git", *arguments], cwd=cwd, check=True, capture_output=True)


def founding_commit(path: Path) -> None:
    git(
        [
            "-c",
            "user.email=tests@example.invalid",
            "-c",
            "user.name=Tests",
            "commit",
            "--allow-empty",
            "--quiet",
            "-m",
            "root",
        ],
        path,
    )


def switched(path: Path, branch: str, session: str = SESSION) -> None:
    assert guard(f"git checkout -b {branch}", path, session=session).returncode == 0
    git(["checkout", "--quiet", "-b", branch], path)


def record(path: Path, session: str = SESSION) -> Path:
    return path / ".git" / "branch-intent" / f"{session}.txt"


@pytest.fixture
def feature_branch(tmp_path: Path) -> Path:
    return repository(tmp_path, "chore/agent-enforcement")


@pytest.fixture
def committed(tmp_path: Path) -> Path:
    path = repository(tmp_path / "checkout", "chore/agent-enforcement")
    founding_commit(path)
    return path


def test_a_well_formed_commit_passes(feature_branch: Path) -> None:
    assert guard(WELL_FORMED, feature_branch).returncode == 0


@pytest.mark.parametrize(
    "message",
    [
        "tidy up trailing whitespace",
        "Chore(#45): a chore",
        "Feat(#1): a feature",
        "Fix(#123): a fix",
    ],
)
def test_accepted_messages(feature_branch: Path, message: str) -> None:
    assert guard(f'git commit -m "{message}"', feature_branch).returncode == 0


@pytest.mark.parametrize(
    "message",
    [
        "feat(#45): a lowercase prefix",
        "Feat: no issue number",
        "Feat(45): no hash",
        "Chore(#45):no space",
        "Refactor(#45): not one of the three",
    ],
)
def test_a_malformed_prefix_is_blocked(feature_branch: Path, message: str) -> None:
    result = guard(f'git commit -m "{message}"', feature_branch)
    assert result.returncode == 2
    assert "prefix is malformed" in result.stderr


@pytest.mark.parametrize(
    ("command", "complaint"),
    [
        pytest.param(
            "git commit -m 'Chore(#45): a chore\n\nand a body'",
            "more than one line",
            id="a multi-line message",
        ),
        pytest.param(
            'git commit -m "Chore(#45): a chore" -m "and a body"',
            "several -m arguments",
            id="a second message argument",
        ),
    ],
)
def test_a_message_with_a_body_is_blocked(
    feature_branch: Path, command: str, complaint: str
) -> None:
    result = guard(command, feature_branch)
    assert result.returncode == 2
    assert complaint in result.stderr


@pytest.mark.parametrize(
    "trailer",
    [
        "Co-Authored-By: Claude <noreply@anthropic.com>",
        "Generated with Claude Code",
        "Claude-Session: abc123",
        "see https://claude.ai/chat/abc123",
    ],
)
def test_an_attribution_trailer_is_blocked(feature_branch: Path, trailer: str) -> None:
    result = guard(f"git commit -m 'Chore(#45): a chore — {trailer}'", feature_branch)
    assert result.returncode == 2
    assert "author or co-author" in result.stderr


@pytest.mark.parametrize("branch", ["main", "production"])
def test_a_commit_on_a_protected_branch_is_blocked(tmp_path: Path, branch: str) -> None:
    result = guard(WELL_FORMED, repository(tmp_path, branch))
    assert result.returncode == 2
    assert f"commit on {branch}" in result.stderr


def test_the_branch_check_survives_a_compound_command(tmp_path: Path) -> None:
    result = guard(f"cd . && {WELL_FORMED}", repository(tmp_path, "main"))
    assert result.returncode == 2


@pytest.mark.parametrize("branch", ["feat/nav-drawer", "fix/poll-error-173", "chore/branch-guard"])
def test_a_conforming_branch_passes(tmp_path: Path, branch: str) -> None:
    assert guard(WELL_FORMED, repository(tmp_path, branch)).returncode == 0


@pytest.mark.parametrize(
    "branch",
    ["worktree-chore+branch-guard", "experiment", "feature/nav-drawer", "chore/a/b"],
)
def test_an_unconventional_branch_is_blocked(tmp_path: Path, branch: str) -> None:
    result = guard(WELL_FORMED, repository(tmp_path, branch))
    assert result.returncode == 2
    assert "git branch -m <type>/<shortname>" in result.stderr


def test_a_detached_head_is_not_blocked_for_its_name(committed: Path) -> None:
    git(["checkout", "--quiet", "--detach"], committed)
    assert guard(WELL_FORMED, committed).returncode == 0


def test_an_unconventional_branch_outranks_the_record(tmp_path: Path, committed: Path) -> None:
    switched(committed, "chore/intent-guard")
    worktree = tmp_path / "tree"
    git(["worktree", "add", "--quiet", "-b", "worktree-chore+intent", str(worktree)], committed)
    result = guard(WELL_FORMED, worktree, session=SESSION)
    assert result.returncode == 2
    assert "git branch -m <type>/<shortname>" in result.stderr


def test_a_combined_short_flag_is_read(feature_branch: Path) -> None:
    result = guard("git commit -am 'Feat: no issue number'", feature_branch)
    assert result.returncode == 2


def test_a_global_option_before_the_subcommand_is_skipped(feature_branch: Path) -> None:
    result = guard(
        f"git -c core.hooksPath=/dev/null -C {feature_branch} commit -m 'Feat: bad'",
        feature_branch,
    )
    assert result.returncode == 2


@pytest.mark.parametrize(
    "command",
    [
        "git log --oneline -3",
        "git status",
        "uv run pytest",
        "grep -r 'git commit' docs",
    ],
)
def test_other_commands_pass(feature_branch: Path, command: str) -> None:
    assert guard(command, feature_branch).returncode == 0


def test_another_tool_passes(feature_branch: Path) -> None:
    assert guard(WELL_FORMED, feature_branch, tool="Edit").returncode == 0


def test_an_amend_without_a_message_passes(feature_branch: Path) -> None:
    assert guard("git commit --amend --no-edit", feature_branch).returncode == 0


def test_a_commit_on_the_recorded_branch_passes(feature_branch: Path) -> None:
    switched(feature_branch, "chore/intent-guard")
    assert guard(WELL_FORMED, feature_branch, session=SESSION).returncode == 0


def test_a_commit_on_another_branch_is_blocked(feature_branch: Path) -> None:
    switched(feature_branch, "chore/intent-guard")
    git(["checkout", "--quiet", "-b", "chore/someone-else"], feature_branch)
    result = guard(WELL_FORMED, feature_branch, session=SESSION)
    assert result.returncode == 2
    assert "chore/intent-guard" in result.stderr
    assert "chore/someone-else" in result.stderr
    assert str(record(feature_branch)) in result.stderr


def test_a_commit_without_a_record_passes(feature_branch: Path) -> None:
    assert guard(WELL_FORMED, feature_branch, session=SESSION).returncode == 0


def test_a_commit_without_a_session_passes(feature_branch: Path) -> None:
    switched(feature_branch, "chore/intent-guard")
    git(["checkout", "--quiet", "-b", "chore/someone-else"], feature_branch)
    assert guard(WELL_FORMED, feature_branch).returncode == 0


def test_another_sessions_record_is_not_read(feature_branch: Path) -> None:
    switched(feature_branch, "chore/intent-guard")
    git(["checkout", "--quiet", "-b", "chore/someone-else"], feature_branch)
    assert guard(WELL_FORMED, feature_branch, session=PEER).returncode == 0


def test_a_second_switch_replaces_the_record(feature_branch: Path) -> None:
    switched(feature_branch, "chore/first")
    switched(feature_branch, "chore/second")
    assert record(feature_branch).read_text().strip() == "chore/second"
    assert guard(WELL_FORMED, feature_branch, session=SESSION).returncode == 0


@pytest.mark.parametrize(
    "command",
    [
        "git checkout -b chore/intent-guard",
        "git checkout -B chore/intent-guard",
        "git switch -c chore/intent-guard",
        "git checkout --quiet -b chore/intent-guard",
    ],
)
def test_every_switch_form_records_the_branch(feature_branch: Path, command: str) -> None:
    assert guard(command, feature_branch, session=SESSION).returncode == 0
    assert record(feature_branch).read_text().strip() == "chore/intent-guard"


def test_a_plain_switch_to_an_existing_branch_records_it(committed: Path) -> None:
    git(["branch", "chore/intent-guard"], committed)
    assert guard("git switch chore/intent-guard", committed, session=SESSION).returncode == 0
    assert record(committed).read_text().strip() == "chore/intent-guard"


def test_a_path_checkout_records_nothing(committed: Path) -> None:
    assert guard("git checkout README.md", committed, session=SESSION).returncode == 0
    restore = "git checkout chore/agent-enforcement -- README.md"
    assert guard(restore, committed, session=SESSION).returncode == 0
    assert not record(committed).exists()
    assert guard(WELL_FORMED, committed, session=SESSION).returncode == 0


def test_a_renamed_worktree_branch_is_not_blocked(tmp_path: Path, committed: Path) -> None:
    worktree = tmp_path / "tree"
    command = f"git worktree add -b worktree-chore+intent {worktree}"
    assert guard(command, committed, session=SESSION).returncode == 0
    git(["worktree", "add", "--quiet", "-b", "worktree-chore+intent", str(worktree)], committed)
    git(["branch", "-m", "chore/intent-guard"], worktree)
    assert guard(WELL_FORMED, worktree, session=SESSION).returncode == 0


def test_the_record_is_shared_across_worktrees(tmp_path: Path, committed: Path) -> None:
    switched(committed, "chore/intent-guard")
    worktree = tmp_path / "tree"
    git(["worktree", "add", "--quiet", "-b", "chore/someone-else", str(worktree)], committed)
    result = guard(WELL_FORMED, worktree, session=SESSION)
    assert result.returncode == 2
    assert "chore/intent-guard" in result.stderr
    assert "chore/someone-else" in result.stderr


def test_a_stale_record_is_pruned(feature_branch: Path) -> None:
    records = record(feature_branch).parent
    records.mkdir(parents=True)
    stale = records / "00000000-0000-0000-0000-000000000000.txt"
    stale.write_text("chore/ancient\n")
    ancient = time.time() - 15 * 24 * 60 * 60
    os.utime(stale, (ancient, ancient))
    switched(feature_branch, "chore/intent-guard")
    assert not stale.exists()
    assert record(feature_branch).exists()


def test_a_commit_on_a_protected_branch_outranks_the_record(tmp_path: Path) -> None:
    path = repository(tmp_path, "chore/start")
    switched(path, "chore/intent-guard")
    git(["checkout", "--quiet", "-b", "main"], path)
    result = guard(WELL_FORMED, path, session=SESSION)
    assert result.returncode == 2
    assert "commit on main" in result.stderr


def test_malformed_input_passes(feature_branch: Path) -> None:
    result = subprocess.run(
        [sys.executable, str(GUARD)],
        input="not json",
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0
