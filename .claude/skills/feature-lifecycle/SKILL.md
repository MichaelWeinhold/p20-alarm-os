---
name: feature-lifecycle
description: The Definition of Done for this repository — issue, feature branch, the local checks, single-line prefixed commit, PR, green pipeline, squash merge. Use before starting any feature, fix or chore, before every commit, and before reporting any work finished.
---

# Working a change from issue to merge

A change is **done** only when every step below has happened, **in this order**. Until
then, never report it as finished or hand it over.

## 1. There is an issue, and it has been groomed

- An issue number was handed over → groom it with the `grooming-an-issue` skill, report
  the grooming back, then follow it.
- A description was handed over → open the issue first with `gh issue create`, in the
  shape `grooming-an-issue` gives, with the description verbatim as the original
  request; report it back, then follow it. A description of something behaving wrongly
  opens as `gh issue create --label bug`, titled `Fix:`, because step 3 reads that label.

**Claim the number before grooming it**, with the session name `ListAgents` prints:
`uv run python scripts/next_issue.py --claim <n> --by <name>`. Exit 1 names the holder:
stop and tell the operator. → ADR 0001, D5.

Cut no branch before the issue exists and reads correctly.

## 2. It is on its own feature branch, cut from `main`

**Find out who else is in this checkout, before the branch.** The first session in a
checkout keeps it; a session that finds another already there works in its own worktree.
Another agent uses `git worktree add` and `git worktree remove`; in Claude Code:

| Mechanism | What it does |
|---|---|
| `ListAgents` | lists the peer sessions; it reports a moment, so re-read it |
| `claude --worktree <name>` | launches into a worktree, branched from `origin/main` |
| `EnterWorktree` / `ExitWorktree` | the same mid-session, and the way back out |
| `.claude/worktrees/` | where both of them land, one directory per worktree |
| `SendMessage` | reaches a peer session, for the repair in step 4 |

In the shared checkout only — a worktree's branch already exists:

```bash
git checkout main && git pull
git checkout -b <type>/<shortname>
```

`feat/<shortname>` for a capability, `fix/<shortname>` for a bug, `chore/<shortname>` for
the rest. **Rename a worktree's branch before the first commit**: the client cuts
`worktree-<name>` where this requires `<type>/<shortname>`, so run
`git branch -m <type>/<shortname>` inside the worktree.

**Verify the branch before the first edit.** The commit guard refuses a wrong branch but
cannot move work already made there.

**In a shared checkout, name a revision in every content read** — `git show <ref>:<path>`,
`git diff <refA>..<refB>` — never `cat` or `grep -r`, which answer for whatever branch a
peer has just checked out. Re-read a live-state read — `git status`,
`git branch --show-current`, `ListAgents`, anything about the phone — where it is used,
never carried across a turn. → ADR 0001, D5.

## 3. All the checks pass locally, with no errors

Run the `## Tooling` block in `AGENTS.md`, not a copy. **Never let CI be the first place
the checks run.**

### On a `bug`, the tests run twice

A fix for an issue labelled `bug` starts with a test that fails **because of the bug**,
written against the unfixed code and watched failing before a line of the fix exists:

```bash
cargo test -p <crate> <name>           # or: uv run pytest tests/test_<thing>.py -k <name>
#   … red, and red for the bug's reason; then the fix …
cargo test --locked && uv run pytest   # green, and nothing else moved
```

**Read the red output before continuing** — a failure on a typo or a missing import
reproduces nothing. Commit the test and the fix together, in **one** commit.

**Say out loud when it does not apply**, and name which case in the PR body:

- The subject is a document, a comment or a message; such an issue carries
  `documentation`, not `bug`.
- It happens only on the phone. First look for the part testable on the host — the logic
  behind the adapter, or a recorded fixture of what the device returned. Only when nothing
  is left does the check go in `docs/manual-steps/` with the reading in
  `docs/device-history/`.

## 4. It is committed

Commit as soon as a **meaningful partial implementation** stands — not once at the end.

