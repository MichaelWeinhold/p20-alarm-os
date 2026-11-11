from __future__ import annotations

from pathlib import Path

import pytest

from check_dependencies import QUESTIONS, main, packages, problems, sections

REPO_ROOT = Path(__file__).resolve().parents[1]
BASE = "a" * 40
HEAD = "b" * 40
BEFORE = """
[project]
name = "p20-alarm-os-tools"
dependencies = ["httpx>=0.28.1", "pydantic>=2.13"]

[dependency-groups]
dev = ["pytest>=9.1", "ruff>=0.16.5"]
"""


def answered(name: str, skip: frozenset[int] = frozenset()) -> str:
    answers = [
        f"{position}. **{question}.** An answer."
        for position, question in enumerate(QUESTIONS, 1)
        if position not in skip
    ]
    return "\n".join([f"## Dependency: {name}", "", *answers])


CRATE = """
[package]
name = "alarmd"

[dependencies]
alarm-core = { path = "../alarm-core" }
libc = "0.2"

[dev-dependencies]
proptest = { workspace = true }
"""


def python(manifest: str) -> dict[str, str]:
    return {"pyproject.toml": manifest}


def run(
    after: str | dict[str, str], body: str | None, before: str | dict[str, str] = BEFORE
) -> int:
    manifests = {
        BASE: python(before) if isinstance(before, str) else before,
        HEAD: python(after) if isinstance(after, str) else after,
    }
    payload = {"number": 610, "body": body, "baseRefOid": BASE, "headRefOid": HEAD}
    return main(
        ["check_dependencies.py", "610", "."],
        lambda _repo, _number: payload,
        lambda _repo, revision: manifests[revision],
    )


def test_a_new_package_without_its_section_fails(capsys: pytest.CaptureFixture[str]) -> None:
    after = BEFORE.replace('"pydantic>=2.13"', '"pydantic>=2.13", "Rich>=14"')
    assert run(after, "Closes #609\n\n## Summary\n\nAdds rich.") == 1
    captured = capsys.readouterr()
    assert captured.out == "PR #610 adds rich and removes none\n"
    assert "'## Dependency: rich'" in captured.err
    assert "on the host only, or on the phone" in captured.err


def test_a_new_package_with_its_section_passes() -> None:
    after = BEFORE.replace('"pydantic>=2.13"', '"pydantic>=2.13", "rich>=14"')
    assert run(after, f"## Summary\n\nAdds rich.\n\n{answered('rich')}\n") == 0


def test_a_new_dev_package_without_its_section_fails() -> None:
    after = BEFORE.replace('"ruff>=0.16.5"', '"ruff>=0.16.5", "hypothesis>=6"')
    assert run(after, "") == 1


def test_a_version_bump_passes() -> None:
    after = BEFORE.replace("pydantic>=2.13", "pydantic>=2.14").replace("ruff>=0.16.5", "ruff>=1")
    assert run(after, None) == 0


def test_a_removal_passes(capsys: pytest.CaptureFixture[str]) -> None:
    assert run(BEFORE.replace(', "pydantic>=2.13"', ""), "") == 0
    assert capsys.readouterr().out == "PR #610 adds no package and removes pydantic\n"


def test_a_package_moved_between_groups_is_not_new() -> None:
    after = BEFORE.replace(', "pydantic>=2.13"', "").replace(
        '"ruff>=0.16.5"', '"ruff>=0.16.5", "pydantic>=2.13"'
    )
    assert run(after, "") == 0


def test_a_section_missing_an_answer_fails(capsys: pytest.CaptureFixture[str]) -> None:
    after = BEFORE.replace('"pydantic>=2.13"', '"pydantic>=2.13", "rich>=14"')
    assert run(after, answered("rich", frozenset({5}))) == 1
    assert "5. What it pulls in" in capsys.readouterr().err


def test_a_section_for_another_package_does_not_count() -> None:
    after = BEFORE.replace('"pydantic>=2.13"', '"pydantic>=2.13", "rich>=14"')
    assert run(after, answered("textual")) == 1


def test_answers_under_a_later_heading_do_not_count() -> None:
    after = BEFORE.replace('"pydantic>=2.13"', '"pydantic>=2.13", "rich>=14"')
    body = "## Dependency: rich\n\n## Test plan\n\n" + answered("x").split("\n", 2)[2]
    assert run(after, body) == 1


def test_a_subheading_inside_a_section_keeps_it_open() -> None:
    body = answered("rich").replace("4.", "### Licence\n\n4.")
    assert sections(body) == {"rich": frozenset(range(1, 7))}


