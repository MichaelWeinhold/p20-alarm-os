from __future__ import annotations

import json
import subprocess
from collections.abc import Sequence
from pathlib import Path

import pytest

from prompt_audit import (
    ALLOWED_TOOLS,
    CLAUDE,
    DISABLE_ENV,
    DISABLE_KEY,
    FAILED,
    SETTINGS,
    SHORTEST_REPORT,
    SKIPPED,
    Result,
    main,
    run_command,
)

VERSION = "2.1.285 (Claude Code)\n"
REPORT = "# Prompt audit\n\n" + "\n".join(f"| {n} | a finding |" for n in range(SHORTEST_REPORT))
WRITERS = ("Edit", "Write", "Bash", "NotebookEdit", "Task", "Agent", "SendMessage")


class Claude:
    def __init__(self, *replies: Result | str) -> None:
        self.replies = [
            reply if isinstance(reply, Result) else Result(0, reply, "") for reply in replies
        ]
        self.calls: list[list[str]] = []

    def __call__(self, argv: Sequence[str], timeout: float, cwd: Path) -> Result:
        self.calls.append(list(argv))
        return self.replies.pop(0) if self.replies else Result(0, "", "")


@pytest.fixture(autouse=True)
def an_installed_claude(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr("prompt_audit.shutil.which", lambda _: "/opt/bin/claude")
    monkeypatch.setattr("prompt_audit.Path.home", lambda: tmp_path / "home")
    monkeypatch.delenv(DISABLE_ENV, raising=False)


def test_the_report_is_printed_and_findings_are_not_graded(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    run = Claude(VERSION, REPORT)
    assert main(["prompt_audit.py", str(tmp_path)], run) == 0
    assert REPORT in capsys.readouterr().out


def test_the_audit_has_only_read_tools_and_no_mcp_servers(tmp_path: Path) -> None:
    run = Claude(VERSION, REPORT)
    main(["prompt_audit.py", str(tmp_path)], run)
    argv = run.calls[-1]
    assert argv[:3] == [CLAUDE, "-p", "/doctor prompt-audit"]
    assert argv[argv.index("--tools") + 1] == "Read,Glob,Grep,Skill"
    assert argv[argv.index("--allowed-tools") + 1] == "Read,Glob,Grep,Skill"
    assert "--strict-mcp-config" in argv
    assert "--mcp-config" not in argv
    assert set(ALLOWED_TOOLS).isdisjoint(WRITERS)


def test_it_runs_where_the_instruction_files_are(tmp_path: Path) -> None:
    seen: list[Path] = []

    def run(argv: Sequence[str], timeout: float, cwd: Path) -> Result:
        seen.append(cwd)
        return Result(0, VERSION if "--version" in argv else REPORT, "")

    assert main(["prompt_audit.py", str(tmp_path)], run) == 0
    assert seen == [tmp_path, tmp_path]


def test_an_absent_claude_is_a_skip(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setattr("prompt_audit.shutil.which", lambda _: None)
    run = Claude(VERSION, REPORT)
    assert main(["prompt_audit.py", str(tmp_path)], run) == SKIPPED
    captured = capsys.readouterr()
    assert "not on PATH" in captured.err
    assert "docs/manual-steps/before-every-promotion-to-production.md" in captured.err
    assert run.calls == []


def test_a_version_below_the_floor_is_a_skip(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    run = Claude("2.1.282 (Claude Code)\n", REPORT)
    assert main(["prompt_audit.py", str(tmp_path)], run) == SKIPPED
    assert "2.1.282" in capsys.readouterr().err
    assert len(run.calls) == 1


def test_bundled_skills_off_by_environment_is_a_skip(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.setenv(DISABLE_ENV, "1")
    run = Claude(VERSION, REPORT)
    assert main(["prompt_audit.py", str(tmp_path)], run) == SKIPPED
    assert "claude-api" in capsys.readouterr().err
    assert len(run.calls) == 1


@pytest.mark.parametrize("relative", SETTINGS)
def test_bundled_skills_off_by_settings_is_a_skip(
    tmp_path: Path, relative: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    settings = tmp_path / relative
    settings.parent.mkdir(parents=True, exist_ok=True)
    settings.write_text(json.dumps({DISABLE_KEY: True}), encoding="utf-8")
    run = Claude(VERSION, REPORT)
    assert main(["prompt_audit.py", str(tmp_path)], run) == SKIPPED
    assert DISABLE_KEY in capsys.readouterr().err


def test_settings_that_say_nothing_about_skills_are_no_obstacle(tmp_path: Path) -> None:
    settings = tmp_path / SETTINGS[0]
    settings.parent.mkdir(parents=True, exist_ok=True)
    settings.write_text(json.dumps({"attribution": {"commit": ""}}), encoding="utf-8")
    assert main(["prompt_audit.py", str(tmp_path)], Claude(VERSION, REPORT)) == 0


def test_a_one_sentence_answer_is_a_skip_rather_than_a_pass(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    run = Claude(VERSION, "The claude-api skill is disabled, so there is nothing to audit.\n")
    assert main(["prompt_audit.py", str(tmp_path)], run) == SKIPPED
    assert "its own absence" in capsys.readouterr().err


def test_a_failed_run_is_not_a_skip(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    run = Claude(VERSION, Result(1, "", "Invalid API key\n"))
    assert main(["prompt_audit.py", str(tmp_path)], run) == FAILED
    captured = capsys.readouterr()
    assert "failed: Invalid API key" in captured.err
    assert "manual-steps" not in captured.err


def test_a_failure_reports_its_cause_rather_than_a_leading_warning(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    stderr = "Warning: no stdin data received in 3s, proceeding without it.\nRate limit reached\n"
    run = Claude(VERSION, Result(1, "", stderr))
    assert main(["prompt_audit.py", str(tmp_path)], run) == FAILED
    assert "failed: Rate limit reached" in capsys.readouterr().err


def test_claude_is_given_no_stdin_to_wait_for(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    seen: dict[str, object] = {}

    def fake(argv: list[str], **options: object) -> subprocess.CompletedProcess[str]:
        seen.update(options)
        return subprocess.CompletedProcess(argv, 0, "", "")

    monkeypatch.setattr("prompt_audit.subprocess.run", fake)
    run_command([CLAUDE, "--version"], 1.0, tmp_path)
    assert seen["stdin"] is subprocess.DEVNULL
