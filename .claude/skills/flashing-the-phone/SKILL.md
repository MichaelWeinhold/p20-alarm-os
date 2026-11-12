---
name: flashing-the-phone
description: How an image is written to the phone's kernel partition and taken back off it — reach fastboot, run the device preflight, flash, read what came up, roll back to the stock image. Use before every write to kernel, a rollback included.
---

# Flashing the phone

**Read `AGENTS.md`, *Device safety*, before the first command**; it says what runs
without asking and what never runs. The commands are in `docs/manual-steps/phone.md`; run
them from there, by the paragraph each step names. → ADR 0005.

1. **Know the sha256 the image must have** — the one its build printed, or the backup's
   in *Verify*.
2. **Reach *Fastboot & Rescue mode*** — from the ROM by *Fastboot*, from an image that
   runs no `adbd` by *Fastboot without adb*.
3. **Run the preflight in *Flash*.** Exit 1 names the reason: remove it and run the
   preflight again, never flash past it. **Stop when the sha256 it prints is not step
   1's.**
4. **Flash with *Flash*'s command, as the very next `fastboot` invocation.** Anything sent
   in between means step 3 again.
5. **Read what came up.** The ROM: *Roll back*'s last three lines. An image of ours: the
   screen, which the operator reports, and issue 15's debug lifeline once it exists.
6. **Leave an image of ours up ten minutes** unless it has visibly failed, so the
   boot-fail monitor counts it as booted. → ADR 0005, D5.
7. **Roll back with *Roll back*** when the try is over or the image never came up: steps 2
   to 5 with the backup, ending on `mmcblk0p31` hashing to it.
8. **Record the run in `docs/device-history/phone.md`**, under a dated heading naming the
   issue: the image's sha256, what the phone showed, and the hash after the rollback.

## When it goes wrong

- **`no 18d1:d00d on USB`** — the phone is not in fastboot: step 2.
- **A `fastboot` invocation answers nothing** — it had no USB reset: let its `timeout`
  end it and run step 3 again.
- **eRecovery comes up** — accept nothing on its screens. Before rolling back, start TWRP
  by *TWRP* and hash `/dev/block/by-name/kernel`, to read whether eRecovery left `kernel`
  alone; record that and how many failed tries came before it. → ADR 0005, D5.
- **Fastboot cannot be reached** — TWRP by *TWRP*. Its write of the backup is untried:
  `dd` onto `/dev/block/by-name/kernel` only, after hashing the pushed copy on the phone,
  and record the run. → ADR 0005, D3.
