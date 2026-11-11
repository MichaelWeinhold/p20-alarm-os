---
paths:
  - ".github/workflows/**"
---

# The pipeline

The `check` job in `checks.yml` runs exactly the `## Tooling` block of `AGENTS.md`, in
that order, after its `uv sync --locked`. `scripts/check_instructions.py` compares the two
lists for **equality**, so add a flag on both sides or neither. → ADR 0001, D3.

**Never skip a check, mark it `continue-on-error`, or make it conditional on the changed
paths.**

## What the job stands on is pinned

- **`runs-on: ubuntu-24.04` or `ubuntu-24.04-arm`, never a `-latest` label.** Take a new
  image in a pull request.
- **Pin every action to a commit SHA with the version in a trailing comment.**
  Dependabot's `github-actions` ecosystem bumps both.
- **`uv sync --locked` and `cargo … --locked`, never unlocked**, so a missing relock
  reddens a pull request.
- **The Rust toolchain comes from `rust-toolchain.toml`**, never from an action input.

The `secrets` job pipes `git ls-files` through `git check-ignore`. **Never give it a list
of its own**; `.gitignore` is the single statement of what must never be tracked.

The `image` job cross-builds the release for `aarch64-unknown-linux-musl`. **Keep it out
of the `check` job**: that job's steps are compared to `AGENTS.md`, and a laptop need not
cross-build.

## One file, three entry points

The checks live in **one** `workflow_call` workflow, `checks.yml`, which the three entry
points call rather than copy:

| Workflow | Trigger | What it does |
|---|---|---|
| `pr.yml` | PR into `main` or `production` | The checks. This is the green light on a PR. |
| `main.yml` | push to `main` | The checks. **Never flashes anything.** |
| `production.yml` | push to `production`, or manual dispatch | The checks. **No flash job.** |

Two more workflow files are **not** entry points:

| Workflow | Trigger | What it does |
|---|---|---|
| `pr-closes.yml` | PR, including on `edited` | One gate on the PR's closing references. |
| `pr-dependencies.yml` | PR, including on `edited` | One gate: each package new in `pyproject.toml` or a `Cargo.toml` has its `## Dependency:` section. → ADR 0001, D7. |

**Adding a check means editing `checks.yml` and the `## Tooling` block in `AGENTS.md`, in
the same commit.**

Two kinds of check stay out of the `check` job:

- **A check that needs the network or a credential.** `scripts/check_issue_umbrellas.py`
  and `scripts/prompt_audit.py` run on demand.
- **A gate that reads the pull request itself.** `pr-closes.yml` and `pr-dependencies.yml`
  keep their own file, trigger and `concurrency` group.

## Deployment

Nothing in the pipeline reaches the phone. **Add no flash job**: a phone is flashed by
hand over USB, from `production`. → ADR 0001, D6.

## Adding a check that touches history

**Never add a check that grades commits already on `main`.** A check that grades files as
they are now is fine **if the change that adds it also brings those files into shape**.

## What is deliberately not checked

Adherence: nothing here claims to measure it. A rule that is violated and matters becomes
a hook or a setting, never a better sentence.
