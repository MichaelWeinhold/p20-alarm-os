from __future__ import annotations

import json
from pathlib import Path

import pytest

from check_instructions import (
    COMBINED_LIMIT,
    GATE_PERMISSIONS,
    LIMIT,
    MANUAL_STEPS,
    MANUAL_STEPS_DIR,
    MANUAL_STEPS_FILE_CEILING,
    MANUAL_STEPS_INDEX_CEILING,
    ON_DEMAND_LIMIT,
    PROSE_WIDTH,
    RULE_LIMIT,
    SKILL_LIMIT,
    always_on_files,
    line_count,
    main,
    problems,
)

RULES = """1. **One.**
2. **Two.**
3. **Three.**
"""

CHECKS = (
    "uv run ruff check .",
    "uv run pytest",
    "uv run python scripts/check_instructions.py",
)

SETTINGS: dict[str, object] = {
    "includeGitInstructions": False,
    "attribution": {"commit": "", "pr": "", "sessionUrl": False},
    "autoCompactWindow": 200_000,
}


def write_settings(root: Path, settings: object) -> None:
    path = root / ".claude" / "settings.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(settings, indent=2) + "\n", encoding="utf-8")


def write_workflow(root: Path, commands: tuple[str, ...], sync: str = "uv sync --locked") -> None:
    path = root / ".github" / "workflows" / "checks.yml"
    path.parent.mkdir(parents=True, exist_ok=True)
    steps = "".join(
        f"      - name: Step {number}\n        run: {command}\n"
        for number, command in enumerate((sync, *commands))
    )
    path.write_text(
        "jobs:\n  check:\n    runs-on: ubuntu-24.04\n    steps:\n"
        + steps
        + "\n  secrets:\n    runs-on: ubuntu-24.04\n    steps:\n"
        + "      - name: Secrets\n        run: |\n          git ls-files\n",
        encoding="utf-8",
    )


def write_gate(root: Path, permissions: tuple[str, ...] = GATE_PERMISSIONS) -> None:
    path = root / ".github" / "workflows" / "pr-closes.yml"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "name: PR closing references\n\npermissions:\n"
        + "".join(f"  {permission}\n" for permission in permissions)
        + "\njobs:\n  closing-references:\n    runs-on: ubuntu-24.04\n    steps: []\n",
        encoding="utf-8",
    )


def write_skill(root: Path, directory: str, name: str, description: str) -> None:
    path = root / ".claude" / "skills" / directory / "SKILL.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        f"---\nname: {name}\ndescription: {description}\n---\n\n# The procedure\n",
        encoding="utf-8",
    )


def instruction_tree(
    root: Path,
    *,
    rules: str = RULES,
    filler: int = 10,
    commands: tuple[str, ...] = CHECKS,
    sync: str = "uv sync",
) -> Path:
    (root / "AGENTS.md").write_text(
        "# AGENTS.md\n\n"
        + "\n".join(f"line {n}" for n in range(filler))
        + "\n\n## Architecture rules\n\n"
        + rules
        + "\n## Tooling\n\n```bash\n"
        + "\n".join((sync, *commands))
        + "\n```\n",
        encoding="utf-8",
    )
    (root / "CLAUDE.md").write_text("@AGENTS.md\n", encoding="utf-8")
    write_settings(root, SETTINGS)
    write_workflow(root, CHECKS)
    write_gate(root)
    write_skill(root, "feature-lifecycle", "feature-lifecycle", "the Definition of Done")
    return root


def test_a_healthy_tree_passes(tmp_path: Path) -> None:
    assert problems(instruction_tree(tmp_path)) == []
    assert main(["check_instructions.py", str(tmp_path)]) == 0


def test_the_real_repository_passes() -> None:
    assert problems(Path(__file__).resolve().parents[1]) == []


def test_a_gap_in_the_numbering_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path, rules="1. **One.**\n2. **Two.**\n8. **Eight.**\n")
    found = problems(root)
    assert len(found) == 1
    assert "1, 2, 8" in found[0]
    assert "1, 2, 3" in found[0]


def test_a_rule_appended_into_the_middle_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path, rules="1. **One.**\n8. **Eight.**\n2. **Two.**\n")
    assert "1, 8, 2" in problems(root)[0]


