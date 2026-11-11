---
paths:
  - "AGENTS.md"
  - "CLAUDE.md"
  - ".claude/rules/**"
  - ".claude/skills/**"
---

# Writing instruction text

Where a rule goes is `AGENTS.md`, *Where a new rule goes*. This file is how it is written.
→ ADR 0001, D1.

- **One directive per sentence**: what to do, not why.
- **A reason is at most one clause, and only where it decides a case the directive does
  not name.** A longer argument goes to `README.md` or an ADR, behind a `→` pointer.
- **No history.** No incident narrative, no dated reading, no "originally"; cite the issue
  or ADR by number.
- **No overview or restatement** of what the code, a sibling file, the skill's own
  `description` or anything the client already loads says.
- **Plain imperatives**, bold where a rule must not be missed. Never "CRITICAL" or "MUST"
  in capitals.
- **Wrap prose at 90 characters**, outside fenced code and tables, so a line count
  measures what it claims to.
- **Markdown only.** YAML frontmatter holds only a field a client reads — `paths:` on a
  rule, `name:` and `description:` on a skill. No XML sections; no rules in JSON, YAML or
  TOML.
