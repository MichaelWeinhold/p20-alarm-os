from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Final

from check_issue_umbrellas import AuditError, mapping, text, whole

GH: Final = "gh"
GIT: Final = "git"
LIMIT: Final = "500"
ISSUE_FIELDS: Final = "number,title,labels,blockedBy,subIssuesSummary"
PULL_FIELDS: Final = "number,title"
PARKED: Final = "future"
OPEN: Final = "OPEN"
CLAIMS: Final = "refs/claims/"
NEXT: Final = 0
REFERENCE: Final = re.compile(r"\(#(\d+)\)")
SESSION: Final = re.compile(r"^[A-Za-z0-9._-]+$")


class ClaimError(RuntimeError):
    pass


def entries(value: object, where: str) -> list[object]:
    if not isinstance(value, list):
        raise AuditError(f"{where} is {type(value).__name__}, expected a list")
    return list(value)


@dataclass(frozen=True, slots=True)
class Issue:
    number: int
    title: str
    labels: frozenset[str]
    blocked_by: tuple[int, ...]
    children: int


@dataclass(frozen=True, slots=True)
class Claim:
    number: int
    holder: str
    since: str
    blob: str

    def __str__(self) -> str:
        return f"{self.holder} since {self.since}" if self.since else self.holder


@dataclass(frozen=True, slots=True)
class Verdict:
    issue: Issue
    reasons: tuple[str, ...]

    @property
    def free(self) -> bool:
        return not self.reasons


def open_blocker(payload: object, where: str) -> int | None:
    fields = mapping(payload, where)
    number = whole(fields.get("number"), f"{where}.number")
    return number if text(fields.get("state"), f"{where}.state") == OPEN else None


def issue_from(payload: object, where: str) -> Issue:
    fields = mapping(payload, where)
    number = whole(fields.get("number"), f"{where}.number")
    blocked = mapping(fields.get("blockedBy"), f"#{number}.blockedBy")
    summary = mapping(fields.get("subIssuesSummary"), f"#{number}.subIssuesSummary")
    labels = entries(fields.get("labels"), f"#{number}.labels")
    blockers = entries(blocked.get("nodes"), f"#{number}.blockedBy.nodes")
    return Issue(
        number=number,
        title=text(fields.get("title"), f"#{number}.title"),
        labels=frozenset(
            text(mapping(label, f"#{number}.labels[{index}]").get("name"), f"#{number}.label")
            for index, label in enumerate(labels)
        ),
        blocked_by=tuple(
            sorted(
                blocker
                for index, node in enumerate(blockers)
                if (blocker := open_blocker(node, f"#{number}.blockedBy[{index}]")) is not None
            )
        ),
        children=whole(summary.get("total"), f"#{number}.subIssuesSummary.total"),
    )


def issues_from(payload: object) -> list[Issue]:
    return sorted(
        (
            issue_from(item, f"issue at position {position}")
            for position, item in enumerate(entries(payload, f"{GH} issue list"))
        ),
        key=lambda issue: issue.number,
    )


def pulls_from(payload: object) -> dict[int, list[int]]:
    found: dict[int, list[int]] = {}
    for position, item in enumerate(entries(payload, f"{GH} pr list")):
        fields = mapping(item, f"pull request at position {position}")
        number = whole(fields.get("number"), f"pull request at position {position}.number")
        for issue in REFERENCE.findall(text(fields.get("title"), f"pull request #{number}.title")):
            found.setdefault(int(issue), []).append(number)
    return found


def reasons(issue: Issue, pulls: dict[int, list[int]], claims: dict[int, Claim]) -> tuple[str, ...]:
    found: list[str] = []
    if PARKED in issue.labels:
        found.append(f"parked — labelled {PARKED}")
    if issue.blocked_by:
        found.append("waits for " + ", ".join(f"#{number}" for number in issue.blocked_by))
    if issue.children:
        found.append(f"an umbrella of {issue.children} sub-issues — work its children")
    if issue.number in pulls:
        found.append(
            "pull request " + ", ".join(f"#{number}" for number in pulls[issue.number]) + " is open"
        )
    if issue.number in claims:
        found.append(f"claimed by {claims[issue.number]}")
    return tuple(found)


def judge(
    issues: list[Issue], pulls: dict[int, list[int]], claims: dict[int, Claim]
) -> list[Verdict]:
    return [Verdict(issue, reasons(issue, pulls, claims)) for issue in issues]


@dataclass(frozen=True, slots=True)
class Claims:
    root: Path

    def git(self, *args: str, stdin: str | None = None) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [GIT, *args], cwd=self.root, input=stdin, capture_output=True, text=True, check=False
        )

    def output(self, *args: str, stdin: str | None = None) -> str:
        result = self.git(*args, stdin=stdin)
        if result.returncode != 0:
            raise ClaimError(result.stderr.strip() or f"{GIT} {args[0]} exited {result.returncode}")
        return result.stdout

    def held(self) -> dict[int, Claim]:
        found: dict[int, Claim] = {}
        listed = self.output("for-each-ref", "--format=%(refname) %(objectname)", CLAIMS)
        for line in listed.splitlines():
            ref, _, blob = line.partition(" ")
            suffix = ref.removeprefix(CLAIMS)
            if not suffix.isdigit():
                continue
            content = self.output("cat-file", "blob", blob).strip()
            holder, _, since = content.partition(" ")
            found[int(suffix)] = Claim(int(suffix), holder, since, blob)
        return found

    def take(self, number: int, holder: str, at: datetime) -> Claim | None:
        since = at.isoformat(timespec="seconds")
        blob = self.output("hash-object", "-w", "--stdin", stdin=f"{holder} {since}\n").strip()
        created = self.git("update-ref", f"{CLAIMS}{number}", blob, "")
        if created.returncode == 0:
            return Claim(number, holder, since, blob)
        if number in self.held():
            return None
        raise ClaimError(created.stderr.strip() or f"{GIT} update-ref exited {created.returncode}")

    def drop(self, claim: Claim) -> None:
        self.output("update-ref", "-d", f"{CLAIMS}{claim.number}", claim.blob)