def test_an_over_long_instruction_file_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path, filler=LIMIT + 1)
    found = problems(root)
    assert len(found) == 1
    assert "loads on every turn" in found[0]
    assert main(["check_instructions.py", str(root)]) == 1


def test_a_file_exactly_at_the_limit_passes(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    agents = root / "AGENTS.md"
    body = agents.read_text(encoding="utf-8").splitlines()
    agents.write_text("\n".join(body + ["x"] * (LIMIT - len(body))) + "\n", encoding="utf-8")
    assert len(agents.read_text(encoding="utf-8").splitlines()) == LIMIT
    assert problems(root) == []


def test_an_unscoped_rule_counts_as_always_on(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "tests").mkdir()
    (root / "tests" / "test_thing.py").write_text("", encoding="utf-8")
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "scoped.md").write_text(
        '---\npaths:\n  - "tests/**"\n---\n\n' + "x\n" * (LIMIT - 50), encoding="utf-8"
    )
    (rules / "unscoped.md").write_text("# Loaded always\n" + "x\n" * (LIMIT + 5), encoding="utf-8")

    names = [path.name for path in always_on_files(root)]
    assert "unscoped.md" in names
    assert "scoped.md" not in names

    found = problems(root)
    assert len(found) == 1
    assert "unscoped.md" in found[0]


def test_frontmatter_without_paths_is_not_scoped(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "titled.md").write_text(
        "---\ndescription: a rule\n---\n\n" + "x\n" * (LIMIT + 5), encoding="utf-8"
    )
    assert "titled.md" in [path.name for path in always_on_files(root)]


def test_a_missing_architecture_section_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "AGENTS.md").write_text("# AGENTS.md\n\nnothing here\n", encoding="utf-8")
    assert "no '## Architecture rules' section" in problems(root)[0]


def test_a_missing_agents_file_fails(tmp_path: Path) -> None:
    assert "AGENTS.md is missing" in problems(tmp_path)


def test_numbers_outside_the_section_are_ignored(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    agents = root / "AGENTS.md"
    agents.write_text(
        agents.read_text(encoding="utf-8") + "\n## Later\n\n7. **Seven.**\n9. **Nine.**\n",
        encoding="utf-8",
    )
    assert problems(root) == []


def test_an_attribution_key_of_the_wrong_type_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_settings(
        root,
        {
            "includeGitInstructions": False,
            "attribution": {"commit": False, "pr": "", "sessionUrl": False},
            "autoCompactWindow": 200_000,
        },
    )
    found = problems(root)
    assert len(found) == 1
    assert "attribution.commit" in found[0]
    assert "ADR 0001, D9" in found[0]


def test_a_setting_with_the_wrong_value_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_settings(
        root,
        {
            "includeGitInstructions": True,
            "attribution": {"commit": "", "pr": "", "sessionUrl": False},
            "autoCompactWindow": 200_000,
        },
    )
    found = problems(root)
    assert len(found) == 1
    assert "includeGitInstructions" in found[0]


def test_a_false_for_a_string_key_is_not_read_as_empty(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_settings(
        root,
        {
            "includeGitInstructions": 0,
            "attribution": {"commit": "", "pr": "", "sessionUrl": 0},
            "autoCompactWindow": 200_000,
        },
    )
    found = problems(root)
    assert len(found) == 2
    assert all("wrong type is discarded" in problem for problem in found)


def test_a_missing_settings_key_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_settings(
        root,
        {
            "includeGitInstructions": False,
            "attribution": {"commit": ""},
            "autoCompactWindow": 200_000,
        },
    )
    found = problems(root)
    assert len(found) == 2
    assert all("has no attribution." in problem for problem in found)


def test_a_missing_compaction_window_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_settings(
        root, {key: value for key, value in SETTINGS.items() if key != "autoCompactWindow"}
    )
    found = problems(root)
    assert len(found) == 1
    assert "has no autoCompactWindow" in found[0]


@pytest.mark.parametrize("window", [967_000, "200000", 200_000.0])
def test_a_changed_compaction_window_fails(tmp_path: Path, window: object) -> None:
    root = instruction_tree(tmp_path)
    write_settings(root, {**SETTINGS, "autoCompactWindow": window})
    found = problems(root)
    assert len(found) == 1
    assert "autoCompactWindow" in found[0]


def test_a_missing_settings_file_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / ".claude" / "settings.json").unlink()
    assert "settings.json is missing" in problems(root)[0]


def test_a_drifted_command_list_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_workflow(root, ("uv run ruff check .", "uv run pytest -q", *CHECKS[2:]))
    found = problems(root)
    assert len(found) == 1
    assert "drifted" in found[0]
    assert "uv run pytest -q" in found[0]


def test_an_extra_workflow_step_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_workflow(root, (*CHECKS, "uv run python scripts/pi_preflight.py"))
    assert "drifted" in problems(root)[0]


def test_a_bare_sync_in_the_workflow_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_workflow(root, CHECKS, sync="uv sync")
    found = problems(root)
    assert len(found) == 1
    assert "must open with 'uv sync --locked'" in found[0]


def test_a_locked_sync_in_agents_md_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path, sync="uv sync --locked")
    found = problems(root)
    assert len(found) == 1
    assert "must open with 'uv sync'" in found[0]


