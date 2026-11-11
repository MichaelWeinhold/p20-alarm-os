from __future__ import annotations

import json
from pathlib import Path

import pytest

from check_issue_umbrellas import (
    ICON,
    INFORMATION,
    Listing,
    issues_from,
    main,
    research_issues,
    review,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
ADRS = {"0005": 8, "0006": 66, "0007": 77}
CLAIM = "Implements **D3** of [ADR 0006](https://example.invalid/0006.md).\n\nThe body follows.\n"
LATE_MENTION = (
    "## Scope\n"
    "\n"
    "`docs/ARCHITECTURE.md` still describes the Pi's Go services as live, running\n"
    "infrastructure that this platform integrates with. They are not. Nothing outside this\n"
    "repository runs on the Pi — there is no space for it — and nothing outside it may be\n"
    "depended on. `CLAUDE.md`, `README.md`, `docs/MANUAL_STEPS.md` and ADR 0006 were all\n"
    "corrected for that when the notifier landed (#21); `ARCHITECTURE.md` was missed.\n"
)


def adr_tree(root: Path, adrs: dict[str, int] = ADRS) -> Path:
    directory = root / "docs" / "adr"
    directory.mkdir(parents=True, exist_ok=True)
    for number, issue in adrs.items():
        (directory / f"{number}-a-decision.md").write_text(
            f"# {int(number)}. A decision\n\n- **Status:** Accepted\n- **Issue:** #{issue}\n",
            encoding="utf-8",
        )
    return root


def entry(
    number: int,
    *,
    state: str = "OPEN",
    title: str = "Chore: a unit of work",
    parent: int | None = None,
    body: str = "",
    total: int = 0,
    completed: int = 0,
) -> dict[str, object]:
    return {
        "body": body,
        "number": number,
        "parent": None
        if parent is None
        else {
            "id": f"I_kwDO{parent}",
            "number": parent,
            "state": "OPEN",
            "title": f"{ICON}️ Research: a theme",
            "url": f"https://example.invalid/issues/{parent}",
        },
        "state": state,
        "subIssuesSummary": {"completed": completed, "percentCompleted": 0, "total": total},
        "title": title,
    }


def round_trip(entries: tuple[dict[str, object], ...]) -> object:
    parsed: object = json.loads(json.dumps(list(entries)))
    return parsed


def listing(*entries: dict[str, object]) -> Listing:
    def load(_: Path) -> object:
        return round_trip(entries)

    return load


def problems(*entries: dict[str, object], adrs: dict[str, int] = ADRS) -> list[str]:
    return review(issues_from(round_trip(entries)), adrs).problems


def information(*entries: dict[str, object], adrs: dict[str, int] = ADRS) -> list[str]:
    return review(issues_from(round_trip(entries)), adrs).information


def test_the_real_adr_directory_maps_every_number_to_its_research_issue() -> None:
    found = research_issues(REPO_ROOT)
    assert all(number.isdigit() and issue > 0 for number, issue in found.items())


def test_an_adr_with_no_issue_line_is_left_out_of_the_map(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    (root / "docs" / "adr" / "0008-undecided.md").write_text(
        "# 8. A decision with no issue\n\n- **Status:** Proposed\n", encoding="utf-8"
    )
    assert "0008" not in research_issues(root)


def test_a_correct_parent_is_not_reported() -> None:
    assert problems(entry(70, parent=66, body=CLAIM)) == []


def test_a_missing_parent_is_reported() -> None:
    found = problems(entry(70, body=CLAIM))
    assert len(found) == 1
    assert "#70" in found[0]
    assert "no parent" in found[0]
    assert "#66" in found[0]


def test_a_wrong_parent_is_reported() -> None:
    found = problems(entry(70, parent=8, body=CLAIM))
    assert len(found) == 1
    assert "#70 has parent #8" in found[0]
    assert "#66" in found[0]


def test_one_of_several_named_adrs_is_enough() -> None:
    body = "From **ADR 0005** (#8), and it waits on the rule from ADR 0006.\n"
    assert problems(entry(62, parent=8, body=body)) == []
    assert problems(entry(62, parent=66, body=body)) == []
    assert len(problems(entry(62, parent=77, body=body))) == 1


def test_an_issue_with_no_adr_reference_is_information_rather_than_a_problem() -> None:
    orphan = entry(16, title="Review the Pipeline Steps")
    assert problems(orphan) == []
    assert information(orphan) == ["#16 Review the Pipeline Steps"]


def test_an_adr_mentioned_past_the_opening_is_not_a_lineage_claim() -> None:
    late = entry(31, title="Fix: ARCHITECTURE.md describes the Go services", body=LATE_MENTION)
    assert problems(late) == []
    assert information(late) == ["#31 Fix: ARCHITECTURE.md describes the Go services"]


def test_a_research_issue_that_is_itself_open_needs_no_parent() -> None:
    title = f"{ICON}️ Research: whether follow-ups get an umbrella"
    umbrella = entry(
        77,
        title=title,
        body="Answers ADR 0005 and ADR 0006 as well.\n",
        total=4,
        completed=1,
    )
    assert problems(umbrella) == []
    assert information(umbrella) == [f"#77 {title}"]


def test_a_closed_parent_with_an_open_child_is_reported() -> None:
    closed = entry(66, state="CLOSED", title=f"{ICON} Research: a theme", total=4, completed=2)
    found = problems(closed)
    assert len(found) == 1
    assert "#66 is closed with 2 of its 4 sub-issues still open" in found[0]


def test_an_open_parent_whose_children_are_all_closed_is_reported() -> None:
    found = problems(entry(66, title=f"{ICON} Research: a theme", total=4, completed=4))
    assert len(found) == 1
    assert "owed a close" in found[0]


def test_a_closed_parent_whose_children_are_all_closed_is_correct() -> None:
    done = entry(40, state="CLOSED", title=f"{ICON} Research: a theme", total=6, completed=6)
    assert problems(done) == []


def test_a_parent_with_no_icon_is_reported() -> None:
    found = problems(entry(66, title="Research: a theme", total=4, completed=2))
    assert len(found) == 1
    assert "does not start with" in found[0]


def test_an_icon_with_no_children_is_reported() -> None:
    found = problems(entry(19, title=f"{ICON}️ Chore: not an umbrella"))
    assert len(found) == 1
    assert "holds no sub-issues" in found[0]


def test_an_icon_without_the_variation_selector_is_accepted() -> None:
    bare = entry(66, title=f"{ICON} Research: a theme", total=4, completed=2)
    rendered = entry(66, title=f"{ICON}️ Research: a theme", total=4, completed=2)
    assert problems(bare) == problems(rendered) == []


def test_main_is_clean_on_a_healthy_tracker(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    code = main(
        ["check_issue_umbrellas.py", str(root)],
        listing(
            entry(66, title=f"{ICON} Research: a theme", total=2, completed=1),
            entry(70, parent=66, body=CLAIM),
            entry(69, parent=66, state="CLOSED", body=CLAIM),
            entry(16, title="Review the Pipeline Steps"),
        ),
    )
    assert code == 0


def test_main_reports_the_parentless_issues_without_failing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = adr_tree(tmp_path)
    code = main(
        ["check_issue_umbrellas.py", str(root)],
        listing(
            entry(16, title="Review the Pipeline Steps"),
            entry(19, title="Daily briefing agent Skills"),
        ),
    )
    assert code == 0
    printed = capsys.readouterr().out
    assert INFORMATION in printed
    assert "#16 Review the Pipeline Steps" in printed
    assert "#19 Daily briefing agent Skills" in printed


def test_main_fails_on_an_orphan(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    root = adr_tree(tmp_path)
    assert main(["check_issue_umbrellas.py", str(root)], listing(entry(70, body=CLAIM))) == 1
    assert "#70" in capsys.readouterr().err


def test_main_fails_hard_on_a_payload_that_is_not_a_list(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)

    def load(_: Path) -> object:
        return {"number": 70}

    assert main(["check_issue_umbrellas.py", str(root)], load) == 2


def test_main_fails_hard_on_a_field_of_the_wrong_type(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    broken = entry(70, body=CLAIM)
    broken["number"] = "70"
    assert main(["check_issue_umbrellas.py", str(root)], listing(broken)) == 2


GROOMED = (
    "## Original request\n"
    "\n"
    "Found while working issue 59, the sudoers allowlist from ADR 0007 D8. Not asked for by\n"
    "anybody — it is a premise in an accepted ADR that issue 59 makes false.\n"
    "\n"
    "## What ADR 0005 D7 says\n"
    "\n"
    "`docs/adr/0005-platform-ui-frontend.md`, D7, on taking the viewer's identity from a\n"
    "forgeable header, rests on a premise the sudo allowlist makes false.\n"
)


@pytest.mark.parametrize(
    "parent",
    [
        pytest.param(8, id="judged on its subject, not its verbatim note"),
        pytest.param(77, id="a parent named only in the verbatim note is tolerated"),
    ],
)
def test_a_groomed_issue_is_judged_on_its_subject_not_its_verbatim_note(parent: int) -> None:
    groomed = entry(
        100,
        title="Chore: ADR 0005 D7 accepts a forgeable header on a false premise",
        parent=parent,
        body=GROOMED,
    )
    assert problems(groomed) == []


def test_a_parent_no_adr_in_the_body_was_researched_in_is_reported() -> None:
    groomed = entry(
        100,
        title="Chore: ADR 0005 D7 accepts a forgeable header on a false premise",
        parent=66,
        body=GROOMED,
    )
    found = problems(groomed)
    assert len(found) == 1
    assert "#100 has parent #66" in found[0]
    assert "declares #8" in found[0]


FOLLOW_UP = (
    "Implements **D5** of [ADR 0006](https://example.invalid/0006-a-decision.md).\n"
    "\n"
    "`AGENTS.md`, *Where a new rule goes*, states the standard. Four facts break it.\n"
)


def test_an_adr_follow_up_opening_with_its_reference_is_still_seen() -> None:
    assert problems(entry(226, parent=66, body=FOLLOW_UP)) == []
    found = problems(entry(226, parent=8, body=FOLLOW_UP))
    assert len(found) == 1
    assert "#226 has parent #8" in found[0]


def test_a_declaration_below_the_old_five_line_window_is_now_seen() -> None:
    body = (
        "## Original request\n"
        "\n"
        "A note with no ADR in it at all, three lines long, as grooming requires it\n"
        "to be kept verbatim.\n"
        "\n"
        "## Scope\n"
        "\n"
        "This carries out ADR 0006 D2.\n"
    )
    assert problems(entry(70, parent=66, body=body)) == []
    found = problems(entry(70, body=body))
    assert len(found) == 1
    assert "declares an ADR and has no parent" in found[0]
    assert "#66" in found[0]


def test_a_title_naming_the_adr_is_a_declaration() -> None:
    issue = entry(
        70,
        title="Fix: ADR 0006 D4 names a path that no longer exists",
        parent=66,
        body="## Original request\n\nSomebody said a path was wrong.\n",
    )
    assert problems(issue) == []
    assert information(issue) == []


def test_an_issue_with_nothing_after_the_verbatim_note_declares_nothing() -> None:
    issue = entry(
        70,
        title="Chore: tidy the pipeline steps",
        body="## Original request\n\nFound while reading ADR 0006. Worth a look.\n",
    )
    assert problems(issue) == []
    assert information(issue) == ["#70 Chore: tidy the pipeline steps"]


WALK_ADRS = {"0004": 52, "0010": 7, "0024": 476, "0029": 574}
WALK = (
    "## Original request\n"
    "\n"
    "Follow-up 1 of ADR 0029, D1, under #574.\n"
    "\n"
    "## Scope\n"
    "\n"
    "**Governed by:** architecture rules 5 and 8; ADR 0010, 0024.\n"
)
FINDING = (
    "## Original request\n"
    "\n"
    "Found on the walk.\n"
    "\n"
    "## Scope, as investigation found it\n"
    "\n"
    "ADR 0024 itself words this correctly.\n"
)


def walk(parent: int | None = 574) -> dict[str, object]:
    return entry(575, title=f"{ICON}️ Research: walk part 1", parent=parent, body=WALK, total=1)


def test_a_finding_under_a_walk_issue_is_not_reported() -> None:
    finding = entry(593, parent=575, body=FINDING)
    assert problems(walk(), finding, adrs=WALK_ADRS) == []


@pytest.mark.parametrize(
    "parent",
    [
        pytest.param(None, id="the walk has no parent"),
        pytest.param(52, id="the walk sits under a research issue it does not name"),
    ],
)
def test_a_finding_under_a_misplaced_walk_issue_is_reported(parent: int | None) -> None:
    finding = entry(593, parent=575, body=FINDING)
    found = [line for line in problems(walk(parent), finding, adrs=WALK_ADRS) if "#593" in line]
    assert len(found) == 1
    assert "#593 has parent #575" in found[0]
    assert "declares #476" in found[0]
