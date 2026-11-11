from __future__ import annotations

import json
import re
import sys
from pathlib import Path

LIMIT = 200
COMBINED_LIMIT = 270
ALWAYS_ON = ("AGENTS.md", "CLAUDE.md")
MANUAL_STEPS = Path("docs/MANUAL_STEPS.md")
MANUAL_STEPS_DIR = Path("docs/manual-steps")
MANUAL_STEPS_INDEX_CEILING = 100
MANUAL_STEPS_FILE_CEILING = 250
SKILL_LIMIT = 500
RULE_LIMIT = 200
ON_DEMAND_LIMIT = 1000
PROSE_WIDTH = 90
RULES = Path(".claude/rules")
SKILLS = Path(".claude/skills")
SETTINGS = Path(".claude/settings.json")
WORKFLOW = Path(".github/workflows/checks.yml")
GATE = Path(".github/workflows/pr-closes.yml")
GATE_PERMISSIONS = ("contents: read", "pull-requests: read", "issues: read")

LOCAL_SYNC = "uv sync"
WORKFLOW_SYNC = "uv sync --locked"

REQUIRED_SETTINGS: tuple[tuple[tuple[str, ...], object], ...] = (
    (("includeGitInstructions",), False),
    (("attribution", "commit"), ""),
    (("attribution", "pr"), ""),
    (("attribution", "sessionUrl"), False),
    (("autoCompactWindow",), 200_000),
)

FRONTMATTER = re.compile(r"\A---\n(.*?)\n---\n", re.DOTALL)
PATHS_KEY = re.compile(r"^paths\s*:", re.MULTILINE)
LIST_ITEM = re.compile(r'^\s+-\s*"?(.+?)"?\s*$')
NAME_KEY = re.compile(r"^name\s*:\s*(.*?)\s*$", re.MULTILINE)
DESCRIPTION_KEY = re.compile(r"^description\s*:\s*(.*?)\s*$", re.MULTILINE)
HEADING = re.compile(r"^##\s+Architecture rules\s*$", re.MULTILINE)
NEXT_HEADING = re.compile(r"^##\s+", re.MULTILINE)
NUMBERED = re.compile(r"^(\d+)\.\s", re.MULTILINE)
TOOLING = re.compile(r"^##\s+Tooling\s*$.*?^```bash\n(.*?)^```", re.MULTILINE | re.DOTALL)
CHECK_JOB = re.compile(r"^  check:[ \t]*$\n(.*?)(?=^  \S|\Z)", re.MULTILINE | re.DOTALL)
RUN_STEP = re.compile(r"^\s+run:[ \t]*(\S.*?)\s*$", re.MULTILINE)
OPEN_BOX = re.compile(r"^\s*- \[ \]", re.MULTILINE)


def frontmatter(text: str) -> str | None:
    match = FRONTMATTER.match(text)
    return match.group(1) if match is not None else None


def body_start(text: str) -> int:
    match = FRONTMATTER.match(text)
    return match.end() if match is not None else 0


def is_path_scoped(text: str) -> bool:
    block = frontmatter(text)
    return block is not None and PATHS_KEY.search(block) is not None


def globs(text: str) -> list[str]:
    block = frontmatter(text)
    if block is None:
        return []
    found: list[str] = []
    inside = False
    for line in block.splitlines():
        if PATHS_KEY.match(line):
            inside = True
            continue
        if not inside:
            continue
        item = LIST_ITEM.match(line)
        if item is None:
            break
        found.append(item.group(1))
    return found


def expression(pattern: str) -> re.Pattern[str]:
    parts: list[str] = []
    index = 0
    while index < len(pattern):
        if pattern.startswith("**", index):
            parts.append(".*")
            index += 2
            continue
        character = pattern[index]
        if character == "*":
            parts.append("[^/]*")
        elif character == "?":
            parts.append("[^/]")
        else:
            parts.append(re.escape(character))
        index += 1
    return re.compile("".join(parts))


def search_root(root: Path, pattern: str) -> Path:
    candidate = root / pattern.split("*")[0]
    return candidate if candidate.is_dir() else candidate.parent


def matches_a_file(root: Path, pattern: str) -> bool:
    wanted = expression(pattern)
    for path in search_root(root, pattern).rglob("*"):
        if path.is_file() and wanted.fullmatch(path.relative_to(root).as_posix()):
            return True
    return False


def rule_files(root: Path) -> list[Path]:
    rules = root / RULES
    return sorted(rules.rglob("*.md")) if rules.is_dir() else []


