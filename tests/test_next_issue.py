from __future__ import annotations

import json
import os
import subprocess
from datetime import UTC, datetime
from pathlib import Path

import pytest

from check_issue_umbrellas import AuditError
from next_issue import (
    CLAIMS,
    Claims,
    Listing,
    claim_next,
    issues_from,
    judge,
    main,
    pulls_from,
)

AT = datetime(2026, 10, 26, 12, 0, tzinfo=UTC)


def issue(
    number: int,
    *,
    labels: tuple[str, ...] = ("chore",),
    blocked_by: tuple[tuple[int, str], ...] = (),
    children: int = 0,
) -> dict[str, object]:
    return {
        "number": number,
        "title": f"Chore: issue {number}",
        "labels": [{"id": f"LA_{name}", "name": name, "color": "c5def5"} for name in labels],
        "blockedBy": {
            "nodes": [{"number": blocker, "state": state} for blocker, state in blocked_by],
            "totalCount": len(blocked_by),
        },
        "subIssuesSummary": {"completed": 0, "percentCompleted": 0, "total": children},
    }


def pull(number: int, title: str) -> dict[str, object]:
    return {"number": number, "title": title}


def round_trip(entries: list[dict[str, object]]) -> object:
    parsed: object = json.loads(json.dumps(entries))
    return parsed


def listing(
    issues: list[dict[str, object]], pulls: list[dict[str, object]] | None = None
) -> Listing:
    def load(_: Path, kind: str) -> object:
        return round_trip(issues if kind == "issue" else pulls or [])

    return load


def reasons_for(
    issues: list[dict[str, object]], pulls: list[dict[str, object]] | None = None
) -> dict[int, tuple[str, ...]]:
    verdicts = judge(issues_from(round_trip(issues)), pulls_from(round_trip(pulls or [])), {})
    return {verdict.issue.number: verdict.reasons for verdict in verdicts}


def git(cwd: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True, check=True
    ).stdout.strip()


