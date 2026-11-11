# 3. Rust on the device, Python on the host

- **Status:** Accepted
- **Date:** 2026-11-11
- **Recorded with:** the bootstrap, issue 1.

## Context

Two kinds of code live here. One program runs on the phone as PID 1, with nothing beneath
it but the kernel; if it panics, the phone shows a black screen. The rest runs on the
laptop: the repository's guards, and later the scripts that unpack and repack `boot.img`
and build the initramfs.

## Decisions

### D1. The on-device program is Rust, built static for `aarch64-unknown-linux-musl`

Memory safety matters most where there is no debugger and no console. A static musl binary
is one file in the initramfs with no loader and no libc beside it. `clippy` pedantic with
warnings denied, `unwrap`/`expect`/`panic!` warned on outside tests, and `unsafe` forbidden
outside adapter crates make the compiler hold most of `AGENTS.md`'s architecture rules.
The toolchain is pinned in `rust-toolchain.toml`.

**Rejected: C.** It needs no toolchain beyond the kernel's, and most kernel examples are in
it, but it gives up the safety and the lints that replace a debugger here. *Reopens if*
the binary must link a vendor C library that cannot be wrapped.

### D2. Host tooling is Python 3.12 with `uv`

The guards are proven Python, and boot-image tooling (`mkbootimg` and friends) is Python
already. `ruff`, `mypy` strict and `pytest` run on it.

**Rejected: shell scripts.** They cannot be typed or unit-tested the way the boot-image
round trip must be. A one-line wrapper is fine; logic is not.

### D3. Hardware sits behind traits, and the logic is tested on the host

`alarm-core` holds the scheduling logic and one trait per device. Adapters that open
`/dev`, `/sys` or `/proc` paths implement those traits in their own crates. The logic is
tested on the laptop and in CI with fakes; only the adapters need the phone.
