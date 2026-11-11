# Manual steps

Work that cannot be scripted. Keep it current: when a PR needs a hand-made action to go
with it, the step is added in the same PR, to the file below for its subsystem.

Status legend: `[ ]` open, `[x]` done.

Each file under `docs/manual-steps/` holds one subsystem's hand-made **state** and how to
**reproduce** it: the commands, one check each, the undo, each lesson as one ⚠️ line beside
the command it constrains, and the open work. A run's output, timestamps and first attempts
go to `docs/device-history/<slug>.md`, under the same slug, in the same change, and the
file ends with a pointer there. When a step is done, delete its box and say in the file
what is now true; `[x]` stays only on checklists that repeat. A procedure meant to be
repeated goes in a skill. Nothing needed to rebuild the setup may exist only in
`docs/device-history/`. → ADR 0001, D8.

Read this index and the file for the subsystem being changed. A new subsystem is a new file
and a new row. A reference to another file's heading is a relative link.

| # | File | What it covers |
|---|---|---|
| 1 | [github.md](manual-steps/github.md) | The repository's settings, security features, labels and rulesets |
| 2 | [before-every-promotion-to-production.md](manual-steps/before-every-promotion-to-production.md) | The checklist that repeats on every promotion |
| 3 | [phone.md](manual-steps/phone.md) | The ROM and kernel on the phone, its partitions, and the stock image's backup |

How an image is tried and flashed, and the rollback, joins `phone.md` once the flashing
research issue has decided them.
