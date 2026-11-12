# 5. Flash `kernel` from fastboot, and roll back the same way

- **Status:** Accepted
- **Date:** 2026-11-11
- **Issue:** #7

## Context

ADR 0004 boots the kernel already on the phone, from the partition named `kernel`
(`mmcblk0p31`, 25165824 bytes); the phone has no `boot`. Before an image of ours reaches
it, four things had to be read off the phone: whether an image can be tried without being
written, how it is written, how the stock image comes back, and what Huawei's boot-fail
monitor does with a boot that is not Android's. The readings, all of 2026-11-11, are in
`docs/device-history/phone.md`, under *fastboot and the way back*.

The bootloader is `Chipset-chicago9.1.0.6(1GA4)_FASTBOOT`, both lock states `UNLOCKED`.
`recovery_ramdisk` holds TWRP 3.7.0_9-0. The stock image of `kernel` is backed up at
`~/p20-backups/kernel-mmcblk0p31-stock.img`, sha256
`fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84`.

## Decisions

### D1. Every try writes `kernel`; there is no tethered boot

`fastboot boot <image>` downloads the image and then answers `invalid command`. An image
is therefore tried by writing it:

```bash
shasum -a 256 <image>
fastboot flash kernel <image> reboot
```

The bootloader answers no `partition-size`, `partition-type`, `current-slot` or `version`,
so `fastboot` cannot check the image against the partition. The host does: an image is
written only when its sha256 is the one just built and its size is at most 25165824 bytes.
A shorter image leaves the old bytes behind it in the partition; the header's sizes say
where the image ends, and the rollback in D3 writes all 24 MiB.

### D2. Fastboot is reached from the ROM with adb, and from any image with the keys

From a running ROM, `adb reboot bootloader`. From an image that runs no `adbd`, with the
cable in: hold Power until the screen goes black, then hold Volume Down until *Fastboot &
Rescue mode* shows. The long press is the PMIC's, so it works whatever the kernel runs.

On the Mac the bootloader answers one `fastboot` invocation per USB enumeration. A libusb
`reset_device` on `18d1:d00d` re-enumerates it without touching the cable, and goes before
every invocation; several commands chained in one invocation all answer, so a flash and
its reboot are one. The command is in `docs/manual-steps/phone.md`, *Fastboot*.

### D3. The rollback is the stock image written back, and it has been run

```bash
shasum -a 256 ~/p20-backups/kernel-mmcblk0p31-stock.img
fastboot flash kernel ~/p20-backups/kernel-mmcblk0p31-stock.img reboot
adb root
adb shell sha256sum /dev/block/mmcblk0p31
```

Both hashes must be `fe6e372e…6a84`. Run once on purpose: the write took 0.14 s, the ROM
booted, and the partition hashed to the backup.

**When fastboot cannot be reached**, the second way back is TWRP: hold Power until black,
then Volume Up and Power together. It comes up as an adb `recovery` device with a root
shell that reads `mmcblk0p31`. Writing from it would be `adb push` of the stock image and
a `dd` onto `/dev/block/by-name/kernel`; that write has not been tried, and is the one
command in this ADR that still needs a yes each time (D4).

### D4. What an agent runs on its own

On 2026-11-11 the operator gave standing permission for `adb` and `fastboot`: "on adb and
fastboot you have full access … allways". Within that, the line is drawn by what can
brick the phone, not by whether a command writes:

| Runs without asking | Never runs |
|---|---|
| `adb devices`, `adb shell` reads, `adb root`, `adb reboot` and `adb reboot bootloader` | a write to any partition but `kernel` |
| `fastboot devices`, `getvar`, `oem get-product-model`, `oem get-build-number`, `oem lock-state info` | `fastboot erase`, `format`, `flashing lock`, `oem lock`, or anything that relocks |
| `fastboot reboot`, `reboot-bootloader` and the USB reset | `fastboot flash` of an image whose sha256 and size were not checked first (D1) |
| `fastboot flash kernel` of an image that passed D1, the stock image included | a `dd` that writes on the phone, without a yes |

A `dd` that writes on the phone, from the ROM or from TWRP, needs a yes each time:
its `of=` can name any partition, and `fastboot flash` can only name the one given.

### D5. The boot-fail monitor counts only what raises an error

In D1's kernel source (ADR 0004), `hw_bfm_enable=1` starts the boot timer at kernel init,
before any userspace, and nothing needs to talk to `/dev/hw_bfm` for it to run. When its
10-minute timer runs out and there is no `/eng` directory, the kernel itself sets
`STAGE_BOOT_SUCCESS`. A failure is recorded only when something calls `boot_fail_err`,
such as a kernel-range error; `bfr_core.c` maps those to eRecovery, with downloading a
recovery and a factory reset among its methods.

So an image of ours is counted as booted after ten minutes up, if its initramfs never
creates `/eng`, and a try that panics or is cut short before that may be counted as
failed. How many failures the bootloader takes before it starts eRecovery, and whether
eRecovery touches `kernel`, is not in the kernel source. It is read off the phone the
first time an image of ours fails: from eRecovery, accept nothing, reach fastboot with D2's
keys, and run D3. If it gets in the way, the remedy is ADR 0004, D4's rebuild.

## Rejected alternatives

**A tethered `fastboot boot`.** The bootloader refuses it with `invalid command` after
the download. *Reopens if* `fastboot boot` answers anything else, which would need a
bootloader other than `Chipset-chicago9.1.0.6(1GA4)_FASTBOOT`.

**Replugging the cable before every `fastboot` invocation.** It works, and needs a
person's hands at every step. *Reopens if* the libusb reset stops re-enumerating the
phone.

**Writing `kernel` with `dd` from the ROM, over `adb root`.** It works only while the
ROM's own image is in `kernel`, since an image of ours runs no `adbd`, and `dd`'s `of=`
can name any partition. *Reopens if* fastboot can no longer be reached and the ROM still
boots.

**TWRP as the first way to write.** It needs the phone in recovery, which is two steps
from fastboot, and its write is a `dd` (D4). It stays the second way back. *Reopens if*
`fastboot flash kernel` stops working.

**Asking before every write, as `AGENTS.md` says today.** The operator turned it down for
`adb` and `fastboot`; the boundaries in D4 are what keep a write recoverable, and asking
added none. *Reopens if* a write ever lands outside `kernel`, or a rollback ever fails to
hash to the backup.

**Huawei's own update paths** — HiSuite, `dload` from an SD card, eRecovery's download.
Each rewrites far more than `kernel`. *Reopens* never as a way to try an image; at most as
a last resort that gives up the custom ROM.

## Consequences

- **The try, the flash and the rollback are in `docs/manual-steps/phone.md`**, and are
  repeated by a `flashing-the-phone` skill, filed under this issue.
- **A device preflight** in `scripts/` checks, before every flash, that the device is
  `ANE-LX1`, both lock states are `UNLOCKED`, the backup is present and hashes to the
  value above, and the image passes D1. The USB reset belongs to it, and the PR that adds
  it answers ADR 0001, D7 for `pyusb`. Filed under this issue.
- **`AGENTS.md`'s "Ask before any command that writes to the phone"** is narrowed to D4's
  table, and `docs/SECURITY.md`'s *Bricking* with it. The narrowing is filed under this
  issue, after issue 16 names `kernel` in the same section.
- **No separate `using-the-phone` skill.** Reading the phone over adb is D4's left column;
  reading it over the debug lifeline is issue 15's.
- **The initramfs never creates `/eng`**, so the kernel itself counts the boot as
  finished (D5).
