# 12. Boot the modem with its radio off, for deep sleep

- **Status:** Accepted; D3 amended with the debug `/init`'s reading by issue 70
- **Date:** 2026-11-19
- **Issue:** #54

## Context

ADR 0010, D5 found the SoC never reaching deep sleep under our `/init`: after every
suspend the LPM3 reported `ap:1, modem:1`, `system sleeped 0 times` and `modem:s-0`, and a
suspend drew about 53 mA. Its first rejected alternative, suspending whenever the phone is
not in use, reopens if the suspend draw falls under about 8 mA. Issue 54 asks the four
questions below, with no step on the phone waiting more than 10 minutes. The readings are
in `docs/device-history/phone.md`, *the deep-sleep votes*; the source is
`Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU`.

| Question | Answer |
|---|---|
| 1. What sets the AP's vote and the modem's? | **`ap:1` is no vote**: the AP prints the report awake. **The modem's clears once its firmware runs**, and under our `/init` it never boots |
| 2. Can our `/init` drop them? | **Yes for the modem, by booting it** the way the ROM does: four partitions mounted, `teecd`, one `sysfs` write |
| 3. What owns `GPIO28`? | **Not found**: no device-tree property names it, no handler takes it |
| 4. What does a suspend draw after that? | Under the ROM, **4.7 to 6.7 mA with the radio off**, 7.7 to 9.2 mA with it on; **under the debug `/init`, 2.9 mA** with it off (issue 70) |

`pm_status_show` in `drivers/hisi/pm/hisi_lpregs.c` prints one byte the LPM3 firmware
writes, on the resume path of `drivers/hisi/pm/pm.c`. No Linux driver writes it.

## Decisions

### D1. `ap:1` is read as the AP being awake to print the report

The ROM read `ap:1` on all nine resumes while `system sleeped` rose by 692 over one
suspend. It records nothing about the suspend, and nothing in the AP is left to drop.

### D2. The modem's vote is held by a modem that never booted

`his_modem` (`drivers/hisi/modem/drv/loadm/loadm_phone.c`) waits for userspace to write 1
to `/sys/devices/platform/his_modem/modem_sysboot_start`. Under our `/init` nothing does,
`balong_power` reads `MODEM_STATE_OFF`, and the LPM3 counts the modem awake. Under the ROM
the write came at 17.64 s, and eight of nine reports read `modem:0`.

### D3. Our `/init` boots the modem as the ROM does, with the radio off

What the ROM's `init.balong_modem.rc` and `fstab.modem` do, in order:

1. Mount `modem_secure` on `/mnt/modem/modem_secure`, `modem_om` on `/modem_log`,
   `modem_fw` on `/vendor/modem/modem_fw` read-only, `modemnvm_img` on
   `/mnt/modem/mnvm2:0`, and `secure_storage` on `/sec_storage`.
2. Start `/vendor/bin/teecd`, since the kernel verifies the modem's `sec_*.bin` images
   through the secure OS.
3. Write 1 to `modem_sysboot_start`.
4. Turn the radio off.

With the radio on, the modem's paging woke the SoC about twice a second and the draw sat
on the 8 mA line; with it off, each suspend slept once. The ROM's charger mode boots the
modem with no `rild`, so `rild` is assumed not to be needed for the boot.

**Amended by issue 70**: the debug `/init` did all four steps by hand, with `vendor`
mounted read-only first, the mounts' `context=` options dropped, and a `tmpfs` on `/data`
for `teecd`. `balong_power` read `MODEM_STATE_READY` within 6 s of the write. With no
`rild` the modem comes up at `+CFUN: 0`, and `AT+CFUN=0` on `/dev/appvcom` answers `OK`, so
step 4 is that command, which also holds the radio off if anything had turned it on. A
600 s suspend slept once, read `modem:0`, woke on `RTC0`, and drew 2.9 mA.

The boot writes the `modemnvm_*` partitions through the kernel's `bsp_blk`. None is on ADR
0005, D4's hard-brick list; their roles are recorded and all eight modem partitions are
backed up under `~/p20-backups/`.

**The boot goes into `alarmd` only once the debug `/init` has done it** and a suspend under
our `/init` is read under 8 mA. Then ADR 0010's first rejected alternative is reopened in
its own ADR. Until then ADR 0010, D2 stands.

### D4. `GPIO28` needs no owner

It is line 4 of the `gpio3` pl061 at `e8a0e000`, pin 25 of `pmx0`. It woke the phone only
with the panel lit, never in the ROM's eleven suspends or in ADR 0010's with the panel off,
and ADR 0010, D2 suspends only once the panel goes off.

## Rejected alternatives

**Leaving the modem unbooted, at 53 mA.** It costs nothing to build, and a charge every
day where the ROM shows a week is in reach. *Reopens if* the modem's boot is found to need
more of Android than `teecd`.

**Booting the modem with the radio left on.** It saves finding a radio-off command, at 7.7
to 9.2 mA, on the line ADR 0010 sets. *Reopens if* the radio cannot be turned off without
`rild`, and a suspend under our `/init` reads under 8 mA with it on.

**Writing `modem_sysboot_start` alone**, with no mounts and no `teecd`. The kernel loads
the images from `/vendor/modem/modem_fw` or `/modem_fw` and verifies them through the
secure OS, so the boot would fail. *Reopens if* the kernel's loader is read to take a path
that needs neither.

**Powering the modem off through `balong_power`'s `state`**, so that it holds no vote.
`modem_reset_set` in `drivers/hisi/modem/drv/onoff/bsp_modem_boot.c` calls `system_error`
with `DRV_ERRNO_RESET_REBOOT_REQ` on any write while `bsp_reset_is_connect_ril()` is false,
which reboots the phone. *Reopens if* a booted modem powered off this way is read to report
`modem:0` and to draw less than one with its radio off.

**Stripped Android with a kiosk clock app**, ADR 0002. The ROM reaches deep sleep, at the
draws above. *Reopens if* the modem cannot be booted from our `/init`, and ADR 0010's
power-off has failed in use.

## Consequences

- **ADR 0010 stands.** Its first rejected alternative reopens through D3's reading, not
  through this ADR.
- **A ROM `init` copied with `cp` in TWRP does not boot**, labelled `rootfs`; the manual's
  undo uses `mv`, and now says why.
- Follow-up issues under issue 54: the modem booted from the debug `/init` with its radio
  off, and the suspend draw read (D3); then `alarmd` booting it, after that reading.
- **D3's condition is met under the debug `/init`** (issue 70): the boot goes into
  `alarmd` next, and ADR 0010's first rejected alternative reopens once a suspend under
  `alarmd` reads under 8 mA.