@pytest.mark.parametrize("workflow", ["checks.yml", "pr-closes.yml"])
def test_a_missing_workflow_fails(tmp_path: Path, workflow: str) -> None:
    root = instruction_tree(tmp_path)
    (root / ".github" / "workflows" / workflow).unlink()
    assert f"{workflow} is missing" in problems(root)[0]


def test_the_gate_without_issues_read_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_gate(root, ("contents: read", "pull-requests: read"))
    found = problems(root)
    assert len(found) == 1
    assert "issues: read" in found[0]
    assert "passes every pull request in silence" in found[0]


def test_the_gate_without_pull_requests_read_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_gate(root, ("contents: read", "issues: read"))
    assert "pull-requests: read" in problems(root)[0]


def test_a_glob_matching_nothing_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "scoped.md").write_text(
        '---\npaths:\n  - "src/home_platform/bus/**"\n---\n\n# A rule\n', encoding="utf-8"
    )
    found = problems(root)
    assert len(found) == 1
    assert "src/home_platform/bus/**" in found[0]
    assert "silently absent" in found[0]


def test_a_glob_matching_a_directory_only_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "src").mkdir()
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "scoped.md").write_text('---\npaths:\n  - "src/**"\n---\n\n# A rule\n', "utf-8")
    assert "matches no file" in problems(root)[0]


def test_a_glob_matching_one_file_passes(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "src" / "home_platform" / "bus").mkdir(parents=True)
    (root / "src" / "home_platform" / "bus" / "contracts.py").write_text("", encoding="utf-8")
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "scoped.md").write_text(
        '---\npaths:\n  - "src/home_platform/bus/**"\n---\n\n# A rule\n', encoding="utf-8"
    )
    assert problems(root) == []


def test_a_paths_key_with_no_glob_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "scoped.md").write_text("---\npaths:\n---\n\n# A rule\n", encoding="utf-8")
    assert "no glob under it" in problems(root)[0]


def test_a_skill_whose_name_differs_from_its_directory_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_skill(root, "promoting-to-production", "promoting", "how a promotion happens")
    found = problems(root)
    assert len(found) == 1
    assert "promoting-to-production/SKILL.md" in found[0]
    assert "invoked by its directory" in found[0]


def test_a_skill_with_an_empty_description_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_skill(root, "grooming-an-issue", "grooming-an-issue", "")
    found = problems(root)
    assert len(found) == 1
    assert "has no description:" in found[0]


def test_a_skill_without_frontmatter_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    path = root / ".claude" / "skills" / "using-the-raspberry-pi" / "SKILL.md"
    path.parent.mkdir(parents=True)
    path.write_text("# No frontmatter\n", encoding="utf-8")
    assert "no frontmatter" in problems(root)[0]


def test_a_missing_skills_directory_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / ".claude" / "skills" / "feature-lifecycle" / "SKILL.md").unlink()
    (root / ".claude" / "skills" / "feature-lifecycle").rmdir()
    (root / ".claude" / "skills").rmdir()
    assert "skills is missing" in problems(root)[0]


def pad_to(path: Path, count: int) -> None:
    body = path.read_text(encoding="utf-8").splitlines()
    assert len(body) <= count
    path.write_text("\n".join(body + ["x"] * (count - len(body))) + "\n", encoding="utf-8")