def skill_files(root: Path) -> list[Path]:
    skills = root / SKILLS
    return sorted(skills.glob("*/SKILL.md")) if skills.is_dir() else []


def always_on_files(root: Path) -> list[Path]:
    found = [root / name for name in ALWAYS_ON]
    found += [
        rule for rule in rule_files(root) if not is_path_scoped(rule.read_text(encoding="utf-8"))
    ]
    return [path for path in found if path.is_file()]


def instruction_files(root: Path) -> list[Path]:
    found = [root / name for name in ALWAYS_ON if (root / name).is_file()]
    return found + rule_files(root) + skill_files(root)


def line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").splitlines())


def body_line_count(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    return len(text[body_start(text) :].splitlines())


def on_demand(root: Path) -> list[tuple[Path, int, int]]:
    skills = [(skill, body_line_count(skill), SKILL_LIMIT) for skill in skill_files(root)]
    rules = [
        (rule, line_count(rule), RULE_LIMIT)
        for rule in rule_files(root)
        if is_path_scoped(rule.read_text(encoding="utf-8"))
    ]
    return skills + rules


def prose_lines(text: str) -> list[tuple[int, str]]:
    start = body_start(text)
    found: list[tuple[int, str]] = []
    fenced = False
    for number, line in enumerate(text[start:].splitlines(), text[:start].count("\n") + 1):
        stripped = line.lstrip()
        if stripped.startswith("```"):
            fenced = not fenced
        elif not fenced and not stripped.startswith("|"):
            found.append((number, line))
    return found


def lines(count: int) -> str:
    return f"{count} line" if count == 1 else f"{count} lines"


def headroom(used: int, limit: int) -> str:
    remaining = limit - used
    if remaining < 0:
        return f"over by {lines(-remaining)}"
    return f"{lines(remaining)} of headroom"


RELOCATE = (
    "Move a procedure into .claude/skills/ or a directory-specific rule into "
    ".claude/rules/ with paths: frontmatter."
)


def gauge(root: Path) -> list[str]:
    measured = [(path, line_count(path)) for path in always_on_files(root)]
    reported = [
        f"{path.relative_to(root).as_posix()} {count}/{LIMIT} ({headroom(count, LIMIT)})"
        for path, count in measured
    ]
    total = sum(count for _, count in measured)
    reported.append(f"always-on total {total}/{COMBINED_LIMIT} ({headroom(total, COMBINED_LIMIT)})")
    loaded = on_demand(root)
    reported += [
        f"{path.relative_to(root).as_posix()} {count}/{limit} ({headroom(count, limit)})"
        for path, count, limit in loaded
    ]
    total = sum(count for _, count, _ in loaded)
    reported.append(
        f"on-demand total {total}/{ON_DEMAND_LIMIT} ({headroom(total, ON_DEMAND_LIMIT)})"
    )
    return reported + manual_steps_gauge(root)


def manual_steps_files(root: Path) -> list[Path]:
    directory = root / MANUAL_STEPS_DIR
    return sorted(directory.glob("*.md")) if directory.is_dir() else []


def open_boxes(path: Path) -> int:
    return len(OPEN_BOX.findall(path.read_text(encoding="utf-8")))


def manual_steps_gauge(root: Path) -> list[str]:
    reported: list[str] = []
    index = root / MANUAL_STEPS
    if index.is_file():
        count = line_count(index)
        reported.append(
            f"{MANUAL_STEPS.as_posix()} {count}/{MANUAL_STEPS_INDEX_CEILING} "
            f"({headroom(count, MANUAL_STEPS_INDEX_CEILING)})"
        )
    files = manual_steps_files(root)
    for path in files:
        count = line_count(path)
        reported.append(
            f"{path.relative_to(root).as_posix()} {count}/{MANUAL_STEPS_FILE_CEILING} "
            f"({headroom(count, MANUAL_STEPS_FILE_CEILING)}), {open_boxes(path)} open"
        )
    if files:
        total = sum(line_count(path) for path in files)
        boxes = sum(open_boxes(path) for path in files)
        reported.append(f"{MANUAL_STEPS_DIR.as_posix()}/ total {lines(total)}, {boxes} open")
    return reported


def length_problems(root: Path) -> list[str]:
    found = [
        f"{name} is missing, and it is one of the always-on files the combined "
        f"ceiling is measured over"
        for name in ALWAYS_ON
        if not (root / name).is_file()
    ]
    for path in always_on_files(root):
        count = line_count(path)
        if count > LIMIT:
            found.append(
                f"{path.relative_to(root)} is {count} lines and loads on every turn; "
                f"the limit is {LIMIT}. {RELOCATE}"
            )
    return found


def combined_problems(root: Path) -> list[str]:
    measured = always_on_files(root)
    total = sum(line_count(path) for path in measured)
    if total <= COMBINED_LIMIT:
        return []
    names = ", ".join(path.relative_to(root).as_posix() for path in measured)
    return [
        f"the always-on set is {total} lines and the combined ceiling is "
        f"{COMBINED_LIMIT} — {names}. The per-file limit of {LIMIT} is satisfied by "
        f"splitting one file in two, so this is the number that catches regrowth. "
        f"{RELOCATE} → ADR 0001, D1."
    ]


def on_demand_problems(root: Path) -> list[str]:
    loaded = on_demand(root)
    found = [
        f"{path.relative_to(root).as_posix()} is {count} lines and the limit is {limit}. "
        "Split the procedure or move its rationale into an ADR. → ADR 0001, D1."
        for path, count, limit in loaded
        if count > limit
    ]
    total = sum(count for _, count, _ in loaded)
    if total > ON_DEMAND_LIMIT:
        found.append(
            f"the skills and path-scoped rules are {total} lines and the on-demand ceiling is "
            f"{ON_DEMAND_LIMIT}. Trim them to .claude/rules/instruction-files.md. "
            "→ ADR 0001, D1."
        )
    return found


def width_problems(root: Path) -> list[str]:
    found: list[str] = []
    for path in instruction_files(root):
        name = path.relative_to(root).as_posix()
        found += [
            f"{name}:{number} is {len(line)} characters of prose and the limit is "
            f"{PROSE_WIDTH}. Rewrap it; fenced code and tables are exempt. → ADR 0001, D1."
            for number, line in prose_lines(path.read_text(encoding="utf-8"))
            if len(line) > PROSE_WIDTH
        ]
    return found


def manual_steps_problems(root: Path) -> list[str]:
    found: list[str] = []
    index = root / MANUAL_STEPS
    if index.is_file() and (count := line_count(index)) > MANUAL_STEPS_INDEX_CEILING:
        found.append(
            f"{MANUAL_STEPS.as_posix()} is {count} lines and the ceiling is "
            f"{MANUAL_STEPS_INDEX_CEILING}. It holds the rules and one row per file; move "
            f"anything else into the file for its subsystem. → ADR 0001, D8."
        )
    found += [
        f"{path.relative_to(root).as_posix()} is {count} lines and the ceiling is "
        f"{MANUAL_STEPS_FILE_CEILING}. Move the record into docs/device-history/, rationale "
        "for a tracked file into a README.md beside it, "
        "or a second subsystem into a file of its own. → ADR 0001, D8."
        for path in manual_steps_files(root)
        if (count := line_count(path)) > MANUAL_STEPS_FILE_CEILING
    ]
    return found


def numbering_problems(root: Path) -> list[str]:
    path = root / "AGENTS.md"
    if not path.is_file():
        return [f"{path.name} is missing"]
    text = path.read_text(encoding="utf-8")
    heading = HEADING.search(text)
    if heading is None:
        return ["AGENTS.md has no '## Architecture rules' section"]
    rest = text[heading.end() :]
    following = NEXT_HEADING.search(rest)
    section = rest[: following.start()] if following else rest
    numbers = [int(number) for number in NUMBERED.findall(section)]
    if not numbers:
        return ["AGENTS.md has no numbered architecture rules"]
    expected = list(range(1, len(numbers) + 1))
    if numbers != expected:
        return [
            "the architecture rules in AGENTS.md read "
            f"{', '.join(str(number) for number in numbers)} and must read "
            f"{', '.join(str(number) for number in expected)}"
        ]
    return []


def value_at(loaded: object, keys: tuple[str, ...]) -> tuple[bool, object]:
    current = loaded
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            return False, None
        current = current[key]
    return True, current


def settings_problems(root: Path) -> list[str]:
    path = root / SETTINGS
    if not path.is_file():
        return [f"{SETTINGS} is missing, and it is the only settings file this repository has"]
    try:
        loaded: object = json.loads(path.read_text(encoding="utf-8"))
    except ValueError:
        return [f"{SETTINGS} is not valid JSON"]
    found: list[str] = []
    for keys, expected in REQUIRED_SETTINGS:
        name = ".".join(keys)
        present, actual = value_at(loaded, keys)
        if not present:
            found.append(f"{SETTINGS} has no {name}, and it must be {json.dumps(expected)}")
            continue
        if type(actual) is not type(expected) or actual != expected:
            found.append(
                f"{SETTINGS} sets {name} to {json.dumps(actual)} and it must be "
                f"{json.dumps(expected)}; a value of the wrong type is discarded in "
                "silence. → ADR 0001, D9."
            )
    return found


def tooling_commands(root: Path) -> tuple[list[str], str | None]:
    path = root / "AGENTS.md"
    if not path.is_file():
        return [], "AGENTS.md is missing"
    block = TOOLING.search(path.read_text(encoding="utf-8"))
    if block is None:
        return [], "AGENTS.md has no bash block under '## Tooling'"
    return [line.strip() for line in block.group(1).splitlines() if line.strip()], None


def workflow_commands(root: Path) -> tuple[list[str], str | None]:
    path = root / WORKFLOW
    if not path.is_file():
        return [], f"{WORKFLOW} is missing"
    job = CHECK_JOB.search(path.read_text(encoding="utf-8"))
    if job is None:
        return [], f"{WORKFLOW} has no 'check' job"
    return RUN_STEP.findall(job.group(1)), None


def parity_problems(root: Path) -> list[str]:
    stated, absent = tooling_commands(root)
    running, missing = workflow_commands(root)
    if absent is not None or missing is not None:
        return [problem for problem in (absent, missing) if problem is not None]
    if stated[:1] != [LOCAL_SYNC]:
        return [f"AGENTS.md's '## Tooling' block must open with {LOCAL_SYNC!r}"]
    if running[:1] != [WORKFLOW_SYNC]:
        return [f"{WORKFLOW}'s check job must open with {WORKFLOW_SYNC!r}"]
    if stated[1:] != running[1:]:
        return [
            f"the five checks in AGENTS.md and in {WORKFLOW} have drifted. AGENTS.md runs "
            f"{stated[1:]} and the check job runs {running[1:]}. They are compared for "
            "equality, so a flag on one side has to be added to the other or dropped."
        ]
    return []


def gate_problems(root: Path) -> list[str]:
    path = root / GATE
    if not path.is_file():
        return [f"{GATE} is missing, and it is the gate on a PR's closing references"]
    declared = path.read_text(encoding="utf-8")
    return [
        f"{GATE} does not grant {permission}. Without it the GraphQL call for "
        "closingIssuesReferences succeeds and returns an empty list, so the gate passes "
        "every pull request in silence. → ADR 0001, D3."
        for permission in GATE_PERMISSIONS
        if permission not in declared
    ]


def glob_problems(root: Path) -> list[str]:
    found: list[str] = []
    for rule in rule_files(root):
        text = rule.read_text(encoding="utf-8")
        if not is_path_scoped(text):
            continue
        name = rule.relative_to(root)
        patterns = globs(text)
        if not patterns:
            found.append(f"{name} has a paths: key with no glob under it, so it can never load")
            continue
        found += [
            f"{name} is scoped to {pattern!r}, which matches no file. A rule whose glob "
            "matches nothing is silently absent — fix the glob, or delete the rule."
            for pattern in patterns
            if not matches_a_file(root, pattern)
        ]
    return found


def skill_problems(root: Path) -> list[str]:
    directory = root / SKILLS
    if not directory.is_dir():
        return [f"{SKILLS} is missing, and AGENTS.md names the procedures that live there"]
    found: list[str] = []
    for skill in skill_files(root):
        name = skill.relative_to(root)
        block = frontmatter(skill.read_text(encoding="utf-8"))
        if block is None:
            found.append(f"{name} has no frontmatter, so the client cannot list it")
            continue
        declared = NAME_KEY.search(block)
        if declared is None or declared.group(1) != skill.parent.name:
            stated = "nothing" if declared is None else repr(declared.group(1))
            found.append(
                f"{name} declares name: {stated} and sits in {skill.parent.name}/. "
                "A skill is invoked by its directory, so the two must agree."
            )
        description = DESCRIPTION_KEY.search(block)
        if description is None or not description.group(1):
            found.append(
                f"{name} has no description:, which is the line the client reads to decide "
                "whether to load the skill at all."
            )
    return found


def problems(root: Path) -> list[str]:
    return (
        length_problems(root)
        + combined_problems(root)
        + on_demand_problems(root)
        + width_problems(root)
        + manual_steps_problems(root)
        + numbering_problems(root)
        + settings_problems(root)
        + parity_problems(root)
        + gate_problems(root)
        + glob_problems(root)
        + skill_problems(root)
    )


def main(argv: list[str]) -> int:
    root = Path(argv[1]).resolve() if len(argv) > 1 else Path.cwd()
    for line in gauge(root):
        print(line)
    found = problems(root)
    for problem in found:
        print(problem, file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
