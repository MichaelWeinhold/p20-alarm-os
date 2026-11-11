from __future__ import annotations

import json
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

ADRS: Final = Path("docs/adr")
ADR_FILE: Final = re.compile(r"^(\d{4})-.+\.md$")
HEADER_ISSUE: Final = re.compile(r"^-\s+\*\*Issue:\*\*\s+#(\d+)\s*$", re.MULTILINE)
MENTION: Final = re.compile(r"\bADR[ /_-]?(\d{4})\b|docs/adr/(\d{4})\b", re.IGNORECASE)
ORIGINAL: Final = re.compile(r"^##\s+Original request\s*$", re.MULTILINE)
SECTION: Final = re.compile(r"^##\s+", re.MULTILINE)
DECLARATION: Final = 5
ICON: Final = "☂"
GH: Final = "gh"
FIELDS: Final = "number,state,title,parent,body,subIssuesSummary"
LIMIT: Final = "200"
OPEN: Final = "OPEN"
CLOSED: Final = "CLOSED"
INFORMATION: Final = "no parent and no ADR declared — a list to read, not a list to empty:"


class AuditError(RuntimeError):
    pass


def mapping(value: object, where: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise AuditError(f"{where} is {type(value).__name__}, expected an object")
    return {str(key): item for key, item in value.items()}


def whole(value: object, where: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise AuditError(f"{where} is {type(value).__name__}, expected an integer")
    return value


def text(value: object, where: str) -> str:
    if not isinstance(value, str):
        raise AuditError(f"{where} is {type(value).__name__}, expected a string")
    return value


@dataclass(frozen=True, slots=True)
class Issue:
    number: int
    state: str
    title: str
    parent: int | None
    body: str
    children: int
    closed_children: int

    @property
    def open_children(self) -> int:
        return self.children - self.closed_children

    @property
    def carries_icon(self) -> bool:
        return self.title.startswith(ICON)

    @property
    def investigated(self) -> str:
        start = ORIGINAL.search(self.body)
        if start is None:
            return self.body
        rest = self.body[start.end() :]
        following = SECTION.search(rest)
        return rest[following.start() :] if following is not None else ""

    @property
    def declaration(self) -> str:
        return "\n".join([self.title, *self.investigated.splitlines()[:DECLARATION]])

    def declared(self) -> set[str]:
        return {number or path for number, path in MENTION.findall(self.declaration)}

    def mentioned(self) -> set[str]:
        whole = f"{self.title}\n{self.body}"
        return {number or path for number, path in MENTION.findall(whole)}


@dataclass(frozen=True, slots=True)
class Report:
    problems: list[str]
    information: list[str]


def issue_from(payload: object, where: str) -> Issue:
    fields = mapping(payload, where)
    number = whole(fields.get("number"), f"{where}.number")
    parent = fields.get("parent")
    summary = mapping(fields.get("subIssuesSummary"), f"#{number}.subIssuesSummary")
    body = fields.get("body")
    return Issue(
        number=number,
        state=text(fields.get("state"), f"#{number}.state"),
        title=text(fields.get("title"), f"#{number}.title"),
        parent=(
            None
            if parent is None
            else whole(
                mapping(parent, f"#{number}.parent").get("number"), f"#{number}.parent.number"
            )
        ),
        body="" if body is None else text(body, f"#{number}.body"),
        children=whole(summary.get("total"), f"#{number}.subIssuesSummary.total"),
        closed_children=whole(summary.get("completed"), f"#{number}.subIssuesSummary.completed"),
    )


def issues_from(payload: object) -> list[Issue]:
    if not isinstance(payload, list):
        raise AuditError(f"{GH} returned {type(payload).__name__}, expected a list")
    return [
        issue_from(item, f"issue at position {position}") for position, item in enumerate(payload)
    ]


def research_issues(root: Path) -> dict[str, int]:
    found: dict[str, int] = {}
    for path in sorted((root / ADRS).glob("*.md")):
        name = ADR_FILE.match(path.name)
        if name is None:
            continue
        header = HEADER_ISSUE.search(path.read_text(encoding="utf-8"))
        if header is not None:
            found[name.group(1)] = int(header.group(1))
    return found


def umbrellas_of(issue: Issue, adrs: dict[str, int]) -> list[int]:
    return sorted({adrs[number] for number in issue.declared() if number in adrs} - {issue.number})


def mentioned_in(issue: Issue, adrs: dict[str, int]) -> set[int]:
    return {adrs[number] for number in issue.mentioned() if number in adrs} - {issue.number}


def placed(issue: Issue | None, adrs: dict[str, int]) -> bool:
    return (
        issue is not None and issue.parent is not None and issue.parent in mentioned_in(issue, adrs)
    )


def defensible(issue: Issue, parent: Issue | None, adrs: dict[str, int]) -> bool:
    return issue.parent in mentioned_in(issue, adrs) or placed(parent, adrs)


def lineage_problems(issues: list[Issue], adrs: dict[str, int]) -> list[str]:
    research = set(adrs.values())
    by_number = {issue.number: issue for issue in issues}
    found: list[str] = []
    for issue in issues:
        if issue.number in research:
            continue
        wanted = umbrellas_of(issue, adrs)
        if not wanted:
            continue
        names = " or ".join(f"#{number}" for number in wanted)
        if issue.parent is None:
            found.append(
                f"#{issue.number} declares an ADR and has no parent; it belongs under {names}"
            )
        elif not defensible(issue, by_number.get(issue.parent), adrs):
            found.append(
                f"#{issue.number} has parent #{issue.parent}, which researched no ADR its body "
                f"names; it declares {names}"
            )
    return found


def lifecycle_problems(issues: list[Issue]) -> list[str]:
    found: list[str] = []
    for issue in issues:
        if issue.children == 0:
            continue
        if issue.state == CLOSED and issue.open_children > 0:
            found.append(
                f"#{issue.number} is closed with {issue.open_children} of its "
                f"{issue.children} sub-issues still open"
            )
        if issue.state == OPEN and issue.open_children == 0:
            found.append(
                f"#{issue.number} is open and all {issue.children} of its sub-issues are closed; "
                "it is owed a close"
            )
    return found


def icon_problems(issues: list[Issue]) -> list[str]:
    found: list[str] = []
    for issue in issues:
        if issue.children > 0 and not issue.carries_icon:
            found.append(
                f"#{issue.number} holds {issue.children} sub-issues and its title "
                f"does not start with {ICON}"
            )
        if issue.carries_icon and issue.children == 0:
            found.append(f"#{issue.number} starts with {ICON} and holds no sub-issues")
    return found


def unthemed(issues: list[Issue], adrs: dict[str, int]) -> list[str]:
    research = set(adrs.values())
    return [
        f"#{issue.number} {issue.title}"
        for issue in issues
        if issue.state == OPEN
        and issue.parent is None
        and (issue.number in research or not umbrellas_of(issue, adrs))
    ]


def review(issues: list[Issue], adrs: dict[str, int]) -> Report:
    return Report(
        problems=lineage_problems(issues, adrs)
        + lifecycle_problems(issues)
        + icon_problems(issues),
        information=unthemed(issues, adrs),
    )


Listing = Callable[[Path], object]


def from_gh(repo: Path) -> object:
    result = subprocess.run(
        [GH, "issue", "list", "--state", "all", "--limit", LIMIT, "--json", FIELDS],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise AuditError(result.stderr.strip() or f"{GH} issue list exited {result.returncode}")
    try:
        parsed: object = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AuditError(f"{GH} returned output that is not JSON: {error}") from error
    return parsed


def main(argv: list[str], listing: Listing = from_gh) -> int:
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path.cwd()
    try:
        report = review(issues_from(listing(root)), research_issues(root))
    except AuditError as error:
        print(error, file=sys.stderr)
        return 2
    for problem in report.problems:
        print(problem, file=sys.stderr)
    if report.information:
        print(INFORMATION)
        for line in report.information:
            print(f"  {line}")
    return 1 if report.problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
