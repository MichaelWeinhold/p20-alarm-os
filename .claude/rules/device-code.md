---
paths:
  - "crates/**"
---

# Writing code that runs on the phone

- **A crate is either logic or an adapter, never both.** `alarm-core` names no device
  path; an adapter crate implements its traits and nothing else.
- **Allow `unsafe` per adapter crate only**, in its own `[lints]` table, and wrap each
  block in a function whose signature is safe.
- **Open a device once, at start-up**, and hand the adapter to the loop; a device that
  does not open is a `HardwareError::Absent` before anything is drawn. A device the kernel
  registers late, as touch after its firmware, is opened when it appears, and the adapter
  says which are still absent.
- **Keep the binary static.** No dependency that needs a C library at run time or a
  `build.rs` that links one; `/init` runs with nothing beside it.
- **Build `/init` `#![no_main]`, with an `extern "C" fn main`.** It starts with file
  descriptors 0 to 2 closed and no `/dev/null`, and Rust's start-up aborts on that before
  `main`. Never write to standard output or error in `/init`. → ADR 0006, D2.
- **Use `std` only until a crate earns its place** under ADR 0001, D7 — the binary is all
  of the image that is ours.
- **Log to the kernel log** (`/dev/kmsg`) once a logger exists; there is no console to
  read.
