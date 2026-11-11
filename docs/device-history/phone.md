# The phone — the record

## 2026-11-11 — first readings, for issue 6

Read over `adb` from the ROM already on the phone, with `adb root` on its `userdebug`
build. Nothing was written to the phone. The serial number, `productid` and partition
UUIDs are left out of every reading below.

**The ROM.**

```
ro.product.device=anne   ro.hardware=hi6250   ro.build.version.release=9
ro.lineage.version=0.23-p-20250919-UNOFFICIAL-anne   ro.build.type=userdebug
ro.debuggable=1   ro.treble.enabled=true   devmodel=ANE-L01
```

**The kernel.**

```
Linux version 4.9.319-Abdelhay-KSU- (root@540b7bd55571) (gcc version 7.5.0 (Linaro GCC
7.5-2019.12) ) #1 SMP PREEMPT Mon Apr 1 00:30:20 UTC 2024
```

KernelSU is built in (`CONFIG_KSU=y`, package `me.weishu.kernelsu`); `su` is not on the
shell's path, so root came from `adb root`.

**The boot partition is `kernel`.** There is no partition named `boot`:

```
kernel            -> /dev/block/mmcblk0p31      25165824 bytes
boot_a            -> /dev/block/platform/hi_mci.0/by-name/kernel_a   (no such partition)
recovery_ramdisk  -> mmcblk0p33   recovery_vendor -> mmcblk0p34   recovery_vbmeta -> mmcblk0p38
erecovery_kernel  -> mmcblk0p28   erecovery_ramdisk -> mmcblk0p29   vbmeta -> mmcblk0p42
dts -> mmcblk0p35   dto -> mmcblk0p36   fastboot -> mmcblk0p5   nvme -> mmcblk0p7
oeminfo -> mmcblk0p8   vrl -> mmcblk0p1   vrl_backup -> mmcblk0p2
```

There is no `ramdisk` partition either.

**The image in `kernel`**, parsed from the copy:

```
magic=ANDROID! header_version=0 page_size=2048 os_version=0x12000000
kernel_size=12966737 (gzip, arm64 Image of 35448840 bytes, no appended DTB)
kernel_addr=0x480000 ramdisk_addr=0x10400000 second_addr=0x1300000 tags_addr=0x7e00000
ramdisk_size=0 second_size=0 name="" no AVB footer
cmdline="loglevel=4 coherent_pool=512K page_tracker=on slub_min_objects=12
  unmovable_isolate1=2:192M,3:224M,4:256M printktimer=0xfff0a000,0x534,0x538
  androidboot.selinux=enforcing buildvariant=user"
12969984 of 25165824 bytes used; 262900 non-zero bytes in the 1 MiB after them
```

The addresses are base `0x478000` plus the offsets `0x8000`, `0xff88000`, `0xe88000` and
`0x7988000`. The decompressed kernel contains the `skip_initramfs` parameter.

**`/proc/cmdline`**, as the bootloader extends it, the parts that bear on the image:

```
… buildvariant=user androidboot.swtype=normal
fastboot_version=Chipset-chicago9.1.0.6(1GA4)_FASTBOOT androidboot.mode=normal
bootlock=unlocked userlock=unlocked hw_bfm_enable=1 hw_bfr_enable=1
androidboot.verifiedbootstate=orange skip_initramfs root=PARTUUID=<system> init=/init
console=ttyS0 …
```

**The running config**, from `/proc/config.gz`, the options issue 6 asks about:

```
CONFIG_IKCONFIG_PROC=y       CONFIG_DEVTMPFS=y         # CONFIG_DEVTMPFS_MOUNT is not set
CONFIG_BLK_DEV_INITRD=y      CONFIG_RD_GZIP=y          CONFIG_RD_LZ4=y
CONFIG_INITRAMFS_SOURCE=""   CONFIG_CMDLINE="console=ttyAMA0"  (neither EXTEND nor FORCE)
CONFIG_MODULES=y
CONFIG_RTC_CLASS=y           CONFIG_RTC_INTF_DEV=y     CONFIG_RTC_INTF_SYSFS=y
CONFIG_RTC_HCTOSYS=y         CONFIG_RTC_DRV_HISI_PMIC=y
CONFIG_PM_SLEEP=y            CONFIG_SUSPEND=y          CONFIG_PM_WAKELOCKS=y
CONFIG_PM_AUTOSLEEP=y        CONFIG_PM_DEBUG=y
CONFIG_USB_CONFIGFS=y        CONFIG_USB_CONFIGFS_RNDIS=y   CONFIG_USB_CONFIGFS_ACM=y
CONFIG_USB_CONFIGFS_F_FS=y   # CONFIG_USB_CONFIGFS_NCM is not set
# CONFIG_USB_CONFIGFS_ECM is not set   # CONFIG_USB_CONFIGFS_EEM is not set
CONFIG_PSTORE=y              CONFIG_PSTORE_RAM=y       CONFIG_PSTORE_CONSOLE=y
CONFIG_PSTORE_PMSG=y
# CONFIG_DRM is not set      CONFIG_FB=y               CONFIG_HISI_FB_6250=y
CONFIG_HUAWEI_LCD_PANEL=y    CONFIG_LCDKIT_DRIVER=y
CONFIG_INPUT_EVDEV=y         CONFIG_HUAWEI_TS=y
CONFIG_HISI_VIBRATOR=y       CONFIG_INPUTHUB_VIBRATOR_CHANNEL=y
CONFIG_HW_BFMR_HISI=y        # CONFIG_VT is not set
```