def claim_next(
    verdicts: list[Verdict], claims: Claims, holder: str, at: datetime
) -> tuple[Claim | None, list[str]]:
    lost: list[str] = []
    for verdict in verdicts:
        if not verdict.free:
            continue
        claim = claims.take(verdict.issue.number, holder, at)
        if claim is not None:
            return claim, lost
        lost.append(
            f"#{verdict.issue.number} was claimed first by {claims.held()[verdict.issue.number]}"
        )
    return None, lost


Listing = Callable[[Path, str], object]
Clock = Callable[[], datetime]


def from_gh(repo: Path, kind: str) -> object:
    fields = ISSUE_FIELDS if kind == "issue" else PULL_FIELDS
    result = subprocess.run(
        [GH, kind, "list", "--state", "open", "--limit", LIMIT, "--json", fields],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise AuditError(result.stderr.strip() or f"{GH} {kind} list exited {result.returncode}")
    try:
        parsed: object = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise AuditError(f"{GH} returned output that is not JSON: {error}") from error
    return parsed


def now() -> datetime:
    return datetime.now(UTC)


def session(value: str) -> str:
    if SESSION.fullmatch(value) is None:
        raise argparse.ArgumentTypeError(f"{value!r} is not a session name")
    return value


def parser() -> argparse.ArgumentParser:
    found = argparse.ArgumentParser(prog="next_issue.py")
    action = found.add_mutually_exclusive_group()
    action.add_argument("--claim", type=int, nargs="?", const=NEXT, metavar="N")
    action.add_argument("--release", type=int, metavar="N")
    found.add_argument("--by", type=session)
    return found


def prune(issues: list[Issue], claims: Claims) -> dict[int, Claim]:
    numbers = {issue.number for issue in issues}
    held = claims.held()
    for claim in [claim for claim in held.values() if claim.number not in numbers]:
        claims.drop(claim)
        del held[claim.number]
        print(f"released #{claim.number}, no longer open, from {claim}", file=sys.stderr)
    return held


def listed(verdicts: list[Verdict]) -> int:
    for verdict in verdicts:
        if verdict.free:
            print(f"#{verdict.issue.number} {verdict.issue.title}")
        else:
            print(
                f"#{verdict.issue.number} is not free: {'; '.join(verdict.reasons)}",
                file=sys.stderr,
            )
    if any(verdict.free for verdict in verdicts):
        return 0
    print("nothing is free", file=sys.stderr)
    return 1


def claimed_next(verdicts: list[Verdict], claims: Claims, holder: str, at: datetime) -> int:
    claim, lost = claim_next(verdicts, claims, holder, at)
    for line in lost:
        print(line, file=sys.stderr)
    if claim is None:
        print("nothing is free", file=sys.stderr)
        return 1
    print(claim.number)
    return 0


def claimed(number: int, verdicts: list[Verdict], claims: Claims, holder: str, at: datetime) -> int:
    verdict = next((verdict for verdict in verdicts if verdict.issue.number == number), None)
    if verdict is None:
        print(f"#{number} is not open", file=sys.stderr)
        return 1
    current = claims.held().get(number)
    if current is not None and current.holder == holder:
        print(number)
        return 0
    if current is None and (claim := claims.take(number, holder, at)) is not None:
        others = [reason for reason in verdict.reasons if not reason.startswith("claimed by")]
        if others:
            print(f"claimed #{number}, which is not free: {'; '.join(others)}", file=sys.stderr)
        print(claim.number)
        return 0
    print(f"#{number} is held by {claims.held()[number]}", file=sys.stderr)
    return 1


def released(number: int, claims: Claims) -> int:
    claim = claims.held().get(number)
    if claim is None:
        print(f"#{number} held no claim", file=sys.stderr)
        return 0
    claims.drop(claim)
    print(f"released #{number} from {claim}", file=sys.stderr)
    return 0


def main(argv: list[str], listing: Listing = from_gh, clock: Clock = now) -> int:
    options = parser().parse_args(argv[1:])
    claim: int | None = options.claim
    release: int | None = options.release
    holder: str | None = options.by
    if claim is not None and holder is None:
        parser().error("--claim needs --by <session>")
    root = Path.cwd()
    claims = Claims(root)
    try:
        if release is not None:
            return released(release, claims)
        issues = issues_from(listing(root, "issue"))
        pulls = pulls_from(listing(root, "pr"))
        verdicts = judge(issues, pulls, prune(issues, claims))
        if claim is None or holder is None:
            return listed(verdicts)
        if claim == NEXT:
            return claimed_next(verdicts, claims, holder, clock())
        return claimed(claim, verdicts, claims, holder, clock())
    except (AuditError, ClaimError) as error:
        print(error, file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