def test_the_gauge_reports_every_always_on_file_and_the_combined_total(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", 120)
    pad_to(root / "CLAUDE.md", 30)
    assert main(["check_instructions.py", str(root)]) == 0
    captured = capsys.readouterr()
    assert captured.out.splitlines() == [
        f"AGENTS.md 120/{LIMIT} (80 lines of headroom)",
        f"CLAUDE.md 30/{LIMIT} (170 lines of headroom)",
        f"always-on total 150/{COMBINED_LIMIT} ({COMBINED_LIMIT - 150} lines of headroom)",
        f".claude/skills/feature-lifecycle/SKILL.md 2/{SKILL_LIMIT} "
        f"({SKILL_LIMIT - 2} lines of headroom)",
        f"on-demand total 2/{ON_DEMAND_LIMIT} ({ON_DEMAND_LIMIT - 2} lines of headroom)",
    ]
    assert captured.err == ""


def test_the_gauge_counts_an_unscoped_rule_into_the_total(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", 100)
    pad_to(root / "CLAUDE.md", 20)
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "unscoped.md").write_text("x\n" * 40, encoding="utf-8")
    assert main(["check_instructions.py", str(root)]) == 0
    reported = capsys.readouterr().out.splitlines()
    assert f".claude/rules/unscoped.md 40/{LIMIT} (160 lines of headroom)" in reported
    assert f"always-on total 160/{COMBINED_LIMIT} (110 lines of headroom)" in reported


def test_a_file_with_no_headroom_reports_zero_rather_than_passing_silently(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", LIMIT)
    assert main(["check_instructions.py", str(root)]) == 0
    assert f"AGENTS.md {LIMIT}/{LIMIT} (0 lines of headroom)" in capsys.readouterr().out


def test_one_line_of_headroom_is_singular(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", LIMIT - 1)
    assert main(["check_instructions.py", str(root)]) == 0
    assert f"AGENTS.md {LIMIT - 1}/{LIMIT} (1 line of headroom)" in capsys.readouterr().out


def test_an_over_long_file_reports_how_far_over_it_is(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path, filler=LIMIT + 1)
    assert main(["check_instructions.py", str(root)]) == 1
    captured = capsys.readouterr()
    assert "AGENTS.md" in captured.out
    assert "over by" in captured.out
    assert "loads on every turn" in captured.err


def test_the_combined_ceiling_fails_while_every_file_is_under_the_per_file_limit(
    tmp_path: Path,
) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", LIMIT - 10)
    pad_to(root / "CLAUDE.md", COMBINED_LIMIT - (LIMIT - 10) + 1)
    assert line_count(root / "AGENTS.md") <= LIMIT
    assert line_count(root / "CLAUDE.md") <= LIMIT
    found = problems(root)
    assert len(found) == 1
    assert f"the always-on set is {COMBINED_LIMIT + 1} lines" in found[0]
    assert "splitting one file in two" in found[0]
    assert "AGENTS.md, CLAUDE.md" in found[0]


def test_the_combined_total_exactly_at_the_ceiling_passes(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", LIMIT - 10)
    pad_to(root / "CLAUDE.md", COMBINED_LIMIT - (LIMIT - 10))
    assert sum(line_count(root / name) for name in ("AGENTS.md", "CLAUDE.md")) == COMBINED_LIMIT
    assert problems(root) == []


def test_a_split_file_still_trips_the_combined_ceiling(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    pad_to(root / "AGENTS.md", LIMIT)
    rules = root / ".claude" / "rules"
    rules.mkdir(parents=True)
    (rules / "half.md").write_text("x\n" * LIMIT, encoding="utf-8")
    assert all(line_count(path) <= LIMIT for path in always_on_files(root))
    assert "the always-on set is" in problems(root)[0]


def test_a_missing_claude_md_is_a_failure_not_a_lower_total(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "CLAUDE.md").unlink()
    found = problems(root)
    assert len(found) == 1
    assert "CLAUDE.md is missing" in found[0]
    assert "combined ceiling is measured over" in found[0]


def test_the_real_repository_is_inside_the_combined_ceiling(
    capsys: pytest.CaptureFixture[str],
) -> None:
    root = Path(__file__).resolve().parents[1]
    assert main(["check_instructions.py", str(root)]) == 0
    reported = capsys.readouterr().out.splitlines()
    assert reported[0].startswith("AGENTS.md ")
    assert "over by" not in "\n".join(reported)
    assert any(line.startswith("always-on total ") for line in reported)
    assert any(line.startswith("on-demand total ") for line in reported)
    index = next(line for line in reported if line.startswith("docs/MANUAL_STEPS.md "))
    assert index.startswith(f"docs/MANUAL_STEPS.md {line_count(root / MANUAL_STEPS)}/")
    assert reported[-1].startswith("docs/manual-steps/ total ")


def write_manual_steps(root: Path, name: str, body: str) -> None:
    path = root / MANUAL_STEPS_DIR / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body, encoding="utf-8")


def write_index(root: Path, count: int) -> None:
    path = root / MANUAL_STEPS
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("x\n" * count, encoding="utf-8")


def test_the_gauge_reports_each_file_with_its_open_boxes_and_the_total(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    write_index(root, 40)
    write_manual_steps(root, "broker.md", "# Broker\n- [ ] one\n  - [ ] two\n- [x] done\n")
    write_manual_steps(root, "host.md", "x\n" * (MANUAL_STEPS_FILE_CEILING - 30))
    assert main(["check_instructions.py", str(root)]) == 0
    assert capsys.readouterr().out.splitlines()[-4:] == [
        f"docs/MANUAL_STEPS.md 40/{MANUAL_STEPS_INDEX_CEILING} "
        f"({MANUAL_STEPS_INDEX_CEILING - 40} lines of headroom)",
        f"docs/manual-steps/broker.md 4/{MANUAL_STEPS_FILE_CEILING} "
        f"({MANUAL_STEPS_FILE_CEILING - 4} lines of headroom), 2 open",
        f"docs/manual-steps/host.md {MANUAL_STEPS_FILE_CEILING - 30}/"
        f"{MANUAL_STEPS_FILE_CEILING} (30 lines of headroom), 0 open",
        f"docs/manual-steps/ total {MANUAL_STEPS_FILE_CEILING - 26} lines, 2 open",
    ]


def test_a_file_exactly_at_the_ceiling_passes(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_index(root, MANUAL_STEPS_INDEX_CEILING)
    write_manual_steps(root, "host.md", "x\n" * MANUAL_STEPS_FILE_CEILING)
    assert problems(root) == []


def test_the_total_is_never_failed_on(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    for number in range(10):
        write_manual_steps(root, f"s{number}.md", "x\n" * MANUAL_STEPS_FILE_CEILING)
    assert problems(root) == []


def test_a_file_one_line_over_the_ceiling_fails_naming_the_moves(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    write_manual_steps(root, "host.md", "x\n" * (MANUAL_STEPS_FILE_CEILING + 1))
    assert main(["check_instructions.py", str(root)]) == 1
    captured = capsys.readouterr()
    assert "over by 1 line)" in captured.out
    assert (
        f"docs/manual-steps/host.md is {MANUAL_STEPS_FILE_CEILING + 1} lines and the ceiling "
        f"is {MANUAL_STEPS_FILE_CEILING}"
    ) in captured.err
    for move in ("docs/device-history/", "README.md", "a file of its own"):
        assert move in captured.err
    assert "shorten" not in captured.err


def test_the_index_one_line_over_its_ceiling_fails(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    write_index(root, MANUAL_STEPS_INDEX_CEILING + 1)
    assert main(["check_instructions.py", str(root)]) == 1
    assert (
        f"docs/MANUAL_STEPS.md is {MANUAL_STEPS_INDEX_CEILING + 1} lines and the ceiling is "
        f"{MANUAL_STEPS_INDEX_CEILING}"
    ) in capsys.readouterr().err


def write_long_skill(root: Path, directory: str, body: int) -> Path:
    path = root / ".claude" / "skills" / directory / "SKILL.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        f"---\nname: {directory}\ndescription: a procedure\n---\n" + "x\n" * body,
        encoding="utf-8",
    )
    return path


def write_scoped_rule(root: Path, count: int, text: str = "") -> Path:
    (root / "tests").mkdir(exist_ok=True)
    (root / "tests" / "test_thing.py").write_text("", encoding="utf-8")
    path = root / ".claude" / "rules" / "testing.md"
    path.parent.mkdir(parents=True, exist_ok=True)
    head = '---\npaths:\n  - "tests/**"\n---\n' + text
    path.write_text(head + "x\n" * (count - len(head.splitlines())), encoding="utf-8")
    return path


def test_the_gauge_reports_every_skill_and_scoped_rule_with_its_headroom(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = instruction_tree(tmp_path)
    write_long_skill(root, "upgrading-the-pi", 100)
    write_scoped_rule(root, 40)
    assert main(["check_instructions.py", str(root)]) == 0
    reported = capsys.readouterr().out.splitlines()
    assert (
        f".claude/skills/upgrading-the-pi/SKILL.md 100/{SKILL_LIMIT} (400 lines of headroom)"
        in (reported)
    )
    assert f".claude/rules/testing.md 40/{RULE_LIMIT} (160 lines of headroom)" in reported
    assert f"on-demand total 142/{ON_DEMAND_LIMIT} (858 lines of headroom)" in reported


def test_a_skill_body_exactly_at_the_limit_passes_and_its_frontmatter_is_not_counted(
    tmp_path: Path,
) -> None:
    root = instruction_tree(tmp_path)
    path = write_long_skill(root, "upgrading-the-pi", SKILL_LIMIT)
    assert line_count(path) > SKILL_LIMIT
    assert problems(root) == []


def test_a_skill_body_over_the_limit_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_long_skill(root, "upgrading-the-pi", SKILL_LIMIT + 1)
    found = problems(root)
    assert len(found) == 1
    assert f"upgrading-the-pi/SKILL.md is {SKILL_LIMIT + 1} lines" in found[0]
    assert f"the limit is {SKILL_LIMIT}" in found[0]


def test_a_scoped_rule_over_the_limit_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_scoped_rule(root, RULE_LIMIT + 1)
    found = problems(root)
    assert len(found) == 1
    assert f".claude/rules/testing.md is {RULE_LIMIT + 1} lines" in found[0]


def test_the_on_demand_ceiling_fails_while_every_skill_is_under_its_limit(
    tmp_path: Path,
) -> None:
    root = instruction_tree(tmp_path)
    share = ON_DEMAND_LIMIT // 3 + 1
    assert share <= SKILL_LIMIT
    for directory in ("grooming-an-issue", "upgrading-the-pi", "using-the-raspberry-pi"):
        write_long_skill(root, directory, share)
    found = problems(root)
    assert len(found) == 1
    assert f"the on-demand ceiling is {ON_DEMAND_LIMIT}" in found[0]


def test_a_prose_line_over_the_width_fails_and_names_its_line(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    (root / "CLAUDE.md").write_text(
        "@AGENTS.md\n\n" + "w" * PROSE_WIDTH + "\n" + "w" * (PROSE_WIDTH + 1) + "\n",
        encoding="utf-8",
    )
    found = problems(root)
    assert len(found) == 1
    assert f"CLAUDE.md:4 is {PROSE_WIDTH + 1} characters" in found[0]


def test_a_wide_line_in_a_skill_or_rule_fails(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    write_scoped_rule(root, 10, "w" * (PROSE_WIDTH + 1) + "\n")
    path = write_long_skill(root, "upgrading-the-pi", 1)
    path.write_text(path.read_text(encoding="utf-8") + "w" * (PROSE_WIDTH + 1) + "\n", "utf-8")
    found = problems(root)
    assert len(found) == 2
    assert ".claude/rules/testing.md:5 is" in found[0]
    assert ".claude/skills/upgrading-the-pi/SKILL.md:6 is" in found[1]


def test_fences_tables_and_frontmatter_are_exempt_from_the_width(tmp_path: Path) -> None:
    root = instruction_tree(tmp_path)
    wide = "w" * (PROSE_WIDTH + 30)
    path = root / ".claude" / "skills" / "feature-lifecycle" / "SKILL.md"
    path.write_text(
        f"---\nname: feature-lifecycle\ndescription: {wide}\n---\n\n"
        f"```bash\n{wide}\n```\n\n| a | b |\n|---|---|\n| {wide} | x |\n",
        encoding="utf-8",
    )
    assert problems(root) == []
