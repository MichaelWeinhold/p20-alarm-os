---
name: grooming-an-issue
description: How to groom a handed-over GitHub issue in this repository before any branch is cut — read what it points at, rewrite the title, description and labels in place with gh issue edit, and report the grooming back. Use whenever an issue number or a feature description is handed over.
---

# Grooming a handed-over issue

Before any branch is cut, read the issue, investigate what it points at, and rewrite it in
place:

```bash
gh issue view <n>
gh issue edit <n> --title "…" --body-file <path> --add-label "…" --parent <umbrella>
```

## What the rewritten issue contains

**Title** — the outcome, prefixed like a commit: `Feat:`, `Fix:`, `Chore:` or
`Research:`. An issue that holds sub-issues carries ☂️ **before** that prefix — `☂️
Research: which kernel the image boots` — from the moment its first child is attached,
and keeps it after the last one closes.

**Description** — in this order:

1. The original wording, **verbatim**, typos and all, under an `## Original request`
   heading.
2. The actual scope, as investigation found it.
3. A **Done when** line naming something that can be checked. When only the phone can
   check it, say what is read off the phone and where the reading is recorded.

Mark anything inferred rather than stated as an assumption.

**Labels** — from the existing set (`gh label list`). Create a new label only when
nothing fits. `hardware` goes on anything that needs the phone to do or verify; `device`
on anything that changes code under `crates/`.

**`bug` goes on a `Fix:` whose subject is code behaving wrongly.** A `Fix:` that corrects
a document, a comment or a line of prose takes `documentation` instead. The label is
load-bearing: `feature-lifecycle` step 3 reads it to decide whether a reproduction test
comes first.

**Parent** — `--parent <research issue>` when the issue implements a decision from an
accepted ADR, naming the issue whose work produced it. Leave an issue that descends from
no decision parentless; never mint a theme to hold one issue. What an issue *waits for* is
`--add-blocked-by`, which is many-to-many; an issue has at most one parent.

**After attaching a parent, run `uv run python scripts/check_issue_umbrellas.py`.**
→ `.claude/skills/feature-lifecycle/SKILL.md`, *A research issue is different*.

## Reading what it points at is part of grooming

If the issue points at something — a failing build, a kernel source tree, a postmarketOS
page, a linked PR, an ADR — read it, and put what it says in the description. That is
usually where the scope changes.

## Report the grooming back before starting

Say what the issue now claims, what changed from the original note, and which
assumptions were marked. Then follow the groomed issue.

## A research issue

Groom it the same way, but its **Done when** names an ADR: `docs/adr/NNNN-*.md` merged
with `Status: Accepted`, answering the questions the issue raises, with a
rejected-alternatives section and the condition that would reopen each one. Its header
carries `- **Issue:** #<n>`, which is how the scripts know it is a research issue. No
implementation starts before that ADR is accepted.