**Device nodes**, as the shell user sees them:

```
/dev/rtc0 crw-r----- system system 252,0
/dev/graphics/fb0 fb1 fb2      /sys/class/graphics/fb0 fb1 fb2      no /dev/dri
no /sys/class/timed_output
```

`/sys/class/rtc`, `/sys/power/state` and `/sys/class/leds` refused the shell user, and
were not read as root.

**The backup.** The whole of `kernel` was streamed to the laptop with
`adb exec-out 'dd if=/dev/block/mmcblk0p31 bs=1048576'`, into
`~/p20-backups/kernel-mmcblk0p31-stock.img`, off the repository. Both ends hashed alike:

```
fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
```

## 2026-11-11 — fastboot and the way back, for issue 7

**Before leaving the ROM**, `kernel` still matched the backup:

```
adb shell sha256sum /dev/block/mmcblk0p31
fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
```

**Recovery is TWRP.** `recovery_ramdisk` (`mmcblk0p33`, 32 MiB) holds a header-v0 image
with no kernel and a 16023684-byte gzip ramdisk whose properties read
`ro.build.display.id=omni_anne-eng 16.1.0 PQ2A.190405.003` and `ro.twrp.version`. It was
copied to a scratch directory to be read, never into the repository. `erecovery_kernel`
(`mmcblk0p28`, 24 MiB) and `erecovery_ramdisk` (`mmcblk0p29`, 32 MiB) were not read.

**The boot-fail monitor.** The partitions it keeps state in are `bootfail_info`
(`mmcblk0p49`) and `rrecord` (`mmcblk0p50`); its device is `/dev/hw_bfm`. The ROM starts
`/system/bin/bfm-ctl` (`init.hi6250.rc`, `class main`, oneshot), whose only strings are
`/dev/hw_bfm` and `BFMR_ENABLE_CTRL ioctl failed!`. `ro.boot.bootreason=shutdown,userrequested`.

In D1's source tree (`drivers/hwbfm/bfm/core/bfm_timer.c`), the boot timer runs a
10-minute short and a 30-minute long timeout; when one expires it raises
`KERNEL_BOOT_TIMEOUT` only if the directory `/eng` exists, and otherwise sets the stage to
`STAGE_BOOT_SUCCESS`. `boot_fail_err` returns early while `hw_bfm_enable` is off. What the
bootloader counts across reboots is not in the kernel source.

**Fastboot.** `adb reboot bootloader` reaches it. The bootloader enumerates on the Mac as
`Fastboot2.0`, `18d1:d00d`, and answers:

```
fastboot oem get-product-model     (bootloader) ANE-LX1
fastboot oem get-build-number      (bootloader) :ANE-LX1 9.1.0.401(C432E13R1P7)
fastboot oem lock-state info       (bootloader)  FB LockState: UNLOCKED
                                   (bootloader)  USER LockState: UNLOCKED
fastboot getvar max-download-size  max-download-size: 471859200
fastboot getvar all                FAILED (remote: 'invalid command')
fastboot getvar product            FAILED (remote: 'invalid command')
fastboot getvar unlocked           FAILED (remote: 'invalid command')
```

**One invocation per enumeration.** After the first answered `fastboot` invocation, every
later one hung with no output until killed, `reboot-bootloader` included, while
`fastboot devices` still listed the phone. Unplugging and replugging the cable restored one
invocation, printing `ERROR: could not clear output pipe; result e00002ed, ignoring....`
before the answer. A libusb `reset_device` on `18d1:d00d` from the Mac does the same
without touching the cable: four reset-then-query rounds in a row all answered, and a
query sent without a reset in between hung. Within one invocation, `fastboot getvar
max-download-size getvar max-download-size` answered both. `fastboot` 37.0.1-15733141,
libusb 1.0.30, from Homebrew.
