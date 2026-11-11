# 1. How this repository is worked

- **Status:** Accepted
- **Date:** 2026-11-11
- **Recorded with:** the bootstrap, issue 1. This ADR answers no research issue; it writes
  down the process the repository starts with, so that changing it is a decision too.

## Context

Most of the code here will be written by coding agents, with one person as the feedback
loop: they flash the phone, read what it shows, and paste the logs back. Two things follow.

- **An agent reconstructs a procedure from memory unless it is told where the procedure
  is**, and a rule that matters has to hold whether or not it was read.
- **The phone cannot be tested in CI.** Everything that can be checked without it must be,
  so that a hardware session is spent on hardware questions.

The process below is one that has already run for several hundred issues on an earlier
single-maintainer project. What it learned is kept here; why each piece exists is in the
decision itself.

## Decisions

### D1. Instruction files are markdown, short, and in one place each

`AGENTS.md` holds invariants and commands and is read by any agent; `CLAUDE.md` imports it
with `@AGENTS.md` and adds only what is specific to Claude Code. Rationale goes in
`README.md`, decisions in `docs/adr/`, procedures in skills, rules for one directory in a
path-scoped rule. Nothing is written in full twice.

`scripts/check_instructions.py` holds the sizes: 200 lines per always-on file and 270 for
the pair, 500 per skill and 200 per rule, 1000 for everything loaded on demand, and prose
wrapped at 90 characters. A line ceiling alone is met by making lines longer; the width
limit closes that.

**Rejected:** XML-structured or YAML rule files — no client reads them as anything but
text, and markdown with frontmatter is what every agent client ships. *Reopens if* a
client defines a structured format it enforces.

### D2. Every change is issue → branch → PR → green → squash merge

One issue per change, groomed before a branch is cut: the original request verbatim, the
scope as found, and a *Done when* that can be checked. Branches are `feat/`, `fix/` or
`chore/`. Commits are one line, `Feat(#n): …`, with the human as the author and no AI
trailer. `.claude/hooks/git_commit_guard.py` refuses anything else, so the rule holds
without being read.

A `bug` starts with a test that fails for the bug's reason, committed with the fix.

**Rejected:** a human review gate. There is one maintainer; a required approval would be
self-approval. *Reopens when* a second person has push access.

### D3. The local checks and the CI job are one list

The `## Tooling` block in `AGENTS.md` and the `check` job in `checks.yml` are compared for
equality, so the checks a session runs before a push are the checks the pipeline confirms.
Gates that read the pull request itself — its closing references and its new packages —
live in their own workflows, because they must re-run when the description is edited.

`pr-closes.yml` grants `issues: read`: without it, GitHub's GraphQL API returns an empty
list of closing references instead of an error, and the gate passes everything.

### D4. A research issue ends in an ADR, and its follow-ups are its children

No implementation starts before its ADR is accepted. Each change the ADR recommends is
filed as a sub-issue of the research issue, which carries ☂️ and closes with its last
child. `scripts/check_issue_umbrellas.py` reports orphans and parents owed a close.

### D5. Parallel sessions do not share a branch

A session that finds another in the checkout works in its own worktree. Issues are claimed
with `scripts/next_issue.py`, which writes the claim as a ref under `refs/claims/` on the
remote, so two sessions cannot take the same issue.

### D6. `main` is integration; `production` is what is on the phone

`main` takes squash or rebase merges. `production` moves forward from `main` by merge
commit only, and is what gets flashed. Nothing in the pipeline reaches the phone: it is
flashed by hand, over USB, from a checkout of `production`. Rulesets protect both, with no
bypass for anyone.

**Rejected:** a flash job on a self-hosted runner with the phone attached — it would put
a write to the device behind a push, and the rule that only `boot` is written would depend
on the job rather than on a person at the cable. *Reopens* never for writes; a read-only
job that boots an image tethered and reads its log could be proposed on its own.

### D7. A package comes in only when it is worth it

The pull request that adds one — to `pyproject.toml` or any `Cargo.toml` — carries a
`## Dependency: <name>` section answering six questions: what it replaces, who publishes
it, whether it is maintained, its licence, what it pulls in, and where it runs. On the
phone every crate is binary size and attack surface; `std` comes first. A version bump
needs no section. `pr-dependencies.yml` enforces the section.

### D8. Hand-made state is written down as state, and its history apart

`docs/manual-steps/<subsystem>.md` holds what was done by hand and how to reproduce it —
the commands, one check each, the undo. What a run printed, and when, goes to
`docs/device-history/<subsystem>.md`. Nothing needed to rebuild the setup may exist only in
the history. The index is `docs/MANUAL_STEPS.md`, with a ceiling of 100 lines and 250 per
file.

### D9. The client's settings are checked, not trusted

`.claude/settings.json` turns off the client's own git instructions and every attribution
key, and sets `autoCompactWindow` to 200000. The attribution keys differ in type — two are
strings, one is a boolean — and a value of the wrong type is discarded without a warning,
so `check_instructions.py` asserts each value and its type.
