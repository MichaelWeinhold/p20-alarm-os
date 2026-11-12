# p20-alarm-os

[![Main](https://github.com/MichaelWeinhold/p20-alarm-os/actions/workflows/main.yml/badge.svg)](https://github.com/MichaelWeinhold/p20-alarm-os/actions/workflows/main.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A single-purpose alarm clock built from an old Huawei P20 Lite. The phone keeps the Linux
kernel it already boots; everything above it — Android, its services, its apps — is
replaced by one static Rust program that draws the time, rings the alarm, and otherwise
keeps the phone in deep sleep.

## Why

An old phone has a good screen, a battery, a real-time clock and a vibrator, which is
everything a bedside clock needs. Running Android to use them means hundreds of processes
and a device that is never really asleep. This project removes that layer instead of
configuring it away, and it is also an exercise in bringing up a device with no public
documentation, starting from the vendor kernel rather than from nothing.

## Status

| Area | State |
|---|---|
| Repository, workflow and CI | **Built.** Guards, a pipeline that matches the local checks, rulesets. |
| Alarm logic (`alarm-core`) | **Built, host-tested.** When each alarm next fires, in local time, and arming the RTC for the earliest one. |
| Which kernel to boot | **Decided.** The kernel already on the phone, `4.9.319`, unchanged — [ADR 0004](docs/adr/0004-boot-the-kernel-already-on-the-phone.md). |
| Flashing and rollback | **Decided.** No tethered boot; flash `kernel` from fastboot and write the stock image back — [ADR 0005](docs/adr/0005-flash-kernel-and-roll-back-from-fastboot.md). |
| Boot image round trip | **Built, host-tested.** `scripts/boot_image.py` repacks the stock image byte for byte. |
| Where `/init` lives | **Decided.** On `system`, since the bootloader loads no ramdisk from `kernel` — [ADR 0006](docs/adr/0006-put-our-init-on-system.md). |
| Debug shell over USB | Planned — P2. |
| Clock on screen, touch, RTC wake, vibration | Planned — P3 to P6. |
| Speaker | Parked. |

## How it works

```
kernel = vendor kernel (unchanged)  ──mounts──▶  system as /
                                                   └── /init = alarmd (static, aarch64-musl, Rust)
```

`alarmd` runs as PID 1. It mounts `/dev`, `/sys` and `/proc`, draws to the framebuffer or
DRM device, reads `/dev/input/event*`, arms `/dev/rtc0` for the next alarm, and suspends
with `echo mem > /sys/power/state`. Only `system` is ever written — `kernel` stays stock
— so recovery and fastboot stay intact and the `system` backup restores the phone.

The logic is kept apart from the hardware: `alarm-core` knows nothing about devices and
takes each one as a trait, so the scheduling rules are tested on a laptop and in CI, and
only the thin adapters need the phone. → [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Hardware

| | |
|---|---|
| Phone | Huawei P20 Lite, codename `anne` |
| SoC | HiSilicon Kirin 659, 8× Cortex-A53, `aarch64` |
| Bootloader | unlocked; a custom ROM is already installed |
| Connection | USB, `adb` and `fastboot` on the laptop |

## What is hard

- **Audio.** Kirin audio goes through a DSP that needs firmware and mixer routing that
  Android's audio HAL normally sets up. The vibrator comes first, and the speaker is parked.
- **Debugging blind.** There is no serial console; a boot that fails is a black screen. A
  debug image with SSH over USB, or `ramoops`, is built before anything else.
- **Deep sleep and RTC wake.** The vendor kernel may expect Android userspace pieces before
  it suspends properly, and the alarm depends on the RTC waking it. That is proven early.

The decisions behind the approach, and what was rejected, are in
[ADR 0002](docs/adr/0002-vendor-kernel-and-own-userspace.md).

## How this repository is worked

Every change is an issue, a branch, a pull request and a green pipeline, merged by squash.
Most of the code is written by coding agents, so the process is written down as rules
and procedures they read, and enforced by mechanisms that hold whether or not they read
them:

- **[AGENTS.md](AGENTS.md)** holds the invariants and commands; this README holds the
  reasons. Nothing is written out in full in both.
- **Skills** under `.claude/skills/` hold the procedures: the Definition of Done, grooming
  an issue, and claiming the next free issue so parallel sessions never collide.
- **A commit hook** refuses commits on protected branches, malformed messages, and any AI
  co-author trailer; the human is the author.
- **The pipeline runs exactly the local checks**, and a script asserts that the two lists
  are equal. Two more gates read the pull request: it may close only its own issue, and a
  new dependency must say why it is worth it.
- **Research comes before code.** An open question becomes a research issue, which ends in
  an ADR with the rejected alternatives and the condition that would reopen each one.
- **`main` is integration; `production` is what is on the phone**, and only a person at the
  USB cable writes to it.

→ [ADR 0001](docs/adr/0001-how-this-repository-is-worked.md) for each of these decisions.

## Documentation

| Document | What it covers |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | The image, the program's crates, the loop, host tooling |
| [docs/ROADMAP.md](docs/ROADMAP.md) | The phases, P0 to P6, and what is parked |
| [docs/SECURITY.md](docs/SECURITY.md) | Bricking risks, what must never be published, licensing, supply chain |
| [docs/MANUAL_STEPS.md](docs/MANUAL_STEPS.md) | The hand-made state, one file per subsystem under `docs/manual-steps/` |
| [docs/device-history/](docs/device-history/) | What each hand-made run printed, and when |
| [docs/adr/](docs/adr/) | Accepted decisions, what was rejected, and what would reopen it |

## Development

Requirements: [uv](https://docs.astral.sh/uv/) and [rustup](https://rustup.rs/); the Rust
toolchain and the `aarch64-unknown-linux-musl` target install themselves from
`rust-toolchain.toml`.

```bash
uv sync
uv run ruff check . && uv run ruff format --check . && uv run mypy && uv run pytest
uv run python scripts/check_instructions.py
cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked
cargo build --release --locked --target aarch64-unknown-linux-musl
```

Linking a binary for the phone needs a Linux host or a cross linker; CI's `image` job
builds the release natively on an `aarch64` runner.

## License

MIT — see [LICENSE](LICENSE). The vendor kernel that the image boots is GPL-2.0 and is not
part of this repository.
