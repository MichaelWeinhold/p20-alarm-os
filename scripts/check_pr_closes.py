from __future__ import annotations

import json
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from check_issue_umbrellas import AuditError, mapping, research_issues, text, whole

PREFIX: Final = re.compile(r"^(?:Feat|Fix|Chore)\(#(\d+)\):")
GH: Final = "gh"
FIELDS: Final = "number,closingIssuesReferences,commits"


@dataclass(frozen=True, slots=True)
class PullRequest:
    number: int
    closes: frozenset[int]
    claimed: frozenset[int]


def numbers_from(payload: object, where: str) -> frozenset[int]:
    if not isinstance(payload, list):
        raise AuditError(f"{where} is {type(payload).__name__}, expected a list")
    return frozenset(
        whole(mapping(item, f"{where}[{position}]").get("number"), f"{where}[{position}].number")
        for position, item in enumerate(payload)
    )


def claims_from(payload: object, where: str) -> frozenset[int]:
    if not isinstance(payload, list):
        raise AuditError(f"{where} is {type(payload).__name__}, expected a list")
    found: set[int] = set()
    for position, item in enumerate(payload):
        headline = text(
            mapping(item, f"{where}[{position}]").get("messageHeadline"),
            f"{where}[{position}].messageHeadline",
        )
        prefix = PREFIX.match(headline)
        if prefix is not None:
            found.add(int(prefix.group(1)))
    return frozenset(found)


def pull_request_from(payload: object) -> PullRequest:
    fields = mapping(payload, f"{GH} pr view")
    number = whole(fields.get("number"), "number")
    return PullRequest(
        number=number,
        closes=numbers_from(fields.get("closingIssuesReferences"), "closingIssuesReferences"),
        claimed=claims_from(fields.get("commits"), "commits"),
    )


def listed(numbers: frozenset[int]) -> str:
    return ", ".join(f"#{number}" for number in sorted(numbers))


def problems(pull: PullRequest, research: frozenset[int]) -> list[str]:
    found: list[str] = []
    stray = pull.closes - pull.claimed
    if stray:
        found.append(
            f"PR #{pull.number} closes {listed(stray)}, which no commit on it claims. "
            f"The only issue a pull request may close is the one in its commit prefix"
            f"{f' ({listed(pull.claimed)})' if pull.claimed else ', and no commit carries one'}. "
            "GitHub reads close/fix/resolve in front of any number anywhere in the body, so "
            "write 'issue N' or move the number away from the keyword."
        )
    umbrellas = pull.closes & research
    if umbrellas:
        found.append(
            f"PR #{pull.number} closes {listed(umbrellas)}, which is a research issue. "
            "A research issue is an umbrella and closes when the last of its follow-ups "
            "does, not on merge day — write 'answers issue N', never 'Closes #N'."
        )
    return found


Reading = Callable[[Path, int], object]


def from_gh(repo: Path, number: int) -> object:
    result = subprocess.run(
        [GH, "pr", "view", str(number), "--json", FIELDS],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise AuditError(result.stderr.strip() or f"{GH} pr view exited {result.returncode}")
    try:
        parsed: object = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AuditError(f"{GH} returned output that is not JSON: {error}") from error
    return parsed


def main(argv: list[str], reading: Reading = from_gh) -> int:
    if len(argv) < 2 or not argv[1].isdigit():
        print(f"usage: {Path(argv[0]).name} <pr-number> [repository-root]", file=sys.stderr)
        return 2
    number = int(argv[1])
    root = Path(argv[2]).resolve() if len(argv) > 2 else Path.cwd()
    try:
        pull = pull_request_from(reading(root, number))
        found = problems(pull, frozenset(research_issues(root).values()))
    except AuditError as error:
        print(error, file=sys.stderr)
        return 2
    print(
        f"PR #{pull.number} closes {listed(pull.closes) or 'nothing'}; "
        f"its commits claim {listed(pull.claimed) or 'nothing'}"
    )
    for problem in found:
        print(problem, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