@pytest.mark.parametrize(
    ("added", "heading"),
    [
        pytest.param("python_dotenv", "## Dependency: python-dotenv", id="underscore"),
        pytest.param("Python.Dotenv", "## Dependency: `python-dotenv`", id="dot and backticks"),
    ],
)
def test_names_are_compared_normalised(added: str, heading: str) -> None:
    after = BEFORE.replace('"pydantic>=2.13"', f'"pydantic>=2.13", "{added}[cli]>=1.2"')
    body = answered("x").replace("## Dependency: x", heading)
    assert run(after, body) == 0


def test_an_optional_dependency_counts() -> None:
    after = BEFORE.replace(
        "[dependency-groups]",
        '[project.optional-dependencies]\npi = ["gpiozero>=2"]\n\n[dependency-groups]',
    )
    added = packages(python(after), "head") - packages(python(BEFORE), "base")
    assert added == {"gpiozero"}
    assert problems(added, "")


def test_an_included_group_is_not_a_package() -> None:
    after = BEFORE + 'lint = [{include-group = "dev"}, "ruff>=0.16.5"]\n'
    assert run(after, "") == 0


@pytest.mark.parametrize(
    "manifest",
    [
        "[project\n",
        '[project]\ndependencies = "httpx"\n',
        "[project]\ndependencies = [3]\n",
        '[project]\ndependencies = [">=1"]\n',
        '[dependency-groups]\ndev = "pytest"\n',
    ],
)
def test_a_malformed_manifest_exits_two(manifest: str) -> None:
    assert run(manifest, "") == 2


@pytest.mark.parametrize(
    "payload",
    [
        [],
        {"number": "610", "body": "", "baseRefOid": BASE, "headRefOid": HEAD},
        {"number": 610, "body": 3, "baseRefOid": BASE, "headRefOid": HEAD},
        {"number": 610, "body": "", "headRefOid": HEAD},
    ],
)
def test_a_malformed_payload_exits_two(payload: object) -> None:
    assert main(["check_dependencies.py", "610"], lambda _r, _n: payload, lambda _r, _v: {}) == 2


@pytest.mark.parametrize("argv", [["check_dependencies.py"], ["check_dependencies.py", "main"]])
def test_a_missing_or_unreadable_number_exits_two(argv: list[str]) -> None:
    assert main(argv, lambda _r, _n: {}, lambda _r, _v: {}) == 2


def test_a_new_crate_without_its_section_fails(capsys: pytest.CaptureFixture[str]) -> None:
    before = {"pyproject.toml": BEFORE, "crates/alarmd/Cargo.toml": CRATE}
    after = {
        **before,
        "crates/alarmd/Cargo.toml": CRATE.replace('libc = "0.2"', 'libc = "0.2"\nevdev = "0.13"'),
    }
    assert run(after, "", before) == 1
    assert "'## Dependency: evdev'" in capsys.readouterr().err


def test_a_path_dependency_is_not_a_package() -> None:
    found = packages({"crates/alarmd/Cargo.toml": CRATE}, "head")
    assert found == {"libc", "proptest"}


def test_a_renamed_crate_is_its_package_name() -> None:
    manifest = '[dependencies]\nnix2 = { package = "nix", version = "0.29" }\n'
    assert packages({"Cargo.toml": manifest}, "head") == {"nix"}


def test_workspace_and_target_tables_count() -> None:
    manifest = (
        '[workspace.dependencies]\nproptest = "1"\n\n'
        '[target.\'cfg(target_os = "linux")\'.dependencies]\nnix = "0.29"\n'
    )
    assert packages({"Cargo.toml": manifest}, "head") == {"proptest", "nix"}


def test_a_crate_moved_into_the_workspace_table_is_not_new() -> None:
    before = {"crates/alarmd/Cargo.toml": CRATE}
    after = {
        "Cargo.toml": '[workspace.dependencies]\nlibc = "0.2"\n',
        "crates/alarmd/Cargo.toml": CRATE.replace('libc = "0.2"', "libc = { workspace = true }"),
    }
    assert run(after, "", before) == 0


def test_the_real_manifests_parse() -> None:
    manifests = {
        path.relative_to(REPO_ROOT).as_posix(): path.read_text(encoding="utf-8")
        for path in [REPO_ROOT / "pyproject.toml", *REPO_ROOT.glob("**/Cargo.toml")]
        if "target" not in path.relative_to(REPO_ROOT).parts
    }
    assert {"pytest", "ruff", "mypy"} <= packages(manifests, "worktree")
