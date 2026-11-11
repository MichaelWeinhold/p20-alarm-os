---
paths:
  - "tests/**"
  - "crates/**"
---

# Writing tests here

- **Fakes implement the same traits.** Drive `alarm-core` through a fake clock, RTC or
  vibrator, never through an adapter.
- **Each test gets fresh state** — `tmp_path` in Python, a value built in the test in
  Rust; never a file under the repository.
- **Never let a test touch the phone, `adb` or `fastboot`.** A test that needs device
  output replays a recorded fixture.
- **Return `Result` from a Rust test that uses `?`** rather than calling `unwrap`.

## Conventions this repository already follows

- `S101`, `S105` and `S106` are off under `tests/`. Give anything else a test needs
  suppressed its own `per-file-ignores` entry in `pyproject.toml`, never a blanket
  widening of `tests/*`.
- Annotate Python tests too: `ANN` is on for the whole tree.
- A test that shells out names its binary and gets `S603`/`S607` suppressed for that file
  alone.
- Rust unit tests live in a `#[cfg(test)] mod tests` beside the code they test.

## Running the suite

Replay a red with the `--randomly-seed=<n>` it printed, never a retry. Debug one test with
`-n 0 -p no:randomly`; never pin an order or raise `timeout`.
