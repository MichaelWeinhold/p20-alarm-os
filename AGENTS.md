# AGENTS.md

Rules for any coding agent working in this repository. Every line is an
instruction; the reasoning is in `README.md` and `docs/adr/`.

## Where a new rule goes

Classify it before writing it. Nothing is stated in full in two places; the
second place gets a pointer. How it is worded → `.claude/rules/instruction-files.md`.

| It is… | It goes in |
|---|---|
| an invariant, a command, or anything that constrains a change | this file |
| rationale, narrative, or anything explaining *why* | `README.md` |
| a decision, what was rejected, and what would reopen it | `docs/adr/` |
| a multi-step procedure | `.claude/skills/<name>/SKILL.md` |
| true only inside one directory | `.claude/rules/<name>.md`, with `paths:` |
| something that must hold whatever an agent decides | `.claude/settings.json` or `.claude/hooks/` |

A device-safety invariant stays in this file **unscoped**, even when a directory would
fit — it must not depend on a glob matching.

`.claude/settings.json` is the **only** settings file. A second one a client writes for
itself is folded in or deleted, never left as a truth nobody sees.

## What this is

A single-purpose alarm-clock image for the Huawei P20 Lite (`anne`, Kirin 659): the
vendor Linux kernel the phone already boots, running one program of ours as `/init` on
`system`. → README, the opening section; ADR 0002 and 0006.

- **Write on-device code in Rust**, under `crates/`, built as one static
  `aarch64-unknown-linux-musl` binary.
- **Write host tooling in Python 3.12 with `uv`**, under `scripts/`. → ADR 0003.
- **Add a package only when it is worth it.** The PR that adds one answers ADR 0001, D7's
  six questions in a `## Dependency: <name>` section per package; a version bump needs
  none. This holds for `pyproject.toml` and every `Cargo.toml` alike.
- **Reuse the vendor kernel; never write a driver from scratch** while the kernel has one.
  → ADR 0002.

## Read before changing anything

**Read `docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/ROADMAP.md`, and
`docs/MANUAL_STEPS.md` and the files it indexes, before changing anything they
describe.** `README.md` says what each one covers.

**Read `docs/adr/` for any decision you are about to revisit.** It records what was
rejected and the condition that would reopen it.

**Start a research issue's implementation only after its ADR is accepted.**

## Architecture rules

The numbers are stable: cite them, and keep them consecutive when one is added.

1. **Keep the logic in `alarm-core` free of hardware.** It takes the clock, the RTC and
   every other device as a trait from `alarm_core::hardware`, and its tests drive fakes.
2. **Put every device behind one trait and one adapter.** An adapter is the only code
   that opens a `/dev`, `/sys` or `/proc` path, and the only place `unsafe` may appear.
3. **Never return a plausible default when a device is absent or fails** — return
   `HardwareError`. A clock that cannot be read arms no alarm.
4. **Never panic on the device.** No `unwrap`, `expect` or `panic!` outside tests; clippy
   warns on each, and CI denies warnings.
5. **Compute time in UTC seconds and convert to local time once**, through `UtcOffset`.
   The RTC holds UTC.
6. **Make a new device additive.** Adding the vibrator must not change the display's
   adapter; anything two adapters share moves into a common module.

## Device safety

**Run any command on the phone without asking** — `adb`, `fastboot flash`, `erase` and
`format`, `dd` from the ROM or TWRP. A soft brick is acceptable; a hard brick never is.
→ ADR 0005, D4.

**Never write, erase or format a partition that can hard-brick the phone**, by `fastboot`
or by `dd`: the eMMC boot areas (`mmcblk0boot0`, `mmcblk0boot1`), `xloader`, the
partition table (`ptable`), `vrl`, `vrl_backup`, `fastboot`, `nvme` or `oeminfo`.

**Never relock the bootloader** — no `flashing lock`, `oem lock` or anything that relocks.

**Read a partition whose role is not recorded in `docs/device-history/phone.md` before
writing it**, and record the role there.

**Back up a partition off the repository before its first write**, under
`~/p20-backups/`, and never delete a backup there. → `docs/SECURITY.md`, *Bricking*.

**Leave `kernel` stock and put the image's `/init` on `system`**; the bootloader loads no
ramdisk from `kernel`, and there is no `boot`. → ADR 0006.

**Write an image only after its sha256 and size are checked.** → ADR 0005, D1.

**Never commit an image, kernel, firmware, device tree or anything read off the phone.**
`.gitignore` lists them, and `checks / secrets` fails the build if one is tracked.

## Tooling

Python 3.12 and `uv` — never pip or poetry. `ruff` for lint and format, `mypy` strict,
`pytest`. Rust stable pinned in `rust-toolchain.toml`, `rustfmt`, `clippy` pedantic.

```bash
uv sync
uv run ruff check .
uv run ruff format --check .
uv run mypy
uv run pytest
uv run python scripts/check_instructions.py
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## How work happens here

**Read the procedure before doing the thing — never reconstruct it from memory.**

| Procedure | Read it |
|---|---|
| `.claude/skills/feature-lifecycle/SKILL.md` | before starting any feature, fix or chore; before every commit; before reporting anything finished |
| `.claude/skills/picking-the-next-issue/SKILL.md` | when told to pick the next issue, before choosing one |
| `.claude/skills/grooming-an-issue/SKILL.md` | when an issue number or a description is handed over, before a branch is cut |
| `.claude/skills/flashing-the-phone/SKILL.md` | before every write to `kernel`, a rollback included |

These are mechanisms, and hold whether or not a procedure is read:

- The branch ruleset blocks a merge until `checks / check`, `checks / secrets`,
  `checks / image`, `closing-references` and `dependencies` are green on an up-to-date
  branch, and permits no direct push, force push or deletion on `main` or `production`.
- `main` takes **squash or rebase** merges only; `production` takes a **merge commit**
  only. `production` is the image that is on the phone.
- `.claude/hooks/git_commit_guard.py` refuses a commit on `main`, `production`, a branch
  named none of `feat/`, `fix/` and `chore/`, or a branch this session did not switch to;
  a message with a body; a malformed prefix; and any AI author, co-author or session
  trailer.

**Work in a tree no other session is holding.** Another session already in this checkout
means a `git worktree` instead. → ADR 0001, D5.

**Run the checks above locally before every push**; CI confirms a green result, never
discovers it. **Never merge a PR whose pipeline has not finished, and never bypass the
ruleset.** There is no human review gate here: green means merge, not green means fix it.

## Code style

- **Write raw code.** No inline comments, no docstrings, no block comments, no `///`.
- **Type strictly, with no half measures.** `mypy` runs `strict` and `warn_unreachable`;
  clippy runs `pedantic` with warnings denied.
- **Never write `Any` in Python** — ruff's `ANN401` makes it an error.
- **Take external input in as `object`** in Python and narrow with `isinstance`; in Rust,
  parse device bytes into a typed value at the adapter and return `HardwareError` on
  anything unexpected.

## Testing

**Test every rule in `alarm-core` on the host**, with fakes for the hardware traits.
**Never let a test touch the phone.** A test that needs device output uses a recorded
fixture under `tests/fixtures/` or the crate's own `testdata/`. →
`.claude/rules/testing.md`.
