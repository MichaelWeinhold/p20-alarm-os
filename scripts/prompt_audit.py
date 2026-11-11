from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Final

CLAUDE: Final = "claude"
PROMPT: Final = "/doctor prompt-audit"
ALLOWED_TOOLS: Final = ("Read", "Glob", "Grep", "Skill")
FLOOR: Final = (2, 1, 283)
VERSION: Final = re.compile(r"(\d+)\.(\d+)\.(\d+)")
VERSION_TIMEOUT: Final = 30.0
AUDIT_TIMEOUT: Final = 1800.0
SKIPPED: Final = 3
FAILED: Final = 1
TIMED_OUT: Final = 124
UNAVAILABLE: Final = 127
DISABLE_ENV: Final = "CLAUDE_CODE_DISABLE_BUNDLED_SKILLS"
DISABLE_KEY: Final = "disableBundledSkills"
SKILL: Final = "claude-api"
SETTINGS: Final = (
    Path(".claude") / "settings.json",
    Path(".claude") / "settings.local.json",
)
SHORTEST_REPORT: Final = 10
STEPS: Final = "docs/manual-steps/before-every-promotion-to-production.md"
SECTION: Final = "Before every promotion to `production`"


@dataclass(frozen=True, slots=True)
class Result:
    code: int
    out: str
    err: str


Runner = Callable[[Sequence[str], float, Path], Result]


class AuditError(RuntimeError):
    def __init__(self, reason: str, code: int) -> None:
        super().__init__(reason)
        self.code = code


def run_command(argv: Sequence[str], timeout: float, cwd: Path) -> Result:
    try:
        completed = subprocess.run(
            list(argv),
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return Result(TIMED_OUT, "", f"no answer within {timeout:.0f}s")
    except OSError as error:
        return Result(UNAVAILABLE, "", str(error))
    return Result(completed.returncode, completed.stdout, completed.stderr)


def detail(result: Result) -> str:
    text = result.err.strip() or result.out.strip()
    return text.splitlines()[-1] if text else f"exit status {result.code}"


def rendered(version: tuple[int, ...]) -> str:
    return ".".join(str(part) for part in version)


def binary() -> str:
    found = shutil.which(CLAUDE)
    if found is None:
        raise AuditError(f"{CLAUDE} is not on PATH, so the audit has nothing to run", SKIPPED)
    return found


def installed(run: Runner, cwd: Path) -> tuple[int, int, int]:
    result = run([CLAUDE, "--version"], VERSION_TIMEOUT, cwd)
    if result.code != 0:
        raise AuditError(f"{CLAUDE} --version failed: {detail(result)}", SKIPPED)
    match = VERSION.search(result.out)
    if match is None:
        raise AuditError(
            f"{CLAUDE} --version answered {result.out.strip()!r}, which carries no version",
            SKIPPED,
        )
    major, minor, patch = (int(part) for part in match.groups())
    if (major, minor, patch) < FLOOR:
        raise AuditError(
            f"{CLAUDE} is {rendered((major, minor, patch))} and the {PROMPT} argument "
            f"was added in {rendered(FLOOR)}",
            SKIPPED,
        )
    return major, minor, patch


def disabled_in(path: Path) -> bool:
    try:
        loaded: object = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return isinstance(loaded, dict) and loaded.get(DISABLE_KEY) is True


def bundled_skills_off(root: Path) -> None:
    if os.environ.get(DISABLE_ENV, "").strip():
        raise AuditError(
            f"{DISABLE_ENV} is set, so the bundled {SKILL} skill the audit hands off to "
            "is absent and nothing would be graded",
            SKIPPED,
        )
    for relative in SETTINGS:
        for path in (root / relative, Path.home() / relative):
            if disabled_in(path):
                raise AuditError(
                    f"{path} sets {DISABLE_KEY}, so the bundled {SKILL} skill the audit "
                    "hands off to is absent and nothing would be graded",
                    SKIPPED,
                )


def report(run: Runner, root: Path) -> str:
    result = run(
        [
            CLAUDE,
            "-p",
            PROMPT,
            "--tools",
            ",".join(ALLOWED_TOOLS),
            "--allowed-tools",
            ",".join(ALLOWED_TOOLS),
            "--strict-mcp-config",
        ],
        AUDIT_TIMEOUT,
        root,
    )
    if result.code != 0:
        raise AuditError(f"{CLAUDE} -p {PROMPT!r} failed: {detail(result)}", FAILED)
    text = result.out.strip()
    if len(text.splitlines()) < SHORTEST_REPORT:
        raise AuditError(
            f"the audit answered in {len(text.splitlines())} lines — "
            f"{text!r} — which is its own absence and not a report",
            SKIPPED,
        )
    return text


def audit(root: Path, run: Runner = run_command) -> str:
    binary()
    installed(run, root)
    bundled_skills_off(root)
    return report(run, root)


def parser() -> argparse.ArgumentParser:
    built = argparse.ArgumentParser(
        prog="prompt_audit",
        description=(
            f"Run {PROMPT} over this repository's instruction files without a session to "
            "type it into, and print the report it returns. The report is read by a person: "
            "this exits 0 whenever the audit ran, however many findings it raised, and "
            f"{SKIPPED} when it could not run at all. The tools it is given can read and "
            "nothing more, so a proposed edit stays a proposal."
        ),
    )
    built.add_argument(
        "root", nargs="?", help="the repository root; the current directory by default"
    )
    return built


def main(argv: Sequence[str], run: Runner = run_command) -> int:
    arguments = parser().parse_args(list(argv[1:]))
    root = Path(arguments.root).resolve() if arguments.root else Path.cwd()
    try:
        print(audit(root, run))
    except AuditError as error:
        state = "skipped" if error.code == SKIPPED else "failed"
        print(f"the prompt audit was {state}: {error}", file=sys.stderr)
        if error.code == SKIPPED:
            print(f'note it in the promotion pull request — {STEPS}, "{SECTION}"', file=sys.stderr)
        return error.code
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
