from __future__ import annotations

from pathlib import Path

import pytest

from check_pr_closes import PullRequest, main, problems, pull_request_from

REPO_ROOT = Path(__file__).resolve().parents[1]
ADRS = {"0007": 77, "0009": 193}


def adr_tree(root: Path, adrs: dict[str, int] = ADRS) -> Path:
    directory = root / "docs" / "adr"
    directory.mkdir(parents=True, exist_ok=True)
    for number, issue in adrs.items():
        (directory / f"{number}-a-decision.md").write_text(
            f"# {int(number)}. A decision\n\n- **Status:** Accepted\n- **Issue:** #{issue}\n",
            encoding="utf-8",
        )
    return root


def payload(number: int, closes: list[int], headlines: list[str]) -> dict[str, object]:
    return {
        "number": number,
        "closingIssuesReferences": [
            {"number": issue, "url": f"https://example.invalid/{issue}"} for issue in closes
        ],
        "commits": [
            {"oid": f"{position:040d}", "messageHeadline": headline}
            for position, headline in enumerate(headlines)
        ],
    }


def run(root: Path, number: int, body: dict[str, object]) -> int:
    return main(["check_pr_closes.py", str(number), str(root)], lambda _repo, _number: body)


def test_a_pr_closing_only_its_own_issue_passes(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    assert run(root, 236, payload(236, [223], ["Chore(#223): refuse a stray branch"])) == 0


def test_the_run_reports_what_it_read(tmp_path: Path, capsys: pytest.CaptureFixture[str]) -> None:
    root = adr_tree(tmp_path)
    assert run(root, 236, payload(236, [223], ["Chore(#223): refuse a stray branch"])) == 0
    assert capsys.readouterr().out == "PR #236 closes #223; its commits claim #223\n"


def test_an_empty_reading_is_reported_rather_than_passing_in_silence(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    root = adr_tree(tmp_path)
    assert run(root, 236, payload(236, [], [])) == 0
    assert capsys.readouterr().out == "PR #236 closes nothing; its commits claim nothing\n"


def test_a_pr_closing_a_second_issue_fails(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    body = payload(81, [77, 62], ["Chore(#77): propose ADR 0006"])
    assert run(root, 81, body) == 1
    found = problems(pull_request_from(body), frozenset())
    assert len(found) == 1
    assert "#62" in found[0]
    assert "#77" in found[0]


def test_a_pr_closing_its_research_issue_fails(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    body = payload(218, [193], ["Chore(#193): propose ADR 0009"])
    assert run(root, 218, body) == 1
    found = problems(pull_request_from(body), frozenset(ADRS.values()))
    assert len(found) == 1
    assert "research issue" in found[0]
    assert "answers issue" in found[0]


@pytest.mark.parametrize(
    ("number", "headline"),
    [
        pytest.param(214, "Chore(#190): part of the broker work", id="part of an issue"),
        pytest.param(229, "Chore(#193): accept ADR 0009", id="an ADR pull request"),
    ],
)
def test_a_pr_that_closes_nothing_passes(tmp_path: Path, number: int, headline: str) -> None:
    root = adr_tree(tmp_path)
    assert run(root, number, payload(number, [], [headline])) == 0


def test_a_promotion_pull_request_passes(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    headlines = [
        "Chore(#223): refuse a stray branch",
        "Chore(#224): assert the settings values",
        "Feat(#208): the sleep schedule",
        "Bump actions/checkout from 6 to 7",
    ]
    assert run(root, 300, payload(300, [], headlines)) == 0


def test_a_close_with_no_prefixed_commit_fails(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    body = payload(301, [223], ["fix a typo in the readme"])
    assert run(root, 301, body) == 1
    found = problems(pull_request_from(body), frozenset())
    assert "no commit carries one" in found[0]


def test_an_unprefixed_commit_beside_a_prefixed_one_is_ignored(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    headlines = ["Chore(#223): refuse a stray branch", "fix a typo"]
    assert run(root, 302, payload(302, [223], headlines)) == 0


def test_a_malformed_prefix_claims_nothing(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    body = payload(303, [223], ["Chore: refuse a stray branch"])
    assert run(root, 303, body) == 1
    assert "no commit carries one" in problems(pull_request_from(body), frozenset())[0]


def test_both_problems_are_reported_together(tmp_path: Path) -> None:
    root = adr_tree(tmp_path)
    body = payload(304, [193, 62], ["Chore(#193): propose ADR 0009"])
    assert run(root, 304, body) == 1
    found = problems(pull_request_from(body), frozenset(ADRS.values()))
    assert len(found) == 2


def test_a_research_issue_that_is_also_unclaimed_is_named_twice() -> None:
    body = payload(305, [77], ["Chore(#223): refuse a stray branch"])
    found = problems(pull_request_from(body), frozenset(ADRS.values()))
    assert len(found) == 2


@pytest.mark.parametrize(
    "body",
    [
        [],
        {"number": "236", "closingIssuesReferences": [], "commits": []},
        {"number": 236, "closingIssuesReferences": {}, "commits": []},
        {"number": 236, "closingIssuesReferences": [], "commits": {}},
        {"number": 236, "closingIssuesReferences": [{"url": "x"}], "commits": []},
        {"number": 236, "closingIssuesReferences": [], "commits": [{"oid": "x"}]},
    ],
)
def test_a_malformed_payload_exits_two(tmp_path: Path, body: object) -> None:
    assert main(["check_pr_closes.py", "236", str(tmp_path)], lambda _r, _n: body) == 2


@pytest.mark.parametrize("argv", [["check_pr_closes.py"], ["check_pr_closes.py", "main"]])
def test_a_missing_or_unreadable_number_exits_two(argv: list[str]) -> None:
    assert main(argv, lambda _r, _n: {}) == 2


def test_the_bootstrap_issue_is_not_a_research_issue() -> None:
    pull = PullRequest(number=2, closes=frozenset({1}), claimed=frozenset({1}))
    from check_issue_umbrellas import research_issues

    assert problems(pull, frozenset(research_issues(REPO_ROOT).values())) == []