- **One line.** No description, no body, no second `-m`.
- **Prefixed with the issue number**: `Feat(#<n>): …` for a capability, `Fix(#<n>): …`
  for a bug, `Chore(#<n>): …` for the rest. Example:
  `Fix(#12): arm the RTC in UTC, not local time`.
- Only an incidental one-off — a typo, a formatting fix — may be a plain one-liner with
  no prefix.
- **The author is the human.** No `Co-Authored-By: Claude`, no "generated with" footer, no
  `Claude-Session:` line, no session link.

A refusal from the commit hook prints its reason: **fix the message, never work around
the hook.**

Push after each commit:

```bash
git push -u origin <branch>
```

### If the commit landed on a peer's branch

Repair it at the ref level, so the peer's `HEAD` and working tree never move:

```bash
git branch -f <your-branch> <sha>   # the commit, onto the branch it belongs on
git push -u origin <your-branch>
git reset --soft <peer-tip>         # the peer's branch, back where they left it
git restore --staged .
```

Then tell the peer — in Claude Code, `SendMessage`. **Never `git reset --hard`, check out
the peer's branch, or force-push anything but your own branch.**

## 5. A PR is open

```bash
gh pr create
```

**The only issue a pull request may close is its own** — the number in the branch's
commit prefix. Write `Part of #<n>` when the merge advances an issue without finishing
it. The PR description carries no attribution footer.

**Write every other number so GitHub cannot read it as a directive.** GitHub closes the
issue after `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves` or
`resolved`, with or without a colon, anywhere in the body and whatever the sentence
means. Write `issue 62`, or move the number away from the keyword.

Then read back what GitHub derived:

```bash
gh pr view <n> --json closingIssuesReferences
```

**An ADR's pull request must not close its research issue**, which closes when its last
follow-up does. Write `answers issue 7`, never `Closes #7`.

`pr-closes.yml` asserts both on every PR, and the ruleset requires its
`closing-references` check; still read the references back. → ADR 0001, D3.

## 6. The pipeline has finished and is green

```bash
gh pr checks <n> --watch
```

A pipeline still running is not done. On failure:

```bash
gh run view <id> --log
```

Fix, commit, push, and wait again until green. If the branch is behind, rebase onto
`origin/main` — never a worktree's local `main`, which nothing here updates:

```bash
git fetch origin && git rebase origin/main && git push --force-with-lease
```

## 7. It is merged

```bash
gh pr merge <n> --squash --delete-branch
```

Only once step 6 is green.

**If the issue has a parent, read the parent's count.** `gh issue view <parent>` at `n/n`
means this was the last open child: run `scripts/check_issue_umbrellas.py`, then close the
parent, since GitHub does not.

### If the work was done in a worktree

**Leave the worktree before merging, and merge from the main checkout:**

1. `ExitWorktree` with **`keep`**.
2. `gh pr merge <n> --squash --delete-branch`, **from the main checkout**.
3. `rmdir .claude/worktrees` when it is left empty.
4. `git pull --ff-only`.

Only now is the change done, and only now is it reported as finished.

**Once the merge is reported, ask the operator for `/clear` before starting another
issue.**

## A research issue is different

A research issue ends in an **accepted ADR** under `docs/adr/`, and no implementation
starts before that. **File each change the ADR recommends as its own follow-up issue
under the research issue**, before the ADR's pull request merges:

```bash
gh issue create --parent <the research issue>
```

File a follow-up found later the same way.

**Run the audit once the set is filed, and again before the umbrella closes:**

```bash
uv run python scripts/check_issue_umbrellas.py
```

| Exit | Meaning |
|---|---|
| `0` | clean |
| `1` | problems on stderr, each naming an issue number |
| `2` | the audit could not run — `gh` failed or its JSON was the wrong shape; fix the tool |

The parentless issues on **stdout** are information and do not fail the run. **Answer a
finding with `gh issue edit`, not a commit.** → ADR 0001, D4.

Attaching the first child puts ☂️ on the research issue's title, and it never comes off
— `grooming-an-issue` has the form.