@pytest.fixture
def worktrees(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> tuple[Path, Path]:
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", os.devnull)
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    for role in ("AUTHOR", "COMMITTER"):
        monkeypatch.setenv(f"GIT_{role}_NAME", "Test")
        monkeypatch.setenv(f"GIT_{role}_EMAIL", "test@example.invalid")
    first = tmp_path / "first"
    second = tmp_path / "second"
    first.mkdir()
    git(first, "init", "--quiet", "--initial-branch=main")
    git(first, "commit", "--quiet", "--allow-empty", "-m", "initial")
    git(first, "worktree", "add", "--quiet", "-b", "other", str(second))
    return first, second


def test_an_open_unblocked_leaf_issue_is_free() -> None:
    assert reasons_for([issue(5)]) == {5: ()}


def test_a_parked_issue_is_not_free() -> None:
    assert reasons_for([issue(5, labels=("future",))])[5] == ("parked — labelled future",)


def test_an_open_blocker_holds_an_issue_and_a_closed_one_does_not() -> None:
    found = reasons_for([issue(5, blocked_by=((3, "OPEN"), (2, "CLOSED")))])
    assert found[5] == ("waits for #3",)
    assert reasons_for([issue(5, blocked_by=((2, "CLOSED"),))])[5] == ()


def test_an_umbrella_is_not_free() -> None:
    assert reasons_for([issue(5, children=2)])[5] == (
        "an umbrella of 2 sub-issues — work its children",
    )


def test_an_open_pull_request_naming_the_issue_holds_it() -> None:
    found = reasons_for(
        [issue(5), issue(55)],
        [pull(9, "Chore(#5): the work"), pull(10, "chore: bump a dependency")],
    )
    assert found == {5: ("pull request #9 is open",), 55: ()}


def test_every_reason_is_listed() -> None:
    found = reasons_for([issue(5, labels=("future",), children=1)], [pull(9, "Feat(#5): x")])
    assert len(found[5]) == 3


def test_candidates_come_in_ascending_number() -> None:
    issues = issues_from(round_trip([issue(9), issue(3), issue(7)]))
    assert [found.number for found in issues] == [3, 7, 9]


def test_a_listing_of_the_wrong_shape_exits_2(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    monkeypatch.chdir(worktrees[0])

    def wrong(_: Path, kind: str) -> object:
        return {"issues": []} if kind == "issue" else []

    assert main(["next_issue.py"], wrong) == 2
    assert "expected a list" in capsys.readouterr().err


def test_a_label_with_no_name_exits_2(
    worktrees: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(worktrees[0])
    broken = issue(5)
    broken["labels"] = [{"id": "LA_x"}]
    assert main(["next_issue.py"], listing([broken])) == 2


def test_listing_prints_free_issues_and_why_the_rest_are_not(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    monkeypatch.chdir(worktrees[0])
    code = main(["next_issue.py"], listing([issue(4), issue(5, labels=("future",))]))
    out, err = capsys.readouterr()
    assert code == 0
    assert out == "#4 Chore: issue 4\n"
    assert "#5 is not free: parked — labelled future" in err


def test_listing_with_nothing_free_exits_1(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    monkeypatch.chdir(worktrees[0])
    assert main(["next_issue.py"], listing([issue(5, children=1)])) == 1
    assert "nothing is free" in capsys.readouterr().err


def test_a_claim_is_visible_from_the_other_worktree(worktrees: tuple[Path, Path]) -> None:
    first, second = worktrees
    claim = Claims(first).take(5, "session-a", AT)
    assert claim is not None
    seen = Claims(second).held()[5]
    assert (seen.holder, seen.since) == ("session-a", "2026-10-26T12:00:00+00:00")


def test_a_second_create_fails_from_either_worktree(worktrees: tuple[Path, Path]) -> None:
    first, second = worktrees
    assert Claims(first).take(5, "session-a", AT) is not None
    assert Claims(second).take(5, "session-b", AT) is None
    assert Claims(first).take(5, "session-b", AT) is None
    assert Claims(first).held()[5].holder == "session-a"


def test_a_claim_lost_in_a_race_moves_on_to_the_next_candidate(
    worktrees: tuple[Path, Path],
) -> None:
    first, second = worktrees
    verdicts = judge(issues_from(round_trip([issue(4), issue(5), issue(6)])), {}, {})
    assert Claims(second).take(4, "session-b", AT) is not None
    claim, lost = claim_next(verdicts, Claims(first), "session-a", AT)
    assert claim is not None
    assert claim.number == 5
    assert lost == ["#4 was claimed first by session-b since 2026-10-26T12:00:00+00:00"]


def test_claim_skips_a_claimed_issue_and_prints_the_one_it_took(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    first, second = worktrees
    monkeypatch.chdir(first)
    Claims(second).take(4, "session-b", AT)
    found = listing([issue(4), issue(5)])
    assert main(["next_issue.py", "--claim", "--by", "session-a"], found, lambda: AT) == 0
    assert capsys.readouterr().out == "5\n"
    assert Claims(second).held()[5].holder == "session-a"


def test_claiming_a_named_issue_held_by_another_session_exits_1(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    first, second = worktrees
    monkeypatch.chdir(first)
    Claims(second).take(5, "session-b", AT)
    code = main(["next_issue.py", "--claim", "5", "--by", "session-a"], listing([issue(5)]))
    assert code == 1
    assert "#5 is held by session-b" in capsys.readouterr().err


def test_claiming_a_named_issue_again_by_its_holder_succeeds(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    monkeypatch.chdir(worktrees[0])
    found = listing([issue(5)])
    assert main(["next_issue.py", "--claim", "5", "--by", "session-a"], found, lambda: AT) == 0
    assert main(["next_issue.py", "--claim", "5", "--by", "session-a"], found, lambda: AT) == 0
    assert capsys.readouterr().out == "5\n5\n"


def test_a_named_issue_that_is_not_free_is_claimed_with_a_note(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    monkeypatch.chdir(worktrees[0])
    found = listing([issue(5, blocked_by=((3, "OPEN"),))])
    assert main(["next_issue.py", "--claim", "5", "--by", "session-a"], found, lambda: AT) == 0
    assert "which is not free: waits for #3" in capsys.readouterr().err


def test_claiming_an_issue_that_is_not_open_exits_1(
    worktrees: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.chdir(worktrees[0])
    assert main(["next_issue.py", "--claim", "7", "--by", "session-a"], listing([issue(5)])) == 1
    assert 7 not in Claims(worktrees[0]).held()


def test_claim_without_a_session_is_refused(worktrees: tuple[Path, Path]) -> None:
    with pytest.raises(SystemExit) as raised:
        main(["next_issue.py", "--claim"], listing([issue(5)]))
    assert raised.value.code == 2


def test_a_session_name_with_a_space_is_refused() -> None:
    with pytest.raises(SystemExit) as raised:
        main(["next_issue.py", "--claim", "--by", "two words"], listing([issue(5)]))
    assert raised.value.code == 2


def test_release_drops_a_claim(
    worktrees: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    first, second = worktrees
    monkeypatch.chdir(first)
    Claims(second).take(5, "session-b", AT)
    assert main(["next_issue.py", "--release", "5"], listing([issue(5)])) == 0
    assert 5 not in Claims(second).held()


def test_release_drops_a_claim_without_calling_github(
    worktrees: tuple[Path, Path], monkeypatch: pytest.MonkeyPatch
) -> None:
    first, second = worktrees
    monkeypatch.chdir(first)
    Claims(second).take(5, "session-b", AT)

    def offline(_: Path, kind: str) -> object:
        raise AuditError(f"gh {kind} list: could not connect")

    assert main(["next_issue.py", "--release", "5"], offline) == 0
    assert 5 not in Claims(second).held()


def test_every_run_releases_the_claims_of_issues_no_longer_open(
    worktrees: tuple[Path, Path],
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    first, second = worktrees
    monkeypatch.chdir(first)
    Claims(second).take(3, "session-b", AT)
    Claims(second).take(5, "session-b", AT)
    assert main(["next_issue.py"], listing([issue(5)])) == 1
    assert set(Claims(second).held()) == {5}
    assert "released #3, no longer open" in capsys.readouterr().err


def test_a_ref_under_claims_that_is_not_a_number_is_ignored(worktrees: tuple[Path, Path]) -> None:
    first, _ = worktrees
    blob = subprocess.run(
        ["git", "hash-object", "-w", "--stdin"],
        cwd=first,
        input="someone\n",
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    git(first, "update-ref", f"{CLAIMS}notes", blob)
    assert Claims(first).held() == {}
