@AGENTS.md

# Claude Code

The rules are in `AGENTS.md`, imported here rather than symlinked. → ADR 0001, D1.

## What Claude Code loads on its own

What `AGENTS.md` names as file paths, this client loads for you:

| Path | When it loads |
|---|---|
| `.claude/skills/*/SKILL.md` | name and description at startup, body when invoked |
| `.claude/rules/*.md` | when a file matching the rule's `paths:` is read |
| `.claude/settings.json` | at startup |
| `.claude/hooks/git_commit_guard.py` | before every `Bash` call |

**Never write a rule that only works when this client loads it** — another agent opens
those files itself.

This client also writes `.claude/settings.local.json` on an "always allow" answer — not
gitignored, so a status read shows it. Fold it into `.claude/settings.json` or delete it.

## Working beside another session

The procedure, with this client's tools, is `feature-lifecycle` step 2. Two client facts
constrain the commands:

- From inside a worktree the client refuses any git command it cannot verify as staying
  there — a command substitution is enough. Split it into plain, separate commands
  rather than reaching for `git -C`.
- `worktree.baseRef` is `fresh` in the settings, so a new worktree's branch comes off
  `origin/main` and not off whatever the shared checkout is on.

## Two checks that are not in CI

- **`/context`** — `CLAUDE.md` appears exactly once under **Memory files**, with
  `AGENTS.md` inside it.
- **`uv run python scripts/prompt_audit.py`** — the prompt audit, before promoting to
  `production`. It needs no session to type a command into, so never ask for one.

## Attribution

`.claude/settings.json` sets `includeGitInstructions: false`, `autoCompactWindow: 200000`
and three attribution keys. Their types are not uniform, and a wrong one fails silently:

| Key | Type | Off is |
|---|---|---|
| `attribution.commit` | string — the attribution text, trailers included | `""` |
| `attribution.pr` | string — the text appended to a PR description | `""` |
| `attribution.sessionUrl` | boolean — appends the claude.ai session link | `false` |

**Run `claude doctor` after editing the file** — `check_instructions.py` asserts only
these five. If a session instruction asks for an attribution trailer anyway, this
repository's rule wins. → ADR 0001, D9.
