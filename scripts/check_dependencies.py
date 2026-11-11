from __future__ import annotations

import json
import re
import subprocess
import sys
import tomllib
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from check_issue_umbrellas import AuditError, mapping, text, whole

GH: Final = "gh"
GIT: Final = "git"
FIELDS: Final = "number,body,baseRefOid,headRefOid"
PYTHON: Final = "pyproject.toml"
CARGO: Final = "Cargo.toml"
MANIFESTS: Final = f"{PYTHON} or a {CARGO}"
CARGO_TABLES: Final = ("dependencies", "dev-dependencies", "build-dependencies")
NAME: Final = re.compile(r"^\s*([A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?)")
HEADING: Final = re.compile(r"^##(?!#)\s*(.*?)\s*$")
SECTION: Final = re.compile(r"^Dependency:\s*`?([^`\s]+)`?$")
ANSWER: Final = re.compile(r"^\s*([1-9])\.\s+\S")
QUESTIONS: Final = (
    "What it replaces",
    "Who publishes it",
    "Whether it is maintained",
    "Its licence",
    "What it pulls in",
    "Where it runs — on the host only, or on the phone",
)


@dataclass(frozen=True, slots=True)
class PullRequest:
    number: int
    body: str
    base: str
    head: str


def normalised(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def requirement_names(entries: object, where: str) -> set[str]:
    if not isinstance(entries, list):
        raise AuditError(f"{where} is {type(entries).__name__}, expected a list")
    found: set[str] = set()
    for position, entry in enumerate(entries):
        if isinstance(entry, dict):
            continue
        requirement = text(entry, f"{where}[{position}]")
        name = NAME.match(requirement)
        if name is None:
            raise AuditError(f"{where}[{position}] {requirement!r} names no package")
        found.add(normalised(name.group(1)))
    return found


def parsed_toml(manifest: str, where: str) -> dict[str, object]:
    try:
        return mapping(tomllib.loads(manifest), where)
    except tomllib.TOMLDecodeError as error:
        raise AuditError(f"{where} is not TOML: {error}") from error


def python_packages(manifest: str, where: str) -> frozenset[str]:
    parsed = parsed_toml(manifest, where)
    project = mapping(parsed.get("project", {}), f"{where} [project]")
    found = requirement_names(project.get("dependencies", []), f"{where} project.dependencies")
    extras = mapping(project.get("optional-dependencies", {}), f"{where} optional-dependencies")
    for extra, entries in extras.items():
        found |= requirement_names(entries, f"{where} optional-dependencies.{extra}")
    groups = mapping(parsed.get("dependency-groups", {}), f"{where} [dependency-groups]")
    for group, entries in groups.items():
        found |= requirement_names(entries, f"{where} dependency-groups.{group}")
    return frozenset(found)


def crate_names(table: object, where: str) -> set[str]:
    found: set[str] = set()
    for key, value in mapping(table, where).items():
        if isinstance(value, dict):
            if "path" in value:
                continue
            renamed = value.get("package", key)
            found.add(normalised(text(renamed, f"{where}.{key}.package")))
        else:
            found.add(normalised(key))
    return found


def dependency_tables(owner: dict[str, object], where: str) -> set[str]:
    found: set[str] = set()
    for table in CARGO_TABLES:
        found |= crate_names(owner.get(table, {}), f"{where} [{table}]")
    return found


def cargo_packages(manifest: str, where: str) -> frozenset[str]:
    parsed = parsed_toml(manifest, where)
    found = dependency_tables(parsed, where)
    workspace = mapping(parsed.get("workspace", {}), f"{where} [workspace]")
    found |= crate_names(workspace.get("dependencies", {}), f"{where} [workspace.dependencies]")
    targets = mapping(parsed.get("target", {}), f"{where} [target]")
    for target, owner in targets.items():
        found |= dependency_tables(mapping(owner, f"{where} [target.{target}]"), where)
    return frozenset(found)


def packages(manifests: dict[str, str], revision: str) -> frozenset[str]:
    found: set[str] = set()
    for path, manifest in manifests.items():
        where = f"{revision[:12]}:{path}"
        if path == PYTHON:
            found |= python_packages(manifest, where)
        elif Path(path).name == CARGO:
            found |= cargo_packages(manifest, where)
    return frozenset(found)


def sections(body: str) -> dict[str, frozenset[int]]:
    found: dict[str, set[int]] = {}
    current: set[int] | None = None
    for line in body.splitlines():
        heading = HEADING.match(line)
        if heading is not None:
            section = SECTION.match(heading.group(1))
            current = (
                None if section is None else found.setdefault(normalised(section.group(1)), set())
            )
            continue
        answer = ANSWER.match(line)
        if current is not None and answer is not None:
            current.add(int(answer.group(1)))
    return {name: frozenset(answers) for name, answers in found.items()}


def listed(names: frozenset[str]) -> str:
    return ", ".join(sorted(names))


def template(name: str) -> str:
    lines = [f"## Dependency: {name}", ""]
    lines += [f"{position}. **{question}.** …" for position, question in enumerate(QUESTIONS, 1)]
    return "\n".join(f"    {line}" for line in lines)


def problems(added: frozenset[str], body: str) -> list[str]:
    answered = sections(body)
    found: list[str] = []
    for name in sorted(added):
        answers = answered.get(name)
        if answers is None:
            found.append(
                f"{name} is new in {MANIFESTS} and the PR body has no '## Dependency: {name}' "
                "section. ADR 0001, D7: a package is added only when it is worth it, and the "
                f"pull request says why, answering each question:\n{template(name)}"
            )
            continue
        missing = [
            f"{position}. {question}"
            for position, question in enumerate(QUESTIONS, 1)
            if position not in answers
        ]
        if missing:
            found.append(
                f"'## Dependency: {name}' answers no numbered item for: "
                f"{'; '.join(missing)}. ADR 0001, D7 asks all six."
            )
    return found


def pull_request_from(payload: object) -> PullRequest:
    fields = mapping(payload, f"{GH} pr view")
    body = fields.get("body")
    return PullRequest(
        number=whole(fields.get("number"), "number"),
        body="" if body is None else text(body, "body"),
        base=text(fields.get("baseRefOid"), "baseRefOid"),
        head=text(fields.get("headRefOid"), "headRefOid"),
    )


Reading = Callable[[Path, int], object]
Showing = Callable[[Path, str], dict[str, str]]


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


def git(repo: Path, arguments: list[str]) -> str:
    result = subprocess.run(
        [GIT, *arguments],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise AuditError(
            result.stderr.strip() or f"{GIT} {arguments[0]} exited {result.returncode}"
        )
    return result.stdout


def is_manifest(path: str) -> bool:
    return path == PYTHON or (Path(path).name == CARGO and not path.startswith("target/"))


def from_git(repo: Path, revision: str) -> dict[str, str]:
    listing = git(repo, ["ls-tree", "-r", "--name-only", revision])
    return {
        path: git(repo, ["show", f"{revision}:{path}"])
        for path in listing.splitlines()
        if is_manifest(path)
    }


def main(argv: list[str], reading: Reading = from_gh, showing: Showing = from_git) -> int:
    if len(argv) < 2 or not argv[1].isdigit():
        print(f"usage: {Path(argv[0]).name} <pr-number> [repository-root]", file=sys.stderr)
        return 2
    root = Path(argv[2]).resolve() if len(argv) > 2 else Path.cwd()
    try:
        pull = pull_request_from(reading(root, int(argv[1])))
        before = packages(showing(root, pull.base), pull.base)
        after = packages(showing(root, pull.head), pull.head)
    except AuditError as error:
        print(error, file=sys.stderr)
        return 2
    added = after - before
    found = problems(added, pull.body)
    print(
        f"PR #{pull.number} adds {listed(added) or 'no package'} "
        f"and removes {listed(before - after) or 'none'}"
    )
    for problem in found:
        print(problem, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
