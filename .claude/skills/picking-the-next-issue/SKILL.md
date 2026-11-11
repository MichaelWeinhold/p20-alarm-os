---
name: picking-the-next-issue
description: How a session picks the next free issue in this repository and claims it so no peer takes it too — release the claims of absent sessions, claim with scripts/next_issue.py, then groom. Use when told to pick, take or work on the next issue.
---

# Picking the next free issue

**`scripts/next_issue.py` decides what is free; never filter the issue list by hand.**
→ ADR 0001, D5.

1. **Read `ListAgents`.** This session's name follows "This session is".
2. **Release the claims of absent holders.** Run `uv run python scripts/next_issue.py`;
   for each `claimed by <holder>` that `ListAgents` does not list, run
   `uv run python scripts/next_issue.py --release <n>` and say so.
3. **Claim one** with `uv run python scripts/next_issue.py --claim --by <name>`. Exit 1
   means nothing is free: report the reasons and stop.
4. **Hand the printed number to `grooming-an-issue`**, then follow `feature-lifecycle`.

**Run `--release <n>` when stopping without a merge**; a merge releases on its own.
