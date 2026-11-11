# 4. Boot the kernel already on the phone

- **Status:** Accepted
- **Date:** 2026-11-11
- **Issue:** #6

## Context

ADR 0002 keeps a vendor kernel and replaces the userspace, and leaves open which build.
There were three candidates: the kernel in the image the phone boots today, a postmarketOS
`huawei-anne` port, and Huawei's published source. Each was read on 2026-11-11; the
phone's own readings are in `docs/device-history/phone.md`.

The phone runs an unofficial LineageOS build for Android 9. Its image is in the partition
named `kernel` — the phone has no `boot` partition — and holds a header-v0 image with a
gzip kernel, `4.9.319-Abdelhay-KSU-`, and no ramdisk. The bootloader boots it
system-as-root: it appends `skip_initramfs root=PARTUUID=<system>` to the command line,
and the kernel honours `skip_initramfs`.

## Decisions

### D1. Boot the kernel that is in `kernel` today, unchanged

The image keeps the gzip kernel from the stock image byte for byte, with the header's base
`0x478000`, its offsets, page size 2048, header version 0 and command line. It is the only
candidate known to drive this phone's panel, touch, RTC and suspend, and its config
already has every option the image needs (D2). Its source is
`github.com/Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU`, 4.9.319; the exact commit
is not recorded in the binary.

### D2. The config has what the image needs

| The image needs | The running kernel has |
|---|---|
| `/dev` without a userspace | `DEVTMPFS=y`; `DEVTMPFS_MOUNT` is off, so `alarmd` mounts it |
| an initramfs | `BLK_DEV_INITRD=y`, `RD_GZIP=y`, `RD_LZ4=y` — but see D3 |
| the RTC, with a wake alarm | `RTC_DRV_HISI_PMIC=y`, `RTC_INTF_DEV=y`, `RTC_INTF_SYSFS=y`; in D1's source the driver implements `set_alarm` and `alarm_irq_enable` and marks itself a wakeup source |
| suspend | `PM_SLEEP=y`, `SUSPEND=y`, with `PM_WAKELOCKS` and `PM_AUTOSLEEP` |
| a USB lifeline | `USB_CONFIGFS=y` with `RNDIS`, `ACM` and `F_FS`; no `NCM`, `ECM` or `EEM` |
| a log that survives a reboot | `PSTORE_RAM=y`, `PSTORE_CONSOLE=y`, `PSTORE_PMSG=y` |
| the panel | `HISI_FB_6250=y` — fbdev at `/dev/graphics/fb0`; `DRM` is off |
| touch, buttons, vibrator | `INPUT_EVDEV=y`, `HUAWEI_TS=y`, `HISI_VIBRATOR=y` |

**USB networking is RNDIS only, and macOS has no RNDIS driver.** The debug lifeline uses
the ACM serial function, which macOS drives natively, or RNDIS to a Linux host; SSH over
NCM to the Mac would need a rebuilt kernel (D4).

### D3. The first image proves its initramfs runs; a renamed parameter is the fallback

The bootloader may add `skip_initramfs` only because today's image carries no ramdisk.
That is an assumption: LineageOS 20 for `anne` packs its ramdisk into the `kernel` image
(`BOARD_CUSTOM_BOOTIMG_HAS_RAMDISK := false` in `android_device_huawei_hi6250-9-common`)
and boots a kernel whose `init/initramfs.c` honours the flag, which works only if the
bootloader drops it. It has not been observed on this phone.

The first image with a ramdisk therefore proves it: its `/init` writes a marker to
`/dev/kmsg` and reboots, and the marker is read from `/sys/fs/pstore` once the stock image
is back. A skipped initramfs fails safe — the kernel mounts `system` and the ROM boots.

If the flag survives, host tooling renames the `skip_initramfs` setup string inside the
decompressed kernel to one of the same length, as Magisk does on system-as-root devices,
and recompresses it. That is the only change to the kernel this ADR allows.

### D4. A rebuilt kernel waits for a need the running one cannot meet

Both 4.9 trees build with published toolchains — the running kernel's with Linaro GCC 7.5,
LineageOS 20's with clang `r416183b` — from the config in `/proc/config.gz`. Neither build
has been tried here. A rebuild costs a cross toolchain on the laptop and in CI, and a
kernel nobody has booted on this phone. It is taken only for one of: SSH to the Mac over
NCM or ECM, Huawei's boot-fail monitor getting in the way, or D3's rename failing.

## Rejected alternatives

**A postmarketOS `huawei-anne` port.** pmaports has no package for it on 2026-11-11, in
any of `device/testing`, `community`, `downstream` or `archived`. The nearest relatives,
the P10 Lite (`warsaw`) and Honor 6X (`berlin`) on the same `hi6250` platform, ran a
4.4.174 downstream kernel and were archived as unmaintained in June 2026. The wiki page
could not be fetched. *Reopens if* a `huawei-anne` device package, or a mainline `hi6250`
kernel that drives the panel, touch, RTC and suspend, lands in pmaports.

**Huawei's GPL source, built as published.** No ANE-L01 archive was found on Huawei's open
source page; the `hi6250` 4.9.148 tarball mirrored in `hisi-oss/kernel_tarballs` is the
ancestor of both community trees, which carry the fixes it needs for today's compilers.
*Reopens if* the community trees disappear, or a distributed image must name Huawei's own
tarball as its source.

**LineageOS 20's kernel, `hisi-oss/android_kernel_huawei_hi6250-9` (4.9.148).** Its
defconfig turns on the same drivers — the PMIC RTC, configfs RNDIS, `PSTORE_RAM`,
`HISI_FB_6250`, `HUAWEI_TS`, `HISI_VIBRATOR` — but it is not the kernel on the phone, and
booting it would be a second unknown beside the image's own. *Reopens if* D4 calls for a
rebuild; it is then a candidate beside the running kernel's source, and the one that
builds and boots wins.

## Consequences

- **The boot image is in `kernel`, not `boot`.** `AGENTS.md`'s device-safety rule names a
  partition this phone does not have, and is corrected under this issue.
- **Huawei's boot-fail monitor is on** (`HW_BFMR_HISI=y`, `hw_bfm_enable=1`,
  `hw_bfr_enable=1`). An image that never reports a finished Android boot may be counted
  as failed and sent to eRecovery, which is never written and restores the phone. The
  flashing research, issue 7, measures it.
- **The corresponding source** of a distributed image is the running kernel's tree in D1,
  not Huawei's.
- **The panel is fbdev**, so the display adapter writes `/dev/graphics/fb0`.
