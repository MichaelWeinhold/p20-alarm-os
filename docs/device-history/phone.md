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

**No tethered boot.** `fastboot boot ~/p20-backups/kernel-mmcblk0p31-stock.img` sent the
image and the bootloader refused to run it:

```
Sending 'boot.img' (24576 KB)                      OKAY [  0.682s]
Booting                                            FAILED (remote: 'invalid command')
```

**The rollback, run once on purpose.** The backup hashed to the sha256 above, the USB was
reset, and the stock image was written back to `kernel` in one invocation:

```
fastboot flash kernel ~/p20-backups/kernel-mmcblk0p31-stock.img reboot
Warning: skip copying kernel image avb footer (kernel partition size: 0, kernel image size: 25165824).
Sending 'kernel' (24576 KB)                        OKAY [  0.658s]
Writing 'kernel'                                   OKAY [  0.141s]
Rebooting                                          OKAY [  0.029s]
```

The warning is the host's: the bootloader answers no `partition-size:kernel`, so
`fastboot` cannot look for an AVB footer, and the image has none. The ROM came back to
`sys.boot_completed=1` with `4.9.319-Abdelhay-KSU-`, `ro.boot.bootreason=bootloader`, and:

```
adb shell sha256sum /dev/block/mmcblk0p31
fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
```

**The boot-fail monitor, read further in D1's source.** With `hw_bfm_enable=1` on the
command line, `bfmr_init` registers `/dev/hw_bfm` and starts the boot timer at kernel
init, before any userspace runs; nothing has to send an ioctl for it to count. On expiry
without `/eng` the kernel itself sets `STAGE_BOOT_SUCCESS`, which stops the timer. A
failure is recorded only when something raises `boot_fail_err`; `bfr_core.c` maps
kernel-range error numbers to `ENTER_ERECOVERY_REASON_BECAUSE_KERNEL_BOOT_FAIL`, with
`FRM_GOTO_ERECOVERY_DOWNLOAD_RECOVERY` and `FRM_GOTO_ERECOVERY_FACTORY_RESET` among the
methods. How many recorded failures the bootloader takes before it starts eRecovery is not
in the kernel; it is read off the phone when an image of ours first fails.

**Fastboot answers nothing about partitions.** `getvar partition-size:kernel`,
`partition-type:kernel`, `is-userspace`, `current-slot` and `version` all answer
`invalid command`.

**The keys.** From fastboot, with the cable in: holding Power until the screen went black,
then Volume Down, brought the phone back to *Fastboot & Rescue mode*, and `fastboot oem
lock-state info` answered after a USB reset. Holding Power until black, then Volume Up and
Power together, started TWRP, which came up on the Mac as
`recovery product:omni_anne model:P20_Lite device:anne` with `ro.twrp.version=3.7.0_9-0`
and a root shell (`uid=0`, `u:r:su:s0`). From there `/dev/block/by-name/kernel` pointed
at `mmcblk0p31`, which hashed to the backup's sha256; `adb reboot` returned to the ROM.
Nothing was written from TWRP.

## 2026-11-12 — the device preflight, for issue 19

Run on the Mac, with the phone in *Fastboot & Rescue mode* after `adb reboot bootloader`.
Nothing was written to the phone.

**Where fastboot answers.** Each invocation, after its own USB reset, wrote every line to
stderr and nothing to stdout; the recordings are `tests/fixtures/fastboot/`. An invocation
sent without a reset hung until `timeout 10` killed it, exit 124. An unknown `oem` command
answered `FAILED (remote: 'invalid command')`, exit 1.

**The reset without pyusb.** `libusb_init`, `libusb_open_device_with_vid_pid` on
`18d1:d00d`, `libusb_reset_device` and `libusb_close`, called through `ctypes` on
Homebrew's `libusb-1.0.dylib`, returned 0, and the next invocation answered. PyPI has no
type stubs for `pyusb` under any name.

**The preflight against the backup**, `uv run python scripts/device_preflight.py
~/p20-backups/kernel-mmcblk0p31-stock.img`, exit 0:

```
image …/kernel-mmcblk0p31-stock.img: 25165824 bytes, sha256 fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
backup …/kernel-mmcblk0p31-stock.img: sha256 fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
device: ANE-LX1
lock state: FB UNLOCKED, USER UNLOCKED
ready: the next fastboot invocation is answered
```

`fastboot getvar max-download-size`, sent next with no reset of its own, answered
`471859200`. The same after `device_preflight.py --reset`, with `oem lock-state info`.

**The preflight against an image one byte over**, a 25165825-byte sparse file, exit 1,
before any USB reset:

```
the image …/oversized.img is 25165825 bytes, over kernel's 25165824
```

## 2026-11-12 — the flashing-the-phone skill, for issue 20

The skill followed once end to end, with `~/p20-backups/kernel-mmcblk0p31-stock.img` as
the image, so the flash was also the rollback. The phone was already in *Fastboot & Rescue
mode*: `adb devices` listed nothing, and `device_preflight.py --reset` exited 0.

**The preflight**, exit 0, printed the image's sha256 as
`fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84`, `ANE-LX1`, and `FB
UNLOCKED, USER UNLOCKED`.

**The flash**, as the next invocation, exit 0:
`timeout 120 fastboot flash kernel ~/p20-backups/kernel-mmcblk0p31-stock.img reboot`

```
Warning: skip copying kernel image avb footer (kernel partition size: 0, kernel image size: 25165824).
Sending 'kernel' (24576 KB)                        OKAY [  0.686s]
Writing 'kernel'                                   OKAY [  0.125s]
Rebooting                                          OKAY [  0.029s]
```

**After it**, the ROM booted (`sys.boot_completed` 1), and after `adb root`, `adb shell
sha256sum /dev/block/mmcblk0p31` read
`fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84`.

## 2026-11-12 — the boot image round trip, for issue 8

Run on the Mac against the backup; nothing touched the phone. The output went to a scratch
directory, never into the repository.

**Unpack**, `uv run python scripts/boot_image.py unpack
~/p20-backups/kernel-mmcblk0p31-stock.img <scratch>/unpacked`, exit 0:

```
kernel: 12966737 bytes
image …/kernel-mmcblk0p31-stock.img: header v0, page size 2048, 12969984 bytes, sha256 a8d27ec65ab9d6596980a9aad00d8f22cd0fe70606221058d684f374b605f686
id: 7e5a3479aedd9e5e7bf1685b71ecca0cd7bc831b, the sha1 of its sections
trailing: 12195840 bytes, 262900 of them non-zero
```

The stored `id` is the SHA-1 `mkbootimg` computes over the kernel, the empty ramdisk and
second stage, and their sizes, so `repack` recomputes it rather than carrying it. Every
byte of the header and the kernel's page that no field holds is zero.

**Repack**, `uv run python scripts/boot_image.py repack <scratch>/unpacked
<scratch>/repacked.img`, exit 0:

```
image …/repacked.img: 12969984 bytes, sha256 a8d27ec65ab9d6596980a9aad00d8f22cd0fe70606221058d684f374b605f686
```

`head -c 12969984 ~/p20-backups/kernel-mmcblk0p31-stock.img | shasum -a 256` read the same
`a8d27ec6…f686`, and the backup still hashed to `fe6e372e…6a84`. The repacked image is
the stock image without the partition's leftover tail, which booted the ROM on the
rollback above; it was not flashed.

## 2026-11-12 — an initramfs in `kernel`, for issue 17

**Before**, in the ROM, `kernel` hashed to `fe6e372e…6a84`, `/sys/fs/pstore` was mounted
(`console-ramoops-0`, `pmsg-ramoops-0`), and `console-ramoops-0` held no `p20-alarm-os`
line. Its last lines were the previous boot's `adb reboot bootloader`:

```
[hisi_poweroff]:hisi_pm_system_reset cmd is bootloader
[blackbox]:[set_reboot_reason]:set reboot_reason is 0x1
```

**The image** was the stock image with a 181604-byte gzip `newc` ramdisk added — `dev`,
`dev/console`, `dev/null`, `proc` and the `initramfs-probe` binary, 334832 bytes, static,
as `init` — by `uv run python scripts/initramfs.py
target/aarch64-unknown-linux-musl/release/initramfs-probe
~/p20-backups/kernel-mmcblk0p31-stock.img <scratch>/probe.img`:

```
image …/probe.img: 13152256 bytes, sha256 2e206472f970fab6788778b3c30a674c83662b6fd50d4c301464f688c986e83d
```

Unpacked again, its kernel was the stock image's 12966737 bytes and its header fields the
stock ones. The preflight exited 0 and printed that sha256, then:

```
timeout 120 fastboot flash kernel <scratch>/probe.img reboot
Sending 'kernel' (12844 KB)                        OKAY [  0.351s]
Writing 'kernel'                                   OKAY [  0.103s]
Rebooting                                          OKAY [  0.029s]
```

**The ROM came up**, on adb 37 s after the reboot, with `sys.boot_completed` 1 and the
first 13152256 bytes of `mmcblk0p31` hashing to `2e206472…e83d`. `/proc/cmdline` still
carried `skip_initramfs root=PARTUUID=<system> init=/init`, and `console-ramoops-0` held
no `p20-alarm-os` line, so `/init` never ran.

**The bootloader never reads the image's ramdisk.** Its own log, which the ROM copies into
the kernel log as `[fastbootlog]:current_bootloader_log:`, read for this boot:

```
load_kernel: avb verify failed: 3
load_kernel: kernel load @ 0x00480000 (0x00c5db51 bytes), read from emmc kernel @ 0x00000800
load_kernel: load_decompress_kernel, lasted 485 ms
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition ramdisk mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition eng_system mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition eng_vendor mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition recovery_ramdisk mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition recovery_vendor mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition erecovery_ramdisk mode 0
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition erecovery_vendor mode 0
load_kernel: cmdline size = 1797
```

It takes `kernel_size` bytes from `kernel` at the first page and nothing else from that
partition; the ramdisk comes from a partition of its own, and for a normal boot that is
`ramdisk`, which this phone does not have. `/dev/block/by-name` lists `kernel`,
`recovery_ramdisk`, `recovery_vendor`, `erecovery_ramdisk`, `erecovery_vendor`,
`eng_system` and `eng_vendor`, and no `ramdisk`. With 0 bytes loaded, the bootloader adds
`skip_initramfs`. Renaming the parameter inside the kernel (ADR 0004, D3's fallback) would
not help: there is no ramdisk for the kernel to unpack.

`fastboot flash kernel` also logged `hisi_secboot_download_verification: kernel vrl copy
error ret(0x000f0003)!` before `partition 'kernel' updated`; the write went through, and
the image's kernel booted.

**The rollback**, with the preflight exiting 0 on the backup's `fe6e372e…6a84`:

```
timeout 120 fastboot flash kernel ~/p20-backups/kernel-mmcblk0p31-stock.img reboot
Sending 'kernel' (24576 KB)                        OKAY [  0.691s]
Writing 'kernel'                                   OKAY [  0.142s]
Rebooting                                          OKAY [  0.029s]
```

The ROM came back to `sys.boot_completed` 1, and `mmcblk0p31` hashed to
`fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84`.

**Recovery mode does get a ramdisk.** `adb reboot recovery` started TWRP, running the
same kernel out of `kernel` — `uname -r` `4.9.319-Abdelhay-KSU-`, the same `Linux
version` line — with its ramdisk from `recovery_ramdisk`. Its `/proc/cmdline`, 1633 bytes,
carried `loglevel=4` and `androidboot.mode=normal` and no `skip_initramfs`, `root=` or
`init=`, and the kernel logged `Freeing initrd memory: 75768K`. TWRP's kernel log held no
`fastbootlog` lines to say which partitions the ramdisk was read from. Nothing was written
from TWRP before the `misc` reading below.

**`adb reboot` from TWRP came back to TWRP**, twice in a row, because `adb reboot recovery`
from the ROM had left the bootloader control block in `misc` (`mmcblk0p20`, 2097152
bytes) set, and the bootloader honours it on every boot until it is cleared:

```
dd if=/dev/block/by-name/misc bs=64 count=1 | od -c
0000000   b   o   o   t   -   r   e   c   o   v   e   r   y  \0  \0  \0
```

`misc` was streamed to `~/p20-backups/misc-mmcblk0p20-boot-recovery.img` first, both ends
hashing to `8c712148fdf3a771f4e19e2876a86b9428ee57df00dd540db54f2f81031402d4`. From TWRP,
`dd if=/dev/zero of=/dev/block/by-name/misc bs=32 count=1` zeroed the 32-byte command
field — TWRP's `dd` refuses `conv=notrunc` — and everything after byte 32 still hashed to
the backup's (`794dbcb5…e1e2`). `adb reboot` then reached the ROM, `sys.boot_completed` 1,
with `mmcblk0p31` at `fe6e372e…6a84` and the command field all zero.

## 2026-11-12 — what a normal boot runs as `/init`, for issue 33

In the ROM — `lineage_anne-userdebug`, `ro.lineage.display.version`
`0.23-p-20250919-UNOFFICIAL-anne` — `/proc/cmdline` puts the image header's command line
first (`loglevel=4 coherent_pool=512K … buildvariant=user`) and the bootloader's
parameters after it, among them, in this order:

```
androidboot.veritymode=enforcing efuse_status=0 skip_initramfs root=PARTUUID=f9f21f33-a8d4-5f0e-9746-594869aec34e
… init=/init eng_system=PARTUUID=… eng_vendor=PARTUUID=… cache=PARTUUID=…/recovery/ patch=PARTUUID=…
```

The command line has no `dm=` table and no `rw`. **`system`** is `mmcblk0p52`, 7634944
sectors (3909091328 bytes), and `/proc/mounts` reads `/dev/root / ext4 ro,seclabel,relatime`:
the kernel mounts it read-only as `/`, by `PARTUUID`, and runs its `/init`, the ROM's
init, `-rwxr-x--- root shell 2216944`. Beside it sit `/init.rc`, `/init.environ.rc`,
`/init.usb.rc`, `/init.usb.configfs.rc` and the two `init.zygote*.rc`. Nothing was
written.

## 2026-11-12 — our `/init` on `system`, for issue 17

**`system`'s role**: the root the kernel mounts read-only on a normal boot and whose
`/init` it runs (above). It was streamed off the phone before its first write,
`adb exec-out 'dd if=/dev/block/mmcblk0p52 bs=4194304'`, to
`~/p20-backups/system-mmcblk0p52-stock.img`, 3909091328 bytes, both ends hashing to
`4d61f9e3a12ddcba2fdd425b5661510aa604c254ae5d33353d2ce5c525412047`. The ROM's `/init`
hashed to `e8e2768ea78c453caf783281cfe835b95fc4f72f73928775d4f30609761978ef`, `-rwxr-x---
root shell`, `u:object_r:init_exec:s0`.

**The swap**, from TWRP, ADR 0006, D2: `mount -o rw /dev/block/mmcblk0p52 /mnt/sys`,
`mv /mnt/sys/init /mnt/sys/init.rom`, the probe pushed to `/mnt/sys/init` with mode 0755,
`umount`, `misc`'s command field zeroed, `adb reboot`. `system`'s root already holds an
empty `/dev` and `/proc`.

**The first probe killed the kernel.** The phone came up in eRecovery, not TWRP; it was
left without choosing anything and taken to TWRP with the keys. The cold boot had emptied
pstore, but Huawei's `dfx` partition (`mmcblk0p41`, read with `dd`) held the boot's kernel
log:

```
[   14.022216s][pid:1,cpu2,swapper/0]Warning: unable to open an initial console.
[   14.030303s][pid:1,cpu2,swapper/0]EXT4-fs (mmcblk0p52): couldn't mount as ext3 due to feature incompatibilities
[   14.035125s][pid:1,cpu1,swapper/0]Kernel init end, jump to execute /init
[   14.045806s][pid:1,cpu0,init]Kernel panic - not syncing: Attempted to kill init! exitcode=0x0000000b
```

and the bootloader's, on the boot after it:

```
bfm_save_log_to_bfi_and_try_to_recovery line: 1691, bootfail_errno: 0x03000001, hisi_reboot_reason: 0x00000060, bfmr_bootstage: 0x03000001, suggested_recovery_method 0
recovery: bootmode is 1!
load_kernel: ramdisk body load @ 0x10400000 (0x00000000 bytes), read from partition erecovery_ramdisk mode 0
```

So the kernel mounted `system` with the changed `/init` and ran it — no verity refused it
— but **file descriptors 0 to 2 were closed**: the kernel found no `/dev/console` to open.
Rust's start-up, before `main`, reopens a closed standard descriptor on `/dev/null` and
aborts when it cannot; `system` has no `/dev/null`. In pid 1 the `SIGABRT` is ignored, and
musl's `abort` falls through to a fault, `SIGSEGV`, exit code `0xb`, before the marker. In
TWRP, `chroot /mnt/sys /init 0<&- 1>&- 2>&-` exited 134, `SIGABRT`, with nothing logged.
One panicking `/init` was enough for the bootloader to start eRecovery on the next boot.

**The second probe** is built `#![no_main]`, with an `extern "C" fn main` that musl calls
directly and no Rust start-up, `e2048b3f885f724b476a400bfe4b0515deb4481a70301a76a599dd52ccaa83ed`,
325288 bytes. In TWRP, from a `chroot` of a `tmpfs` holding only it and an empty `/dev`,
with 0 to 2 closed, it logged its marker and rebooted; `misc` was clear, and the bootloader
started TWRP with `reboot_reason=recovery`, not eRecovery.

**As `/init` on `system`** it ran as pid 1 on a normal boot. TWRP's
`console-ramoops-0` then read:

```
[   13.532470s][pid:1,cpu7,swapper/0]Warning: unable to open an initial console.
[   13.546173s][pid:1,cpu4,swapper/0]Kernel init end, jump to execute /init
[   13.553863s][pid:1,cpu7,init]p20-alarm-os: /init ran from system
[   13.554931s][pid:1,cpu5,init]p20-alarm-os: cmdline 1/5: loglevel=4 coherent_pool=512K … enter_recovery=0 androidboot.mode=normal …
```

with the rest of the command line in parts 2 to 5, `skip_initramfs
root=PARTUUID=f9f21f33-…` and `init=/init` among them, and TWRP's own `/proc/cmdline`
carrying `reboot_reason=recovery`. From the kernel's jump to the marker took 7.7 ms.

**The way back**, from TWRP: `rm /mnt/sys/init`, `mv /mnt/sys/init.rom /mnt/sys/init`,
which hashed to `e8e2768e…78ef` with its label kept, `umount`, `adb reboot`. The ROM came
back to `sys.boot_completed` 1, with `/init` at `e8e2768e…78ef` and `mmcblk0p31` at
`fe6e372e…6a84`.

## 2026-11-12 — the battery, the charger and the panel, for issue 27

Read three ways: over `adb root` from the ROM; from TWRP's root shell, whose ramdisk runs
the same kernel; and from a probe of ours as `/init` on `system`, swapped in as ADR 0006,
D2 does and logged to pstore. The phone was on the Mac's USB port, a charging downstream
port (`charger_type` 1), throughout.

**The battery.** `power_supply/Battery`, from the ROM:

```
brand=SCUDATL technology=Li-poly health=Good status=Full capacity=100
charge_full_design=3000 charge_full=2283 capacity_fcc=2283 batt_limit_fcc=2187 bat_qmax=2247
voltage_max=4400 voltage_now=4385482 current_now=127 temp=260 cycle_count=7
```

The gauge has learned 2283 of 3000 mAh, 76 %. `cycle_count` 7 on a phone this old is not
believable. Units: `capacity` in %, `current_now` in mA with a positive value flowing into
the battery, `voltage_now` in µV, `temp` in tenths of a °C. Nobody looked at the battery
for swelling.

**The kernel's charging limits** are the device tree's, read in TWRP under
`/proc/device-tree/SCUDATL_2900_39k_4400`: `fcc` 2900 mAh, `vbat_max` 4400 mV, and
`temp_para` as rows of °C from, °C to, input current, charge current, termination voltage
and hysteresis (−1 is the maximum):

```
< 0 °C     no charge            0–5 °C   290 mA  4400 mV    5–10 °C   870 mA  4400 mV
10–45 °C   max     4400 mV      45–50 °C 1015 mA 4100 mV    > 50 °C   no charge
```

`charging_core` has `iterm` 100 mA and `high_temp_limit` 1, and `coul_core` cuts off below
−20 °C and above 60 °C. `charging_core.c` applies the table, and lowers the termination
voltage as the battery ages, from the charger's monitor work, with no userspace involved.

**The charger's nodes**, `hw_power/charger/charge_data`, writable by root:

| Write | Reading |
|---|---|
| `enable_charger` 0 | `status` `Not charging`, `current_now` 0, the coulomb counter frozen at −786 for 40 s; the phone runs from USB, `Ibus` 16–95 mA with the ROM asleep |
| `enable_charger` 1 | `status` `Full`, `current_now` 123–130 within 5 s |
| `enable_hiz` 1 | USB input cut, `Ibus` 0, `Vbus` 5210 mV; the battery powers the phone, −39 mA with the ROM asleep |

At 100 % on the cable the battery takes 109–130 mA at 4.385 V, above `iterm`'s 100 mA,
so the charger never terminates. It holds the battery full. `huawei_charger.c`'s
`charge_start_charging`, which runs on every plug-in, clears the disable flag that
`enable_charger` 0 set, and it takes `charge_wakelock`. That wake lock is released only on
charge done at 100 % from a standard, non-standard, FCP or PD charger, never from the
Mac's port.

**The panel**, from the ROM with USB input cut and the ROM awake (`svc power stayon`),
averaged over 12 s each:

| State | `current_now` |
|---|---|
| ROM asleep, panel off | −38 mA |
| panel on, `lcd_backlight0` 0 | −130 mA |
| backlight 200 / 800 / 2000 | −141 / −157 / −185 mA |
| backlight 4000 / 8081 (maximum) | −236 / −351 mA |

The backlight costs about 27 mA per 1000 units over the panel's own 92 mA. The green
notification LED at 255 costs about 5 mA.

**TWRP**, USB input cut, panel timed out, averaged over 12 s each: −193 mA with every core
on `performance`, −176 mA on `interactive` or `powersave` (cpu0 at 480 MHz), −167 mA on
cpu0–3 alone, −166 mA on cpu0 alone. The three `cpuidle` states, WFI, cpu-sleep and
cluster-sleep, are all in use. DDR runs pinned at 933 MHz under `pm_qos`. The modem reads
`MODEM_STATE_OFF`.

**The probe as `/init`**, three runs, read from pstore in TWRP. The first,
`0e74662d66fd79631567389d6ccbbb9192ccd9326e8ace4eb00139e8754671fb`, 346728 bytes, sampled
for 306 s:

```
start        capacity=100 status=Full current_now=-1  voltage_now=4345491 temp=240 ibus=704 backlight=4000 modem=MODEM_STATE_OFF
charging     status=Full        current_now=126…116 temp=240…250 ibus=413…533   (8 samples, 105 s)
charger_off  status=Not_charging current_now=0  ibus=293…354   (enable_charger 0, 4 samples)
charger_on   status=Full        current_now=141, 136   (enable_charger 1)
hiz          current_now=-347…-366   backlight 0, 200, 800, 2000 and 8081 alike
restored     status=Full current_now=311 ibus=676
```

The interfaces in `/sys/class/net` are `lo`, `dummy0`, the tunnels and the modem's
`rmnet*`; there is no `wlan0`. **The kernel charges with our binary as PID 1.**
`enable_charger` stops and restarts charging as it does under the ROM. Writing
`lcd_backlight0` changes nothing measurable, and it reads back what was written.

The second probe, `184596263d6b3db95d313e22397f4f168e72aee14ab8f0e4708be4044d3cd1da`,
logged nothing after `modem=`. `balong_power/modem_state` ends in a NUL, and the kernel log
cut each record there. The third, `4ac4a9c18112ad153607d56ca77c44ebee0867d2f88629b1960b2c6ab64bb3ba`,
347448 bytes, strips control characters from what it logs. With USB input cut, every
write succeeded and was read back, and the draw did not move:

| Step, 5 samples at 4 s | `cpus` | `freq0` / `freq4` | `current_now` |
|---|---|---|---|
| defaults | 0-7 | 1709000 / 2362000 | −350 to −357 mA |
| `powersave` on cpu0 and cpu4 | 0-7 | 480000 / 1402000 | −347 to −351 mA |
| cpu1–7 offline | 0 | 480000 | −347 to −352 mA |
| `fb0/blank` 4, powerdown | 0 | 480000 | −347 to −353 mA |
| cpu1–7 back online | 0-7 | 480000 / 1402000 | −349 to −353 mA |

So under our `/init` the phone draws about 350 mA, about 180 mA more than TWRP with its
panel timed out. `lcd_backlight0` reads 4000, the bootloader's level, from the first sample
on. Whether the panel stayed lit was not looked at. Every probe rebooted into TWRP, and
the ROM's `/init` was renamed back, hashing to `e8e2768e…78ef`.

## 2026-11-12 — the charge stop under our `/init`, for issue 36

**The nodes' bytes**, read with `od -c` over `adb root` from the ROM on the Mac's port, are
the adapter's fixtures in `crates/battery/testdata/`: each value ends in one `\n`.
`capacity` `100`, `status` `Full`, `voltage_now` `4339811`, `temp` `250`, `current_now`
`0`, `health` `Good`, `enable_charger` `1`. With `enable_charger` 0, `status` read
`Not charging` and `current_now` `0`. With `enable_hiz` 1, USB input cut and the cable in,
`status` read `Full` and `current_now` `-34`: `status` says the cable is in, not that USB
powers the phone. `Discharging` was not read, since that needs the cable pulled by hand.

**The probe**, `crates/init-probe` at
`374dbfd22827e9e63cde2edbefe47387e7bca1a724b4594ecf0eb72104253293`, 355616 bytes, runs
`alarm_core::battery::stop_at_limit` with the default 80 % through `SysfsBattery`, writes
`enable_charger` 1 to stand in for a plug-in, runs the stop again, and writes 1 back. It
ran first from a `chroot` of a `tmpfs` in TWRP with 0 to 2 closed, then as `/init` on
`system`, swapped in and back as `docs/manual-steps/phone.md` says. From
`console-ramoops-0` after the boot as pid 1, at 100 % on the cable:

```
13.736  start          status=Full          current_now=0    ibus=548  enable_charger=1
13.738  charge stop: Stopped
18.779  stopped        status=Full          current_now=0    ibus=309  enable_charger=0
23.846  plugged_in     status=Full          current_now=137  ibus=444  enable_charger=1
23.855  charge stop: Stopped
28.914  stopped_again  status=Not_charging  current_now=0    ibus=317  enable_charger=0
33.964  restored       status=Full          current_now=137  ibus=497  enable_charger=1
```

**A charge stops under our `/init`**, and a reset `enable_charger` is stopped again on the
next pass. `status` follows the write late: 5 s after the first stop it still read
`Full`, with no current flowing; after the second it read `Not charging`. The TWRP run
read `Not charging` 5 s after both. A stop that is not yet visible in `status` is still
read back from `enable_charger`. The stop from below the limit, a charge climbing to 80 %,
was not run on the phone; the phone was full. The ROM's `/init` was renamed back, hashing
to `e8e2768e…78ef`, and the ROM reached `sys.boot_completed` 1.

## 2026-11-12 — a shell and a log without a console, for issue 15

Read over `adb root` from the ROM and from TWRP. Nothing was written to a partition but
`misc`'s command field, zeroed after `adb reboot recovery`.

**The rest of the config**, from `/proc/config.gz`, beyond issue 6's reading:

```
# CONFIG_NETCONSOLE is not set     # CONFIG_VT is not set     # CONFIG_LOGO is not set
CONFIG_USB_F_ACM=y  CONFIG_USB_U_SERIAL=y  CONFIG_USB_F_FS=y  # CONFIG_USB_CONFIGFS_SERIAL is not set
CONFIG_SERIAL_AMBA_PL011=y  CONFIG_SERIAL_AMBA_PL011_CONSOLE=y   # CONFIG_DEVMEM is not set
CONFIG_PSTORE_ZLIB_COMPRESS=y  CONFIG_MAGIC_SYSRQ=y  CONFIG_MAGIC_SYSRQ_DEFAULT_ENABLE=0x1
CONFIG_PANIC_TIMEOUT=0  # CONFIG_PANIC_ON_OOPS is not set  # CONFIG_KGDB is not set
CONFIG_HISI_BB=y  # CONFIG_HISI_DFX_CORE is not set
```

`FRAMEBUFFER_CONSOLE` is absent, since it needs `VT`. The host side has
`USB_NET_CDC_NCM`, `CDCETHER` and `RNDIS_HOST`, which do not make the phone a gadget.

**pstore**, `/sys/module/ramoops/parameters`: `mem_size` 1048576, `console_size` 524288,
`pmsg_size` 131072, `record_size` 131072, `dump_oops` 1, `ecc` 0. The device tree reserves
`pstore-mem` and Huawei's `bbox-mem`. `/dev/pmsg0` is `c-w--w--w- root log 253,0`. `dfx`
is `mmcblk0p41`, 32768 sectors, 16 MiB.

**A CDC-ACM function beside adb, in the ROM.** The ROM's gadget is
`/config/usb_gadget/g1`, `12d1:103a`, class 0, on UDC `hisi-usb-otg`, with only `ffs.adb`
in `configs/b.1`. A script run with `setsid nohup` made `functions/acm.dbg` (`port_num` 0,
`/dev/ttyGS0` at `244,0`), unbound the UDC, linked it as `f2`, and bound it again. The Mac
showed `/dev/cu.usbmodem101` and `/dev/tty.usbmodem101` within 12 s, with no driver
installed, and the phone as `P20 Lite`, `12d1:103a`. A Python script on the Mac, with the
port raw, wrote `mac-to-phone hello`, read for 8 s and wrote `mac-to-phone bye`:

```
b'mac-to-phone hello\r\np20-acm-test 12\r\np20-acm-test 13\r\np20-acm-test 14\r\np20-acm-test 15\r\n'
```

The phone's `cat /dev/ttyGS0` received both lines; the first came back as the tty's echo.
Writes 1 to 39 returned; nobody read the first 11 on the Mac.

**The restore lost USB.** By the script's restore the ROM had already removed the `f2`
link (`rm: … f2: No such file or directory`), and binding the UDC again left it empty. The
Mac saw no USB device at all, not after a replug, and not after the operator restarted
the phone with Power. *Fastboot & Rescue mode*, reached with the keys, enumerated; `fastboot reboot`
brought the ROM back on adb. In the ROM, Android's init owns the gadget.

**A panic after boot**, from the ROM: a `p20-alarm-os: panic test kmsg marker` line to
`/dev/kmsg`, a `p20-alarm-os: panic test pmsg marker` line to `/dev/pmsg0`, then
`echo c > /proc/sysrq-trigger` at 70 s. The ROM was back on adb 39 s later, with no
eRecovery, and its `/proc/cmdline` carried `reboot_reason=AP_S_PANIC
normal_reset_type=AP_S_PANIC enter_recovery=0`. `/sys/fs/pstore` held:

```
console-ramoops-0   202750 bytes   the boot before, to `Kernel panic - not syncing: Fatal exception`
dmesg-ramoops-0     218346 bytes   the oops dump, with the kmsg marker at 58.611755 s
pmsg-ramoops-0      131060 bytes   the ROM's logd records, ending in the pmsg marker
```

So a warm reboot after a panic keeps all three, and a panic once the boot has counted as
good does not start eRecovery. A panic in `/init` did (*our `/init` on `system`*).

**The ROM's shell without Android**, from TWRP: `system` mounted read-only, its `/system`
bound into a `tmpfs` root with `devtmpfs`, `proc` and `sysfs` beside it, and no property
area. `/init` on `system` still hashed to `e8e2768e…`.

```
chroot … /system/bin/sh -c 'echo sh ran as $(id); echo $0 $KSH_VERSION'
sh ran as uid=0(root) gid=0(root) …
/system/bin/sh @(#)MIRBSD KSH R56 2018/01/14 Android        exit 0
PATH=/system/bin: ls /sys/class/power_supply, cat /proc/version, dmesg | tail -2, uname -r   all answered
getprop ro.build.type                                         empty, exit 0
/system/bin/adbd --version                                    Android Debug Bridge Daemon version 1.0.40
```

`system` also holds `toybox` (473816 bytes), `linker64`, `gdbserver`, `gdbserver64` and
`strace`. Its root has `/config`, `/dev`, `/proc`, `/sys`, `/mnt`, `/acct` and `/storage`
to mount on. `misc` was cleared and the ROM reached `sys.boot_completed` 1.

## 2026-11-12 — the debug `/init`, for issue 9

**The binary**, `crates/debug-init` at
`64caf049c1d498dc86cb0a18224d45ec66a6569bfa9182da42a40bce66ecddaa`, 383312 bytes, swapped
onto `system` from TWRP as `docs/manual-steps/phone.md` says, with `init.rom` hashing to
`e8e2768e…78ef`. `misc`'s command field was zeroed first.

**A gadget with `acm` alone enumerates under our `/init`**, which ADR 0009, D1 assumed.
49 s after `adb reboot` from TWRP, with no driver installed, the Mac showed
`/dev/cu.usbmodemp20_alarm_os1`, the name built from the gadget's serial number, and
`ioreg` read `1d6b:0104`, `debug shell`, `p20-alarm-os`. A Python script on the Mac, with
the port raw, sent each command followed by an end marker:

```
uname -r                     4.9.319-Abdelhay-KSU-
id                           uid=0(root) gid=0(root) groups=0(root)
tty                          /dev/ttyGS0
ls -l /sys/fs/pstore         console-ramoops-0 69509 bytes, pmsg-ramoops-0 954 bytes
```

`console-ramoops-0` held the boot before, TWRP's, ending in `reboot: Restarting system
with command 'adb'`. The shell is `mksh` with the ROM's `mkshrc` prompt, `:/ #`, and
`/dev/ttyGS0` as its controlling terminal. It survived the Mac closing and reopening the
port between runs.

**A killed shell starts again, and `exit` reboots into TWRP.** `kill -9 $$` was answered
1 s later by a new shell; `exit` brought TWRP up 22 s later, with `reboot_reason=recovery
enter_recovery=1` and `misc`'s command field still zero. TWRP's `console-ramoops-0`:

```
13.708404  debug /init ran from system
13.708496  debug: mounted proc on /proc          … sysfs, pstore, configfs, tmpfs, each in 0.1 ms
13.712829  debug: gadget bound to hisi-usb-otg
13.719207  debug: /system/bin/sh started as 342
90.107696  debug: /system/bin/sh ended with signal: 9 (SIGKILL), starting it again
91.109130  debug: /system/bin/sh started as 360
129.767120 debug: /system/bin/sh ended with exit status: 0, rebooting into recovery
129.840545 reboot: Restarting system with command 'recovery'
```

`/sys/class/udc/hisi-usb-otg` was there at the first look, so the wait for it never ran.
The ROM's `/init` was renamed back, hashing to `e8e2768e…78ef`, and the ROM reached
`sys.boot_completed` 1.

## 2026-11-12 — the panel, for issue 10

**`fb0` is `hisifb0`, 1080×2280 XRGB8888 in three pages**, read from the debug shell
under our `/init`, where `devtmpfs` names it `/dev/fb0`. `/sys/class/graphics/fb0` reads
`modes` `U:1080x2280p-0`, `bits_per_pixel` 32, `stride` 4320, `pan` `0,0`.
`FBIOGET_VSCREENINFO` and `FBIOGET_FSCREENINFO`, recorded byte for byte in
`crates/framebuffer/testdata/`:

```
xres 1080  yres 2280  xres_virtual 1080  yres_virtual 6840  bits_per_pixel 32
red 16/8  green 8/8  blue 0/8  transp 0/0
type 0 (packed pixels)  visual 2 (truecolor)  line_length 4320  smem_len 29552640
```

**A draw reaches the panel only after a pan.** A probe filled page 1 with `ffff0000` over
its complement and ran `FBIOPAN_DISPLAY` with `yoffset` 2280; the panel showed red on top
and cyan below, so red is bits 16–23 and the pan is what puts a page on the panel.

**`open` lights the panel and the last `close` turns it off and frees the buffer.** The
kernel log shows `lcdkit_off` and `dpe_regulator_disable` on the last close. The backlight is `/sys/class/leds/lcd_backlight0/brightness`, 4000
after a reboot; `alarmd` writes 2000, logged as `hisifb_set_backlight: … bkl_lvl = 2000`.

**An `mmap` after the buffer was freed can take the kernel down.** Twice in one boot, a
process mapped `fb0` after another had closed it:

```
alarmd: Unable to handle kernel NULL pointer dereference at virtual address 0000000c
PC is at hisi_fb_mmap+0x1ec/0x360
LR is at fb_mmap+0x6c/0x150
Kernel panic - not syncing
```

and once mapped pages outside memory instead, `alarmd: Unhandled fault: level 3 address
size fault (0x92000043)` on its first write, with the next probe's `mmap` panicking as
above. Each panic rebooted into the debug `/init`. Thirty open, map, write, unmap, close
cycles in one process before that raised nothing, so the cycles are not proof of safety.
The first `mmap` of a boot never failed: `alarmd` maps `fb0` once and never closes it.

**The RTC read 1970.** `date` under the debug `/init` gave 1 January 1970, against the
issue's assumption. It was set from the Mac's UTC with `date -u` and `toybox hwclock -w -u
-f /dev/rtc0`, and read back 1794512278 at 19:37:58 UTC.

**`alarmd` draws the local time and moves it at the minute**, at
`d1ccabdeab41d69d6c0e126b591a70cc2e448502bc574080eca7aa1b49639065`, 350328 bytes.
Pushed to `/mnt` on a fresh boot and run as the first user of `fb0` at 19:48:46 UTC, the
panel showed `20:48` in white on black, then `20:49`; its log had `RUNNINGTEST set charge
enable = 0`, the charge stop at 80 %. Swapped onto `system` from TWRP in place of the debug
`/init`, with `init.rom` at `e8e2768e…78ef` and `misc`'s command field zeroed, and rebooted
at 19:53 UTC: the panel showed the local time once booted and changed at the next minute.
`alarmd` is `/init` on `system` now.

## 2026-11-12 — where the kernel comes from, for issue 46

Nothing was read off the phone; the backup was compared with a published image. The
kernel's build time, `Mon Apr 1 00:30:20 UTC 2024`, falls 29 s before release
`2024-04-01-1230` of `Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU` was published;
its asset is `Kernel-4.9.319-KSU-enforcing-01042024-1220.img`:

```
gh release download 2024-04-01-1230 -R Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU
12969984 bytes, sha256 a8d27ec65ab9d6596980a9aad00d8f22cd0fe70606221058d684f374b605f686
head -c 12969984 ~/p20-backups/kernel-mmcblk0p31-stock.img | shasum -a 256
a8d27ec65ab9d6596980a9aad00d8f22cd0fe70606221058d684f374b605f686
```

The same hash as issue 8's repacked image. The tag `2024-04-01-1230` is commit
`4a89ea72d610f5e3cd7d23d9d3b0919eba9fa8a2`. The release before it, `2024-04-01-1229`, was
published at 00:29:23, before the build time. Where the ROM and TWRP were downloaded from
was not found: a search for `0.23-p-20250919-UNOFFICIAL-anne` returned nothing.

## 2026-11-12 — touch and the side keys, for issue 11

**The input devices**, read from the debug shell under our `/init`, by
`/sys/class/input/eventN/device/name`:

| Node | Name | Reports |
|---|---|---|
| `event0` | `hisi_gpio_key` | `KEY_VOLUMEDOWN` 114, `KEY_VOLUMEUP` 115 |
| `event1` | `hisi_on` | `KEY_POWER` 116 |
| `event2` | `fingerprint` | — |
| `event3` | `HI6250_hi6555c_CARD Headset Jack` | — |
| `event4` | `huawei,ts_kit` | a Goodix GT1158, multi-touch protocol A |

A record is the 24-byte aarch64 `input_event`: `tv_sec` and `tv_usec` as 64-bit, then
`type` u16, `code` u16 and `value` i32, little-endian. A touch frame is
`ABS_MT_PRESSURE`, `ABS_MT_POSITION_X` (0–1080), `ABS_MT_POSITION_Y` (0–2280),
`ABS_MT_TRACKING_ID`, `SYN_MT_REPORT`, `BTN_TOUCH` 1 in the first frame only, and
`SYN_REPORT`; the release is `SYN_MT_REPORT`, `BTN_TOUCH` 0, `SYN_REPORT`. There is no
`ABS_MT_SLOT`. The keys send `EV_KEY` 1 and 0 with a `SYN_REPORT` each. Recorded in
`crates/input/testdata/`: four taps, at (509, 1100), (564, 1142), (645, 1209) and
(635, 1139), 5136 bytes; Volume Up then Volume Down twice, 288 bytes; Power, 96 bytes.

**Under our `/init`, `huawei,ts_kit` waits for its firmware.** The driver asks for
`ts/anne_gt1x_1158_auo.bin` through the kernel's user helper and waits 60 s in
`/sys/class/firmware/ts!anne_gt1x_1158_auo.bin/`; no `event4` exists until it is answered.
The file is on `odm` (`mmcblk0p44`, ext4, uevent `PARTNAME=odm`), under
`etc/firmware/ts/`. Mounted read-only and answered with `loading` 1, the file into `data`,
`loading` 0, `event4` appeared, and the driver asked next for
`ts/anne_gt1x_anne54030_mutto.img`, the same version as the controller's, so it flashed
nothing. `odm` is only ever mounted read-only.

**Touch works only while `fb0` is open.** Closing the panel logs `lcdkit_off`, which
suspends the GT1158; the taps above were recorded with a process holding `/dev/fb0`.

**`alarmd` sets, rings, snoozes and dismisses**, at
`e8d42ffeeab7825ce29ed53873e6fb9978ef1d06c74294d197c898752c2998ff`, 397688 bytes, swapped
onto `system` from TWRP in place of the debug `/init`, with `init.rom` at `e8e2768e…78ef`
and `misc`'s command field zeroed, and rebooted at 20:44:45 UTC with no cable attached. The
alarm, grey, was stepped by taps to two minutes ahead and armed by a tap off it; at that
minute the panel turned amber, and a volume key returned it to black. In a second boot at
20:51:53 UTC, the user saw it ring again after the snooze and Power dismiss it. Leaving
by the keys emptied pstore, and `dfx` held only an older boot, so neither run left a log.
`alarmd` is `/init` on `system` now.

## 2026-11-13 — suspend, wake and power-on from the RTC, for issue 12

Read under the debug `/init` (`64caf049…ddaa`), swapped in from TWRP in place of `alarmd`.
Each suspend ran unattended from a script in `/mnt`, started from the shell, which waited
for `/sys/class/power_supply/USB/online` to read 0 before suspending: the cable was out.
Every step went to `/dev/kmsg` and to `/mnt/r.log`; `debugfs` mounted on `/mnt/dbg` gave
`suspend_stats` and `wakeup_sources`.

**The RTC wakes a suspended phone on time, with no Android userspace.** Armed with
`echo +600 > /sys/class/rtc/rtc0/wakealarm`, then `echo mem > /sys/power/state`:

```
armed for 1794562576, 600 s
before rtc=1794561976 up=220.44 rm=1436 v=4023746 usb=0 ok=0 fail=0
after rc=0 rtc=1794562576 up=819.87 rm=1428 v=3946491 usb=0 ok=1 fail=0
wake up irq num: 78, irq name: RTC0
SR:AP WAKE IRQ(LPM3 NVIC): 10 (RTC0)
```

`rtc0` woke at the second it was armed for, and `suspend_stats` counted the success. The
first run, with the cable in, suspended too: `charge_wakelock` does not stop a direct
`echo mem`.

**The SoC never reaches deep sleep.** After every suspend, the LPM3's report:

```
SR:wakelock status, ap:1, modem:1, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
SR:system sleeped 0 times.
SR:sr times of sub system, ap:s-1, r-1, modem:s-0, r-0, hifi:s-0, r-0, iomcu:s-1, r-1.
```

The AP and the modem both hold a vote, and the modem never suspended. Which driver holds
each vote was not found.

**Suspended with the panel off, the phone draws about 53 mA.** The gauge, at suspend and
resume of the 600 s run above:

```
[hisi_coul_core]SUSPEND! cc=843899, time=43330, wakeup_avg_current:60
[hisi_coul_core]delta_time=615s, iavg_ua=53080
[hisi_coul_core]FCC=2283000uAh, … CC=852706uAh
```

8807 µAh in 615 s. A full charge of 2283 mAh lasts about 43 hours suspended, and the band
from 20 to 80 % about 26. Awake between suspends, with the panel off, the gauge's
`wakeup_avg_current` read 60 to 93 mA; a night awake under the debug `/init` went from
about 2180 to 1320 mAh in 11.3 hours, about 75 mA.

**With the panel lit, about 336 mA suspended.** The bootloader leaves the panel lit under
our `/init`. `: < /dev/fb0`, an open and close with no `mmap`, runs `lcdkit_off` and
darkens it, on a fresh boot after the touch driver's firmware wait has ended (about 75 s).

**Opening `/dev/fb0` after a suspend panicked the kernel**, an external abort in
`set_reg` under `hisifb_ctrl_fastboot`; the phone rebooted into the debug `/init`.

**The side button wakes a suspended phone**, as `GPIO23`, IRQ 139, the power key. So does
**a plug-in**, as `GPIO29`, IRQ 195, LPM3 NVIC 227, with `usb_wake_lock` active after.
With the panel lit, `GPIO28` (IRQ 194, a pl061 line of `gpiochip` at `e8a0e000`, with no
handler) woke the phone early three times — once 548 s into a 900 s suspend. With the
panel off it never did; no `gpio` property in `/sys/firmware/devicetree/base` names it.

**`wakealarm` does not switch on a powered-off phone.** Armed, then
`echo o > /proc/sysrq-trigger` — `toybox` has no `reboot`, and the ROM's needs Android's
`init` — the phone stayed off until the side button. `drivers/rtc/rtc-hisi-pmic.c` in
the kernel's source explains it: `hisi_rtc_shutdown` writes the PMU's alarm register only
when `poweroff_rtc_alarm` is enabled, and clears it otherwise.

**A `CLOCK_POWEROFF_ALARM` timer does.** `kernel/time/alarmtimer.c` arms the PMU through
`set_power_on_alarm` when a `timerfd` of clock 12 is set with `TFD_TIMER_ABSTIME`, 60 s
before the target, or 15 s after the call when the target is less than 60 s away; closing
the descriptor leaves it armed. A probe of 1216 bytes, `timerfd_create(12, 0)` then
`timerfd_settime` 90 s ahead, ran from `/mnt` 5 s after the cable came out, and SysRq `o`
powered the phone off. The user saw it go off, then come back on by itself; the next
boot's `/proc/cmdline` read:

```
normal_reset_type=BR_POWEROFF_ALARM
reboot_reason=AP_S_COLDBOOT
androidboot.mode=normal
```

A cold boot empties pstore, so the run's own log did not survive. The bootloader's stages
on that boot, from the same line: `bl1_time=1642 bl2_time=9112 logo_time=2559` (ms), and
the debug `/init` logged its start at 12.47 s on the kernel's clock. The draw while off,
and how long a press of the side button must last to switch the phone on, were not
measured.

**The shell exited with status 0 twice and the phone rebooted into TWRP**: once after a
replug, once 12 s after a boot by the side button. Each time its `/dev/ttyGS0` seemed to
read an end of file. A third time it exited with status 1 as the port went away, and took
a script exec'd from a child shell with it; a script exec'd by the top-level shell outlived
the port.

The debug `/init` is `/init` on `system` now, with `init.rom` at `e8e2768e…78ef`.

## 2026-11-13 — where the settings can live, for issue 30

Read from TWRP over `adb`, with `/dev/block/platform/hi_mci.0/by-name` and
`/sys/class/block/*/size`. Nothing was written but `reserved4`, and it was written back.

**The partitions**, by name, device and size in bytes; the type is `blkid`'s, `-` where
it names none:

| Name | Device | Bytes | Type |
|---|---|---|---|
| `vrl` | `mmcblk0p1` | 262144 | - |
| `vrl_backup` | `mmcblk0p2` | 262144 | - |
| `fw_lpm3` | `mmcblk0p3` | 262144 | - |
| `frp` | `mmcblk0p4` | 786432 | - |
| `fastboot` | `mmcblk0p5` | 4194304 | - |
| `modemnvm_factory` | `mmcblk0p6` | 4194304 | - |
| `nvme` | `mmcblk0p7` | 6291456 | - |
| `oeminfo` | `mmcblk0p8` | 67108864 | - |
| `reserved3` | `mmcblk0p9` | 4194304 | - |
| `modemnvm_backup` | `mmcblk0p10` | 4194304 | - |
| `modemnvm_img` | `mmcblk0p11` | 8388608 | ext4 |
| `modemnvm_system` | `mmcblk0p12` | 4194304 | - |
| `secure_storage` | `mmcblk0p13` | 33554432 | ext4 |
| `reserved4` | `mmcblk0p14` | 2097152 | - |
| `reserved5` | `mmcblk0p15` | 2097152 | - |
| `persist` | `mmcblk0p16` | 2097152 | - |
| `modem_secure` | `mmcblk0p17` | 14680064 | ext4 |
| `modem_om` | `mmcblk0p18` | 33554432 | ext4 |
| `splash2` | `mmcblk0p19` | 67108864 | ext4 |
| `misc` | `mmcblk0p20` | 2097152 | - |
| `modemnvm_update` | `mmcblk0p21` | 25165824 | - |
| `reserved2` | `mmcblk0p22` | 48234496 | - |
| `preload` | `mmcblk0p23` | 8388608 | ext4 |
| `teeos` | `mmcblk0p24` | 4194304 | - |
| `trustfirmware` | `mmcblk0p25` | 2097152 | - |
| `sensorhub` | `mmcblk0p26` | 16777216 | - |
| `fw_hifi` | `mmcblk0p27` | 12582912 | - |
| `erecovery_kernel` | `mmcblk0p28` | 25165824 | - |
| `erecovery_ramdisk` | `mmcblk0p29` | 33554432 | - |
| `erecovery_vendor` | `mmcblk0p30` | 16777216 | - |
| `kernel` | `mmcblk0p31` | 25165824 | - |
| `eng_system` | `mmcblk0p32` | 12582912 | - |
| `recovery_ramdisk` | `mmcblk0p33` | 33554432 | - |
| `recovery_vendor` | `mmcblk0p34` | 16777216 | - |
| `dts` | `mmcblk0p35` | 29360128 | - |
| `dto` | `mmcblk0p36` | 4194304 | - |
| `modem_fw` | `mmcblk0p37` | 100663296 | ext4 |
| `recovery_vbmeta` | `mmcblk0p38` | 1048576 | - |
| `erecovery_vbmeta` | `mmcblk0p39` | 1048576 | - |
| `eng_vendor` | `mmcblk0p40` | 12582912 | - |
| `dfx` | `mmcblk0p41` | 16777216 | - |
| `vbmeta` | `mmcblk0p42` | 4194304 | - |
| `cache` | `mmcblk0p43` | 134217728 | ext4 |
| `odm` | `mmcblk0p44` | 134217728 | ext4 |
| `hisitest0` | `mmcblk0p45` | 2097152 | - |
| `hisitest1` | `mmcblk0p46` | 2097152 | - |
| `hisitest2` | `mmcblk0p47` | 4194304 | - |
| `patch` | `mmcblk0p48` | 33554432 | ext4 |
| `bootfail_info` | `mmcblk0p49` | 2097152 | - |
| `rrecord` | `mmcblk0p50` | 16777216 | - |
| `reserved9` | `mmcblk0p51` | 14680064 | - |
| `system` | `mmcblk0p52` | 3909091328 | ext4 |
| `preas` | `mmcblk0p53` | 1182793728 | ext4 |
| `preavs` | `mmcblk0p54` | 33554432 | ext4 |
| `cust` | `mmcblk0p55` | 201326592 | ext4 |
| `version` | `mmcblk0p56` | 33554432 | ext4 |
| `vendor` | `mmcblk0p57` | 822083584 | ext4 |
| `product` | `mmcblk0p58` | 981467136 | ext4 |
| `userdata` | `mmcblk0p59` | — | ext4 |

TWRP's shell overflowed on the sizes of `system` and `userdata`; `system`'s is the one
read for issue 17.

**Eight partitions are zero end to end**, by `cmp <device> /dev/zero` reaching the
device's end first: `reserved2`, `reserved3`, `reserved4`, `reserved5`, `reserved9`,
`hisitest1`, `hisitest2` and `persist`. `hisitest0` starts
`[8/18 17:58:40]Ddr screen version info:`. `rrecord` differs from zero at byte 1025,
`bootfail_info` starts `XIFB`, and `dfx` holds the boot's log (issue 17).

**`userdata` is ext4 and not encrypted.** TWRP's `recovery.fstab` names it `f2fs` with
`encryptable=footer`, but `blkid` and `/proc/mounts` read ext4, mounted read-write on
`/data` with no password asked, and `/data/data` lists package names in clear. There is
no `/data/unencrypted`, and `ro.crypto.state` is empty.

**`cache`** is ext4, 121299 KiB with 47 used, mounted read-write by TWRP. It holds
`backup`, `backup_stage`, `lost+found` and `recovery`, whose `last_log.gz`, `log.gz`,
`recovery.fstab` and `storage.fstab` carry the time of the current boot into TWRP.

**A record on `reserved4` survives the bootloader.** `reserved4` and `reserved5` were
streamed to `~/p20-backups/reserved4-mmcblk0p14-stock.img` and
`reserved5-mmcblk0p15-stock.img`, 2097152 bytes each, both ends at `5647f05e…1eee`. A
4096-byte record (`c23972d0…5b63`) was written at offset 0 with `dd`, which made the
partition `a02c8e6d…bdde`. After `adb reboot recovery`, through the bootloader and back
into TWRP, the first 4096 bytes still read `c23972d0…5b63`, the partition `a02c8e6d…bdde`,
and `reserved5` `5647f05e…1eee`. The reboot left `boot-recovery` in `misc`; its command
field was zeroed. The backup was written back to `reserved4`, which reads
`5647f05e…1eee` again.

TWRP's `dd` refuses `conv=`: `dd: conv option disabled`.

**`reserved4`'s role**: unused by the ROM, the bootloader and TWRP; ours, for the
settings record of ADR 0011.

## 2026-11-15 — the panel off and on, for issue 37

Read under the debug `/init` (`64caf049…ddaa`), with `alarmd` built from `07ec897`
(`e3ade91c…190f`, 399016 bytes) on `system` and started from the shell 92 s into a
boot, after the touch driver's firmware wait. Draws are `Battery/current_now`, negative
for a discharge, with the cable in and
`/sys/class/hw_power/charger/charge_data/enable_hiz` at 1, so the battery carries the
whole phone; it went back to 0 after each reading.

**`FBIOBLANK` on the held `fb0` turns the panel off and on, and never panicked.**
`FB_BLANK_POWERDOWN` logs `hisi_fb_blank: fb0, blank_mode(4)`, runs `lcdkit_off`, sets
the backlight to 0, suspends `ts_kit` and sends the sensor hub `ST_SCREENOFF`;
`FB_BLANK_UNBLANK` logs `blank_mode(0)`, runs `lcdkit_on` and `mipi_init`, resumes
`ts_kit`, and `alarmd` writes the backlight to 2000. The panel shows the last frame
drawn. A press logs `[hisi_powerkey_handler] power key press interrupt!` and the unblank
follows within 2 ms.

**Awake with the panel off, the phone draws about 64 to 80 mA**, whatever the backlight
node holds: 1000, 2000 and 3000 read the same. Lit, with taps on the panel:

| Backlight | Draw, mean of 12 to 13 s |
|---|---|
| 1000 | 260 mA |
| 2000 | 276 mA |
| 3000 | 306 mA |
| 4000, the bootloader's, before `alarmd` | about 362 mA |

**Touch registers after a relight.** Taps kept the panel lit for 62 s in one run and
73 s in another, against the 10 s a press alone gives.

**A press after a suspend lights the held `fb0`, with no panic.** The cable out,
`wakealarm` armed at `+60` and `echo mem > /sys/power/state` two seconds after the panel
went off; the side button woke the phone before the RTC did:

```
[  242.431762] PM: suspend entry 2026-11-15 15:49:37.361302048 UTC
[  242.757659] [hisi_coul_core]SUSPEND! cc=-647734, time=4076, wakeup_avg_current:80
[  255.599731] wake up irq num: 139, irq name: GPIO23
[  255.683441] PM: suspend exit 2026-11-15 15:49:50.083469791 UTC
[  255.683685] [pid:505,cpu5,alarmd][hisifb I]:hisi_fb_blank: fb0, blank_mode(0) +!
[  265.888610] [pid:505,cpu7,alarmd][hisifb I]:hisi_fb_blank: fb0, blank_mode(4) +!
```

The user saw the time, and touch worked. Two presses 4 minutes later, awake and off the
cable, lit it again. A wake by the RTC with the panel then lit by a press was not read.

A pulled or knocked cable ends the debug `/init`'s shell with status 129, `SIGHUP`, and
the phone reboots into TWRP; `alarmd` started from that shell needs `trap "" HUP`.
`alarmd` under the debug `/init` writes no kernel log: its `devtmpfs` mount on `/dev`
fails with `EBUSY`.

The debug `/init` is `/init` on `system` now, with `alarmd` at `/alarmd` and the suspend
script at `/p20-run.sh` beside it.

## 2026-11-17 — suspended while the panel is off, woken by the RTC, for issue 50

Read under the debug `/init` (`64caf049…ddaa`), with `alarmd` built from `ea96be2`
(`6741d3d4…4256`, 404120 bytes) copied to `/mnt` over the serial shell and started from
it behind `trap "" HUP`, in a fresh boot. The clock was set to 05:55:00 UTC first, so the
07:00 alarm, at `+01:00`, was five minutes ahead; the user lit the panel and armed it.

**`alarmd` suspends each time the panel goes off, and the RTC wakes it at the armed
second.** Its own kernel log, read over serial after `kill -STOP` paused it during the
ring:

```
[  153.036285] [pid:379,cpu5,alarmd]PM: suspend entry 2026-11-17 05:55:10.969645998 UTC
[  234.666503] [pid:379,cpu0,alarmd]wake up irq num: 139, irq name: GPIO23
[  235.222808] [pid:379,cpu5,alarmd]PM: suspend exit 2026-11-17 05:56:32.556003125 UTC
[  235.223724] [pid:379,cpu5,alarmd][hisifb I]:hisi_fb_blank: fb0, blank_mode(0) +!
[  246.708190] [pid:379,cpu5,alarmd][hisifb I]:hisi_fb_blank: fb0, blank_mode(4) +!
[  247.000030] [pid:379,cpu5,alarmd]PM: suspend entry 2026-11-17 05:56:44.333005207 UTC
[  366.507995] [pid:379,cpu0,alarmd]wake up irq num: 78, irq name: RTC0
[  367.051177] [pid:379,cpu6,alarmd]PM: suspend exit 2026-11-17 05:58:44.542877604 UTC
[  368.052490] [pid:379,cpu5,alarmd]PM: suspend entry 2026-11-17 05:58:45.544189062 UTC
[  442.509155] [pid:379,cpu0,alarmd]wake up irq num: 78, irq name: RTC0
[  443.057067] [pid:379,cpu4,alarmd]PM: suspend exit 2026-11-17 06:00:00.547598438 UTC
[  444.058319] [pid:379,cpu5,alarmd][hisifb I]:hisi_fb_blank: fb0, blank_mode(0) +!
```

The press woke it from the first suspend; 10 s later the panel went off and the window
began. The RTC woke it at the window's end, 120 s on, and a second later `alarmd`
suspended again armed for 06:00:00, which it woke at; the panel lit for the ring a second
after that. Ringing in this build is the panel lit amber, with no vibration or sound.

**The cable was in for that run, and the phone suspended fully all the same** — user
space frozen, the non-boot CPUs off, `charger type: CDP` read at each resume. An earlier
run the same morning, with the cable out, woke and showed the alarm at 07:00 as the user
watched; its log was lost when the phone was powered off through eRecovery.

**`wakealarm` and `mem` from the shell behave as the adapters expect.** `echo 0`, then
the epoch `+20`, read back `1794905711`; `echo mem > /sys/power/state` returned 0 at
that second:

```
[  148.281280] PM: suspend entry 2026-11-17 08:54:51.296722896 UTC
[  167.504913] wake up irq num: 78, irq name: RTC0
[  168.084350] PM: suspend exit 2026-11-17 08:55:11.579127604 UTC
```

**The serial gadget comes back after a resume, but only while the phone stays awake.**
`alarmd` suspends again one to eleven seconds after a wake, before the Mac can open the
port, so a reading needs `alarmd` paused while it is awake — here, while it rang. After a
resume the shell's tty takes no end-of-file until `stty sane`. Writing `""` and then
`hisi-usb-otg` to `/config/usb_gadget/debug/UDC` after a resume left the gadget dead
until a restart. Opening and closing the port on the Mac hangs up the debug `/init`'s
shell, and the phone reboots into TWRP.

**Starting `alarmd` in a boot that had already suspended restarted the phone at once.**
That boot's log was overwritten before it was read. *Assumption:* the first open of `fb0`
after a resume panics the kernel; `alarmd` as `/init` opens `fb0` before any suspend.

Power and Volume Up with the cable in enter eRecovery; with it out, TWRP. Powering off
clears pstore. The phone's clock was set back from the Mac's UTC afterwards, and
`wakealarm` disarmed.

## 2026-11-18 — the serial shell from a host script, for issue 42

Read under the debug `/init` (`/init` hashing to `64caf049…ddaa`), 547 s into a boot, with
`scripts/serial_shell.py` from `feat/serial-shell` and one run per command:

```
uv run python scripts/serial_shell.py 'uname -r'    4.9.319-Abdelhay-KSU-, exit 0
uv run python scripts/serial_shell.py 'uname -r'    4.9.319-Abdelhay-KSU-, exit 0
uv run python scripts/serial_shell.py false         exit 1
```

**Each run opens and closes the port, and the shell outlives it.** `/proc/uptime` read
546.96 s before the runs and 566.08 s after them, with no reboot into TWRP between.

**A pushed file arrives whole.** `alarmd` built from `2c38b04`, 404320 bytes, sha256
`3acce6d3ac5d9b11276e9a59fe8ff9e1391cda887d74a629b130ca56ab6e6e2d`, went to `/mnt/alarmd`
in 7.9 s, and `sha256sum` on the phone matched the Mac's. A 404120-byte random file took
7.5 s. `base64` and `sha256sum` are the ROM's, in `/system/bin`. The copy was removed.

## 2026-11-18 — the battery symbol, for issue 49

`alarmd` built from `66d89bf`, 404784 bytes, sha256
`2248d64e834a33677aeb94bbae5a67e605f5c6a5a389d0d55eed72ff0e5f67c9`, was copied over
`/init` on `system` from TWRP. It replaced `64caf049…ddaa` and left `init.rom` at
`e8e2768e…78ef`. Read in TWRP just before the reboot, on the cable:

```
Wed Nov 18 11:38:37 GMT 2026
capacity 12
status   Charging
```

**The lit face showed what the host renders for 12 %**, as read by the operator: the
symbol in red with one bar, `12%` beside it in the top-right corner, and the time and
the alarm where they were. The capacity was not read again while `alarmd` ran, since no
`adb` reaches the phone then.

## 2026-11-18 — the off draw, a power-off on the cable, and the side button, for issue 53

Read under the debug `/init` built from `3c6e776` (`86080445…0bc7`, 383656 bytes), copied
over `alarmd` (`2248d64e…5f67c9`) from TWRP, with `init.rom` left at `e8e2768e…78ef`.
Every reading went through `scripts/serial_shell.py`; the phone's clock matched the Mac's
to the second. The phone powered off with `echo o > /proc/sysrq-trigger`.

**A power-off on the cable boots our `/init` again, in a charge mode.** Powered off at
11:57:11 UTC with the cable in, at 24 % and charging; the serial port came back at
11:58:26, and that boot's `/proc/cmdline` read:

```
androidboot.mode=charger
normal_reset_type=BR_POWEROFF_CHARGE
reboot_reason=AP_S_COLDBOOT
bl1_time=1663 bl2_time=9777 logo_time=3068
```

The debug `/init` started at 13.64 s on the kernel's clock and the battery kept charging
at about 616 mA, since nothing under it stops a charge. A plug-in to a phone that was off
booted it the same way at 20:08:50, with the same `androidboot.mode` and reset type. What
the panel showed between the power-off and the boot was not watched.

**The gauge restarts from a fresh open-circuit voltage only after a long rest.** After 8
hours off, the plug-in boot logged `using pmu ocv from fastboot`, `initial OCV = 3766759`
and `CC=1uAh`; a boot 3 minutes after the next power-off logged `using save ocv` with the
same voltage and `cc_adjust=-3260 uah`, the coulomb counter carried across the power-off.
`capacity` read 24 on every boot while `SOC real` read 27.5 %: `last_soc_enable` carries
the last value shown.

**The draw while off is far below a suspend's 53 mA, and was not measured.** The cable
came out at 12:08:42 and the phone powered off 5 s later, at 24 %, `capacity_rm` 589 and
`voltage_now` 3831971 after 66 s at rest with `enable_charger` 0. Eight hours later the
fresh voltage above gave `RC=627275uAh`, about 27.5 %. At 53 mA the phone would have lost
about 424 mAh, 19 %. The starting charge, estimated from a voltage read soon after a
charge, carries an error larger than what the 8 hours used.

**A hold of about 3 s on the side button switched the phone on**, as the operator timed
it; whether shorter holds were tried was not recorded. The boot read
`normal_reset_type=BR_PRESS_1S`, `androidboot.mode=normal`, `bl1_time=1654 bl2_time=9192
logo_time=2644`, the debug `/init` starting at 12.68 s on the kernel's clock, and
`hisi_powerkey` logging `long_presspowerkey_happen` at 11.76 s. The press itself was not
timed against the port.

`alarmd` (`2248d64e…5f67c9`) went back over `/init` from TWRP afterwards, at 26 % and
charging.

## 2026-11-18 — the alarm kept on `reserved4`, for issue 58

`alarmd` built from `63cdb5e`, 417048 bytes, sha256
`cdaa1d64587006bb43f87b3671eee16d3f0c20559702d765b10afea12edbb3ef`, was copied over
`/init` on `system` from TWRP. It replaced `2248d64e…5f67c9` and left `init.rom` at
`e8e2768e…78ef`. Before the boot, `reserved4` (`mmcblk0p14`, `PARTNAME=reserved4`,
2097152 bytes) hashed to its backup, `5647f05e…1eee`, all zeros, and `misc`'s command
field was clear.

The operator set the alarm by touch, let the panel go off, and reached TWRP with the keys.
`reserved4`'s first 8192 bytes, read there and checked on the host with `zlib.crc32`:

```
slot 0: v1 len 13 seq 0 crc 0xa1018a09 ok  offset 3600 window 120  21:51 days 0x7f armed 1
slot 1: v1 len 13 seq 1 crc 0x3a567525 ok  offset 3600 window 120  21:53 days 0x7f armed 1
```

**Two saves landed in the two slots, the second with the next sequence**, and every byte
from 8192 to the end of the partition was still zero. The newest slot holds 21:53, armed,
every day; the operator recalled setting 21:52. The key reset emptied pstore, so that
boot's `settings saved` lines were not read.

**The saved alarm came back armed and rang.** After `adb reboot` from TWRP into `alarmd`,
with nothing touched, the alarm rang at the time set, as the operator heard it. The minute
it rang was not read against a clock.

## 2026-11-18 — powered off until the alarm, for issue 52

`alarmd` built from `46886b0`, 422768 bytes, sha256
`3d555c020d77fc5113f3ff911978273d55a79243c34d994fc29f01e16c97e94f`, was copied over
`/init` on `system` from TWRP. It replaced `cdaa1d64…b3ef` and left `init.rom` at
`e8e2768e…78ef`; `misc`'s command field was zeroed before `adb reboot` at 21:21:41 UTC,
with the battery at 68 % and charging. The window was the saved 120 s; times below are
local, UTC+1, as the operator read them off the panel.

**On the cable, the window's end left the phone suspended, and the alarm rang.** The
operator set the alarm to 22:31 by touch and let the panel go off; it rang at 22:31. The
moment the panel went off was not read.

**Off the cable, the phone powered off and the power-on alarm switched it on two minutes
early.** The operator set the alarm to 22:39, pulled the cable and let the panel go off.
The phone booted at 22:37, which only a power-on from off does, and rang at 22:39. When it
powered off was not seen, since the panel was already dark, and whether the panel stayed
dark on that boot until the alarm was not reported.

The keys that reached TWRP afterwards emptied pstore, so neither boot's `booted by` line
nor the `powered off, on again at` line was read.

## 2026-11-19 — the debug shell across a lost line, for issue 55

The debug `/init` built from `014844e`, 383896 bytes, sha256
`997e8bb0c454e171c7dda8bec8992eb51d81b8b1a6da290c95a9881c1532e0bf`, was copied over
`/init` on `system` from TWRP. It replaced `alarmd` `3d555c02…e97f` and left `init.rom`
at `e8e2768e…78ef`; `misc`'s command field read zero before `adb reboot` at 10:49:22 UTC.
The port came back 16 s later. Times are the kernel log's clock; `/proc/uptime` read
about 4 s behind it.

**Every lost line started the shell again, and none rebooted into TWRP:**

```
 13.659637  debug: /system/bin/sh started as 340
183.099700  debug: /system/bin/sh ended with exit status: 0, starting it again
184.100921  debug: /system/bin/sh started as 349
367.143859  debug: /system/bin/sh ended with exit status: 0, starting it again
368.146606  debug: /system/bin/sh started as 352
481.992340  debug: /system/bin/sh ended with exit status: 0, starting it again
482.995697  debug: /system/bin/sh started as 358
```

The end at 367 s was the operator's replug. The end at 482 s was a pull,
`CHARGER_DISCONNECT` at 482.80 s and `CHARGER_CONNECT` at 632.94 s, and the shell ended
with status 0, not the 129 an earlier pull gave. What ended it at 183 s was not seen; the
operator touched nothing they recall, and the phone's port is sometimes loose. After
each, `scripts/serial_shell.py 'cat /proc/uptime'` answered with the uptime still rising.

**A shell started with the cable out waited for it.** Pid 358 started 0.2 s after the
pull, logged no end in the 150 s the cable was out, and answered after the plug-in.

**The marker still reaches TWRP.** `touch /mnt/recovery; exit` sent over the port, read
back from TWRP's pstore:

```
671.080169  debug: /system/bin/sh ended with exit status: 0, /mnt/recovery asks for recovery, rebooting into it
671.162322  reboot: Restarting system with command 'recovery'
```

`alarmd` (`3d555c02…e97f`) went back over `/init` from TWRP afterwards, with `misc`'s
command field reading zero.

## 2026-11-19 — the deep-sleep votes, for issue 54

**The ROM, with Android's userspace, reaches deep sleep.** `init.rom` (`e8e2768e…78ef`)
was put back over `/init` from TWRP, with `alarmd` (`3d555c02…e97f`) kept beside it as
`init.alarmd`. LineageOS booted at 11:30:26 UTC with no SIM, Wi-Fi and Bluetooth off, and
the modem registered for emergency calls only on MCC 262; the battery read 30 %. The
kernel log was kept with `cat /proc/kmsg` to a file under `/data/local/tmp`, which went
to the host's scratch space and is not in the repository. Times are the kernel log's
clock.

**With the radio on**, the cable out from 158.0 s to 498.9 s, Android suspended six
times and woke on its own `RTC0` alarms, then on the side button and the plug-in:

```
183.517395 SR:wakelock status, ap:1, modem:0, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
183.517608 SR:system sleeped 67 times.
183.517639 SR:sr times of sub system, ap:s-1, r-1, modem:s-449, r-448, hifi:s-2, r-2, iomcu:s-0, r-1.
189.510833 SR:wakelock status, ap:1, modem:1, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
498.858428 wake up irq num: 139, irq name: GPIO23
498.858459 wake up irq num: 195, irq name: GPIO29[]
498.858734 SR:system sleeped 759 times.
498.858734 SR:sr times of sub system, ap:s-6, r-6, modem:s-1146, r-1145, hifi:s-2, r-2, iomcu:s-0, r-1.
```

**With airplane mode on**, set by `settings put global airplane_mode_on 1` and its
broadcast, `mVoiceRegState` 3, the cable out from 656.6 s to 984.9 s, three suspends:

```
775.521057 SR:system sleeped 760 times.
775.521057 SR:sr times of sub system, ap:s-7, r-7, modem:s-1278, r-1277, hifi:s-2, r-2, iomcu:s-1, r-1.
836.521972 SR:system sleeped 761 times.
984.910400 SR:system sleeped 762 times.
984.910400 SR:sr times of sub system, ap:s-9, r-9, modem:s-1278, r-1277, hifi:s-2, r-2, iomcu:s-3, r-3.
```

Of nine reports, eight read `ap:1, modem:0` and one `ap:1, modem:1`. With the radio on,
`system sleeped` rose by 692 and the modem's own suspend count by 697: the SoC slept and
woke again with the modem's paging. In airplane mode each suspend added one sleep and the
modem none.

**The draw**, the gauge's `iavg_ua` over its `delta_time`, read at each resume:

| Radio | Suspend | Seconds | Average |
|---|---|---|---|
| on | 191.1 → 234.5 s | 44 | 9163 µA |
| on | 234.9 → 338.5 s | 104 | 9069 µA |
| on | 338.9 → 354.5 s | 16 | 19575 µA |
| on | 354.9 → 498.9 s | 144 | 7700 µA |
| off | 658.6 → 775.5 s | 119 | 6292 µA |
| off | 775.9 → 836.5 s | 61 | 6668 µA |
| off | 836.9 → 984.9 s | 148 | 4743 µA |

No suspend was longer than 148 s, and no step waited more than 10 minutes.

**What the ROM does to boot the modem.** `/vendor/etc/init/init.balong_modem.rc` mounts
`/vendor/etc/fstab.modem` — `modem_secure` on `/mnt/modem/modem_secure`, `modem_om` on
`/modem_log`, `modem_fw` on `/vendor/modem/modem_fw` read-only, `modemnvm_img` on
`/mnt/modem/mnvm2:0` — and writes 1 to
`/sys/devices/platform/his_modem/modem_sysboot_start` on `zygote-start`.
`/vendor/etc/init/charger/init.chip.charger.rc` does the same in charger mode, with no
`rild`. `teecd`, `/vendor/bin/teecd` as `u:r:tee:s0`, started at 17.09 s, with
`secure_storage` on `/sec_storage`; the write came at 17.64 s.

**Under the debug `/init`** (`997e8bb0…e0bf`), nothing writes `modem_sysboot_start`. The
kernel's thread waits for it, and `balong_power` reads off:

```
13.150299 [pid:283,cpu5,modem_sysboot_s]modem_sysboot_start_init start
$ cat /sys/devices/platform/balong_power/state
MODEM_STATE_OFF
```

`/vendor/modem`, `/mnt/modem` and `/dev/block/by-name` do not exist there.

**`GPIO28` has no owner in the live device tree.** It is line 4 of `gpio3`, phandle
`0x268`, whose `gpio-ranges` is `<&pmx0 0 21 8>`, so pin 25 of `pmx0`. No property under
`/sys/firmware/devicetree/base` holds `0x268` followed by 4, and none named `*gpio*` holds
28. `/proc/interrupts` lists no handler for it. It woke none of the ROM's eleven suspends,
all with the panel off.

**The modem's partitions** were streamed from TWRP to `~/p20-backups/`, each
`<name>-mmcblk0p<n>-2026-11-19.img`, with the phone's and the host's sha256 equal. Nothing
was written to them but by the ROM's own kernel:

| Name | Device | sha256 |
|---|---|---|
| `modemnvm_factory` | `mmcblk0p6` | `11123c37…892d` |
| `modemnvm_backup` | `mmcblk0p10` | `df715361…1a91` |
| `modemnvm_img` | `mmcblk0p11` | `3d0d3bc4…ea76` |
| `modemnvm_system` | `mmcblk0p12` | `bdd1c3eb…1522` |
| `secure_storage` | `mmcblk0p13` | `7e0a1321…e8a2` |
| `modem_secure` | `mmcblk0p17` | `2df9ce8f…acf9` |
| `modem_om` | `mmcblk0p18` | `d3fee5c2…944d` |
| `modemnvm_update` | `mmcblk0p21` | `a902ebf2…be1c` |

**The roles**, from the ROM's `fstab.modem` and the kernel's
`drivers/hisi/modem/drv/nvim/`: `modemnvm_img` holds the modem's working NV,
`/mnt/modem/mnvm2:0/modem_nv/nv.bin`; `modemnvm_backup`, `modemnvm_factory`,
`modemnvm_system` and `modemnvm_update` are its raw copies, written by the kernel through
`bsp_blk`; `modem_secure` and `modem_fw` hold the modem's images; `modem_om` its logs;
`secure_storage` the secure OS's store, through `teecd`.

**A ROM `init` copied with `cp` from TWRP did not boot.** The copy was labelled
`u:object_r:rootfs:s0`, and the phone came up in fastboot twice. `rm` of the copy and `mv`
of `init.rom` to `init` kept `u:object_r:init_exec:s0`, and the ROM booted.

TWRP's `dd` refuses `bs=1M`: `dd: block size '1M': illegal number`.

`alarmd` (`3d555c02…e97f`) went back over `/init` from TWRP afterwards, with `init.rom` at
`e8e2768e…78ef` and `misc`'s command field reading zero.

## 2026-11-19 — the modem booted from the debug `/init` with its radio off, for issue 70

Read under the debug `/init` (`997e8bb0…e0bf`), swapped in from TWRP over `alarmd`
(`3d555c02…e97f`), which was kept beside it as `/init.alarmd`; `init.rom` stayed at
`e8e2768e…78ef`. Every step was typed into the serial shell; the battery read 85 %. Times
are the kernel log's clock. Both kernel logs went to the host's scratch space and are not
in the repository.

**The mounts**, with the ROM's `fstab.modem` and `fstab.hi6250` options less `context=`,
since no SELinux policy is loaded: `vendor` (`mmcblk0p57`) read-only on `/vendor`, then
`modem_secure`, `modem_om`, `modem_fw` read-only, `modemnvm_img` and `secure_storage`, each
answering 0. `/dev/block` does not exist under the debug `/init`; the devices were mounted
as `/dev/mmcblk0p<n>`, and `/dev/block/by-name` was made from each partition's `PARTNAME`
in `uevent`. `modem_fw` holds `sec_balong_modem.bin`, `sec_lphy.bin`, `sec_modem_dt.img`,
`sec_tas.bin`, `sec_was.bin` and `sec_xphy_mcore.bin`.

**`teecd`** started as `/vendor/bin/teecd` from `/`, with a `tmpfs` on `/data` holding the
`sec_storage_data` directories `init.tee.rc` makes, and no property service. It stayed up
and logged only `TC_NS_sync_sys_time: tee adjust time failed, return error ffff0000`.

**The boot.** The write came at 119.46 s:

```
119.459594 modem_sysboot_start write ok
120.268402 [sec]: <load_image> load image /vendor/modem/modem_fw/sec_balong_modem.bin to secos success
120.629516 [sec]: <load_image> verify image /vendor/modem/modem_fw/sec_balong_modem.bin success
120.630554 send ipc to unreset ccore succeed
121.675689 [sec]: <load_image> verify image /vendor/modem/modem_fw/sec_was.bin success
123.696533 Balong_power mdrv_set_modem_state:101 set state 1 ,time slice 123696
123.697692 [TAF_MMA_ProcStartCnfSucc_PowerInit_PhoneMode] mdrv_sysboot_get_bootmode = 1, ulAutoSwitchOnFlg = 0
```

`balong_power`'s `state` read `MODEM_STATE_READY` within 6 s of the write, and still did
at the end, 1849 s into the boot.

**The radio**, over `/dev/appvcom`, opened read-write from the shell with each command
ended by a carriage return. The first `AT` drained the modem's queued reports, among them
`^SIMST:255,0`, `^SRVST: 0` and `^SRCHEDPLMNINFO:0,1,26203,51157`, then answered `OK`.
`AT+CFUN?` read `+CFUN: 0` before any command; `AT+CFUN=0` answered `OK`, `AT+CFUN?`
read `+CFUN: 0` again, and `AT^SYSINFOEX` read
`^SYSINFOEX:0,0,2,255,,0,"NO SERVICE",0,"NO SERVICE"`. With no `rild`, the modem comes up
with its radio off.

**The suspend**, from a script in `/mnt` that turned the panel off with `: < /dev/fb0`,
waited for `USB/online` to read 0, and armed `wakealarm` for +600 s. The cable came out at
1233.6 s:

```
1250.158477 [hisi_coul_core]SUSPEND! cc=-1943065, time=11093, wakeup_avg_current:49
1849.537536 wake up irq num: 78, irq name: RTC0
1849.537567 SR:wakelock status, ap:1, modem:0, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
1849.537597 SR:system sleeped 1 times.
1849.537597 SR:wake times, system:1, woken up by ap:1, modem:0, hifi:0, iomcu:0, lpm3:0.
1849.537628 SR:sr times of sub system, ap:s-1, r-1, modem:s-9, r-8, hifi:s-0, r-0, iomcu:s-1, r-1.
1849.537658 SR:AP WAKE IRQ(LPM3 NVIC): 10 (RTC0)
1849.546997 [hisi_coul_core]delta_time=604s, iavg_ua=2920
```

**The draw, 2920 µA over 604 s**, against the ROM's 4743 to 6668 µA with its radio off and
the 53 mA of ADR 0010, D5. The battery read 88 % before and after; awake, off the cable and
with the panel off, the gauge read 46 to 51 mA.

**The first try did not suspend.** The script, started with `exec` from the serial shell,
logged only its start: the plug-out hung up `/dev/ttyGS0`, and `SIGHUP` ended it, as
issue 50's `alarmd` had needed `trap "" HUP`. With the trap as its first line, the same
script ran to the end.

The debug `/init` stays on `system`, with `alarmd` at `/init.alarmd` and `init.rom` beside
it.

## 2026-11-19 — `alarmd` boots the modem with its radio off, and suspends at 4.8 mA, for issue 71

Read under the debug `/init` (`997e8bb0…e0bf`), with `init.rom` at `e8e2768e…78ef` and
`alarmd` at `/init.alarmd` left as they were. Each build of `alarmd` was pushed to `/mnt`
and started by a script there once `USB/online` read 0, with `trap "" HUP` and
`/proc/sys/kernel/printk` at 8; the script stopped `alarmd` with `SIGSTOP` after its first
suspend and kept the kernel log in `/mnt`. Times are the kernel log's clock. The kernel
logs went to the host's scratch space and are not in the repository.

**The first builds booted the modem and logged `modem booted`, but not the radio.** Every
`AT+CFUN=0` failed with `Operation not permitted`, for 60 s of tries. A probe opening
`/dev/appvcom` four ways read the modem's queued reports and `+CFUN: 0` when opened
blocking, with or without `O_NOCTTY`; with `O_NONBLOCK` every read answered `EPERM`.
`appvcom` is now opened blocking and read on a thread of its own.

**Every suspend was refused**, 0.3 s and 65 s after the boot:

```
icc: icc_channel_has_data NV[channel-2] stop suspend flow
PM: suspend exit
```

With the modem booted by hand and the cable in, a suspend tried every 20 s was refused the
same way 13 times over 4 minutes. The modem's boot had logged why:

```
[nv]:<modem_nv_delay 1154> modem nv wait for nv block device 0 s
fail to open file /dev/block/by-name/modemnvm_backup, ret -2!
…
[nv]:<modem_nv_delay 1160> get block device modemnvm_backup fail
fail to open file /dev/block/by-name/modemnvm_system, ret -2!
```

The kernel's NV driver finds a partition through `flash_find_ptn`, as
`/dev/block/by-name/<name>`, which `ueventd` makes on the ROM and nothing made under our
`/init`; issue 70 had made it by hand. With a link there to every partition, the same boot
read `modemnvm_backup block size is 0x400000.` and `reload success!`, and `balong_power`
read `MODEM_STATE_READY` 8 s after the write instead of 15 s.

**One build powered off instead.** With the radio fixed but no links, the operator pulled
the cable, saw the time, and plugged it back in some minutes later; the phone booted to
the boot screen and stayed there with nothing on USB, through a forced restart and a
second one after a replug. Two restarts emptied `pstore`, so the reading is inferred: the
refused suspends ran out the two-minute window, `alarmd` powered off with no alarm set,
and the plug-in booted it.

**One hand suspend on the cable reset the phone.** With the links made and the modem
booted by hand, one of the suspends tried every 20 s on the cable came back as a reboot
with `reboot_reason=LPM3_S_EXCEPTION`. `printk` was at its default, and `pstore` holds
nothing past 75.8 s. It did not come again in the three suspends below.

**The suspend under `alarmd`** (`d264c48c…e77e`), off the cable:

```
342.921447 modem_sysboot_start write ok
343.212646 [nv]:<bsp_nvm_core_init 1000> reload success!
347.467132 p20-alarm-os: modem booted
347.467132 p20-alarm-os: modem radio off
352.224700 PM: suspend entry 2026-11-19 16:05:19.268162969 UTC
352.478118 [hisi_coul_core]SUSPEND! cc=993, time=148, wakeup_avg_current:179
471.516296 wake up irq num: 78, irq name: RTC0
471.516326 SR:wakelock status, ap:1, modem:0, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
471.516357 SR:system sleeped 1 times.
471.516387 SR:wake times, system:1, woken up by ap:1, modem:0, hifi:0, iomcu:0, lpm3:0.
471.524658 [hisi_HI6555V100_coul]ccout_reg=0x13a83,ccin_reg = 0x1, cl_out_time=267, cl_in_time=0, cc_adjust=1152 uah
471.525573 [hisi_coul_core]delta_time=130s, iavg_ua=29963
```

**The draw asleep, 4.8 mA**: the coulometer's count went from 993 µAh at the suspend to
1152 µAh at the wake, 159 µAh over 119 s. `alarmd`'s window ends a suspend after two
minutes, so the gauge's `iavg_ua`, 29963 over 130 s, takes in the 11 s before it, awake
with the panel lit and the modem booting: 70 to 993 µAh, about 300 mA.

**On the cable**, three suspends of 15 s each by hand, with `alarmd` stopped and its modem
up, each read `modem:0`, woke on `RTC0` and counted in `system sleeped`.

**The gauge's own average, 4.6 mA.** In the same boot, with `alarmd` stopped and the modem
it booted still `MODEM_STATE_READY`, the operator pulled the cable; a script suspended for
5 s and at once for 300 s, so the gauge's average covers almost nothing awake:

```
1519.658538 PM: suspend entry 2026-11-19 16:24:47.125841146 UTC
1520.004241 [hisi_coul_core]SUSPEND! cc=104, time=401, wakeup_avg_current:85
1819.537078 wake up irq num: 78, irq name: RTC0
1819.537078 SR:wakelock status, ap:1, modem:0, hifi:0, iomcu:0, hisee:0, hotplug:0,0.
1819.545867 [hisi_HI6555V100_coul]ccout_reg=0x8c5a,ccin_reg = 0x8c2, cl_out_time=509, cl_in_time=192, cc_adjust=482 uah
1819.546417 [hisi_coul_core]delta_time=300s, iavg_ua=4560
```

The count agrees: 104 to 482 µAh over 300 s is 4.5 mA.

The debug `/init` stays on `system`, with `alarmd` at `/init.alarmd` and `init.rom` beside
it; the `alarmd` there is still `3d555c02…e97f`, without the modem.

## 2026-11-20 — the off draw, 1.2 mA, for issue 53

Read under the debug `/init` `997e8bb0…e0bf`, left on `system` from issue 55 with `alarmd`
`3d555c02…e97f` at `/init.alarmd` and `init.rom` at `e8e2768e…78ef`. It was built from
`014844e`; a build from `b3d7677` on `main` is the same 383896 bytes and hashes
`7a27e4dc…e7f8`, the two trees differing only in `Cargo.lock` entries of other crates.
Every reading went through `scripts/serial_shell.py`. Each power-off was a script in
`/mnt` that cleared `rtc0/wakealarm`, waited for `USB/online` to read 0, slept 5 s and
wrote `o` to `/proc/sysrq-trigger`, with `trap "" HUP` as its first line; its time is
when the serial port left the Mac, plus 5 s. Times are UTC on the Mac's clock; the
phone's read 4 to 5 s behind it.

**Start, 2026-11-19.** After a boot by `LPM3_S_EXCEPTION`, on the cable at 25.0 °C:
`status` `Full`, `capacity` 100, `voltage_now` 4347764, `capacity_rm` 2282,
`charge_full` 2283, `charge_full_design` 3000, `cycle_count` 7. Powered off at 16:49:05.

**`abs_cc` counts in mAh**, with the kernel log's sign: it read −2285 while `CC=` read
−2282347 µAh, the charge that had gone in.

**Both ends were fresh-OCV boots**, by a plug-in to a phone that was off, each read
`androidboot.mode=charger`, `normal_reset_type=BR_POWEROFF_CHARGE` and
`reboot_reason=AP_S_COLDBOOT`. `enable_charger` 0 was written as the first command, at `/proc/uptime`
15.9 s and 11.7 s, and `abs_cc` read 0 then.

| | 2026-11-20, after 17 h 20 min off | after 8 h 31 min more off |
|---|---|---|
| plug-in | about 10:08:37 | about 18:41:21 |
| battery | 21.0 °C | 22.0 °C |
| `initial OCV` | 4312772 µV, `OCV_temp=220` | 4307773 µV, `OCV_temp=230` |
| `SOC real` | 971 | 966 |
| `FCC=` | 2280000 µAh | 2281000 µAh |
| `RC=` | 2213880 µAh | 2203446 µAh |
| `CC=` | 0 µAh | 0 µAh |

Both logged `using pmu ocv from fastboot.`, `fcc_flag= 1, ocv_level:0`. Between the
two, the phone was powered off at 10:10:19. A plug-in's time is the kernel log's first
wall-clock stamp, less its kernel time and the command line's `bl1_time`, `bl2_time` and
`logo_time`, about 14.5 s.

**The draw while off is about 1.2 mA**: 10.43 mAh over the 30662 s from 10:10:19 to
18:41:21. The charge taken before `enable_charger` 0 was `abs_cc` 0, under the counter's
1 mAh step, and the 5 s on the battery before the power-off cost well under 1 mAh. The
two OCVs are 5.0 mV apart for those 10.43 mAh, so an error of 1 mV in either moves the
figure by about 0.25 mA. At 1.2 mA the band from 80 to 20 % of the 2280 mAh read would
last about 47 days with the phone off.

**The full charge was not a good start.** From `capacity_rm` 2282 at `Full` to the first
fresh `RC=` is 68 mAh over 17 h 20 min, 3.9 mA, three times the figure between two fresh
OCVs.

The debug shell's `touch /mnt/recovery; exit` started TWRP, but `adb` saw nothing until
the operator plugged the cable in again, about 4 minutes later. From TWRP, `alarmd` built
from `b3d7677`, 507376 bytes, sha256
`d264c48c798a44799ddfd4c113c801ae18b294a20b32766e0529c777b320e77e`, was copied over
`/init` on `system`, replacing the debug `/init`; `init.rom` still hashed `e8e2768e…78ef`,
`init.alarmd` was left at `3d555c02…e97f`, and `misc`'s command field read zero before
`adb reboot` at 18:47:34.

## 2026-11-20 — the notification LED, for issue 64

Read from TWRP and then under the debug `/init` `7a27e4dc…e7f8`, swapped in from TWRP over
`alarmd` `d264c48c…e77e`; `init.rom` stayed at `e8e2768e…78ef` and `init.alarmd` at
`3d555c02…e97f`. The kernel source read is `Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU`
at `4a89ea72`, `drivers/leds/hisi_leds.c`. Every LED was off when no step lit it. Times
are the kernel log's clock; the kernel logs went to the host's scratch space and are not
in the repository.

**The nodes** are `/sys/class/leds/red`, `green` and `blue`, one `hisilicon,hisi-pmic-led`
driver on the PMU at `fff34000`. `brightness` takes 0 to 255 and has three steps: 0 turns
the colour off, 255 sets the current to the device tree's `each_max_iset` (red 7, green
1, blue 7), and anything else to 1. `led_alwayson` acts only in factory mode, and
`led_config` has no colour flag set in the device tree.

**The PMU blinks on its own.** The three colours share one timing register, its high
nibble the on code and its low nibble the off code, each from `hisi_led_get_time`: 0 for
0 ms, then 1 to 9 for up to 500, 1000, 2000, 4000, 6000, 8000, 12000, 14000 ms and over.
Every write to `brightness` sets it back to steady on. Writing `trigger` `timer`, then
`delay_on` 500, then `delay_off` 2000, logged

```
[hisi_led_set_blink] id is 0, delay-on:500, delay-off:0
[hisi_led_set_blink] id is 0, delay-on:0, delay-off:2000
```

so only the last delay written survives. With on code 0 the operator saw red flash; with
off code 9 and the current at 1, each flash lasted about half a second, more than 20 s
apart. The current at 1, steady, was clearly visible in room light.

**What it costs awake**, from TWRP with `enable_hiz` 1 and the panel timed out: 15 reads
of `current_now` a second apart per step, each lit step against the unlit one before it,
three rounds, about −200 mA unlit and a spread of ±2 mA:

| Step | Over unlit |
|---|---|
| red 255 / 128 | 5.3 / 0.3 mA |
| green 255 / 128, both current 1 | 1.3 / 3.0 mA |
| blue 255 / 128 | 5.0 / 1.1 mA |

**What it costs asleep.** The modem was booted as in `docs/manual-steps/modem.md` and
answered `+CFUN: 0`, and the panel was turned off with `: < /dev/fb0` past 95 s. A script
in `/mnt` then wrote `enable_hiz` 1 and ran eleven suspends of 240 s, set an LED before
each and read the coulometer from the count at `SUSPEND!` to the one at the wake. The
cable stayed in; `cl_in_time` did not move. The count reset itself at the eighth wake,
after its reading.

| | Step | Count | Draw |
|---|---|---|---|
| 1 | off | 1450 → 1635 µAh | 2.79 mA |
| 2 | red 255 | 1658 → 2235 µAh | 8.69 mA |
| 3 | off | 2258 → 2569 µAh | 4.68 mA |
| 4 | green 255 | 2590 → 2994 µAh | 6.09 mA |
| 5 | off | 3015 → 3312 µAh | 4.47 mA |
| 6 | red 1 | 3335 → 3702 µAh | 5.53 mA |
| 7 | off | 3726 → 3992 µAh | 4.01 mA |
| 8 | red 255, on code 0, off code 3 | 4012 → 4347 µAh | 5.05 mA |
| 9 | off | 23 → 291 µAh | 4.04 mA |
| 10 | red 1, on code 0, off code 9 | 23 → 351 µAh | 4.94 mA |
| 11 | off | 22 → 394 µAh | 5.63 mA |

The six unlit suspends read 2.8 to 5.6 mA, 4.3 on average. Against the two either side,
red at 255 costs 4.9 mA, as awake; green at 255 1.5 mA, red at 1 1.3 mA, red at 255
blinking 1.0 mA, and red at 1 blinking every 20 s 0.1 mA. Anything under about 1.5 mA is
inside the unlit suspends' spread. Red at 1 with off code 3 was not read; at the blink's
duty from step 8 it would cost about 0.3 mA.

**It stays lit and holds no vote.** Every suspend woke on `RTC0`, read
`ap:1, modem:0, hifi:0, iomcu:0, hisee:0` and added one to `system sleeped`; the coulometer
counted the lit LED's current through each. The driver's `suspend` and `resume` only take
and release its mutex.

**A power-off turns it off.** From TWRP, red at 255 steady, a script waited for
`USB/online` to read 0, slept 5 s and wrote `o` to `/proc/sysrq-trigger`. The operator saw
the LED go dark 2 to 10 s after the panel. `hisi_led_shutdown` disables all three colours,
and `alarmd`'s power-off reaches it through `device_shutdown`.

From TWRP, `alarmd` `d264c48c…e77e` was copied back over `/init` on `system`;
`init.rom` still hashed `e8e2768e…78ef`, `init.alarmd` was left at `3d555c02…e97f`, and
`misc`'s command field read zero before `adb reboot` at 20:35:39 UTC.

## 2026-11-20 — the vibrator, for issue 13

Read from TWRP and then under the debug `/init` `7a27e4dc…e7f8`, swapped in from TWRP over
`alarmd` `d264c48c…e77e`; `init.rom` stayed at `e8e2768e…78ef` and `init.alarmd` at
`3d555c02…e97f`. The kernel source read is
`Abdelhay-Ali/android_kernel_huawei_hi6250_KernelSU` at `4a89ea72`:
`drivers/input/misc/hisi_vibrator.c` and `Abdelhay_defconfig`, which sets
`CONFIG_HISI_VIBRATOR=y`, `CONFIG_LEDS_TRIGGER_TRANSIENT=y` and leaves
`CONFIG_HISI_PMIC_VIBRATOR` unset. Times in the kernel log lines are its clock.

**The node** is `/sys/class/leds/vibrator`, linked to `hisi_vibrator@fff34000`, an LED
class device whose `brightness_set` writes the PMU's register `0x31` on any non-zero value
and clears it on 0; `max_brightness` reads 255 and `vibrator_min_timeout` 30. Under TWRP
and under our `/init` alike, `trigger` offers `timer` and `transient` and has
`[transient]` selected before anything writes it, and `activate`, `duration`, `state` and
`brightness` read `0`. The `trigger` and `activate` reads under our `/init` are
`crates/vibrator/testdata/`.

**A pulse runs on the kernel's timer.** Writing `transient` to `trigger` again set
`duration` back to 0. Then `1000` to `duration`, `1` to `state` and `1` to `activate` read
`activate` 1 and `brightness` 255 at once, and both 0 after 1.5 s:

```
[   42.452545] [pid:342,cpu7,sh]vibrator hisi_vibrator@fff34000: hisi_vibrator open
[   43.444366] [pid:0,cpu7,swapper/7]vibrator hisi_vibrator@fff34000: hisi_vibrator close
```

**`activate` 0 stops it at once.** A `duration` of 5000, stopped 0.5 s in, closed at
`53.143096` after opening at `52.631530`, and nothing followed in the next 5 s.

**`alarmd` woke from a suspend and vibrated.** Built from `7b90fe4`, 511232 bytes, sha256
`d4ddf76db5b7ae5e4e70528d2bb7ac7e0f171bed607994f58437114ab3786e7a`, it was copied over
`/init` on `system` from TWRP in place of the debug `/init`; `init.rom` still hashed
`e8e2768e…78ef` and `misc`'s command field read zero. `reserved4`'s newer slot, sequence
5, held one alarm at 19:59 every day, armed, at UTC+1. The RTC was set to 18:56:00 UTC at
21:13:50 by the Mac, on the cable. As the operator watched, the panel went dark, then lit
amber at the alarm and vibrated in 1 s pulses; Volume Down stopped the vibration at once.
A later press of Power dismissed the snooze, as `ringer` does, and nothing rang again.

**A snoozed alarm vibrates again.** The RTC was set to 18:58:15 UTC at 21:41:39 by the
Mac, from TWRP, and the phone rebooted. Volume Down snoozed the ring; it rang and vibrated
again when the snooze ended, about 21:51:25 by the Mac, and Power stopped the vibration at
once.

From TWRP, the RTC was set back to the Mac's UTC at 21:55:23, `wakealarm` disarmed, and
the phone rebooted into `alarmd` `d4ddf76d…6e7a` at 21:55:30.

## 2026-11-21 — the navigation bar, for issue 79

`alarmd` was built from `b87d3ad`, 511984 bytes, sha256
`bcd39a5ee9f84bd730125b3986219b4d212add613ed2ce555d333bfade60bf6d`. From TWRP it was copied
over `/init` on `system`, replacing `alarmd` `d4ddf76d…6e7a`. `init.rom` still hashed
`e8e2768e…78ef`, `init.alarmd` was left at `3d555c02…e94f`, and `misc`'s command field
read zero. The RTC read 1795254832, which was the Mac's UTC, before `adb reboot` at
09:53:56 UTC.

**The bar is drawn and each tab opens its screen.** The operator saw the alarm screen as
before: the time, the battery and the alarm digits, now with a bar along the bottom. A
tap on the bar's middle or right third opened the timer or settings screen, both empty
apart from the bar, as issue 79 intends. The image was still up at 10:07:48 UTC, past the
boot-fail monitor's ten minutes. It was left on the phone.

## 2026-11-21 — alarm sounds on `system`, for issue 82

Read from TWRP over `adb`. Nothing was written: `system` was mounted read-only on a
directory of TWRP's ramdisk and unmounted after.

**The ROM's alarm sounds are on `system`**, in `/system/media/audio/alarms`, 4064 KiB by
`du` with its subdirectory. Probed on the host with `ffprobe` from a copy kept off the
repository:

| File | Bytes | Seconds | Rate, channels |
|---|---|---|---|
| `Argon.ogg` | 507549 | 19.39 | 44100, 2 |
| `Carbon.ogg` | 99766 | 3.36 | 44100, 2 |
| `CyanAlarm.ogg` | 103481 | 1.85 | 44100, 2 |
| `Helium.ogg` | 63612 | 2.09 | 44100, 2 |
| `Krypton.ogg` | 219662 | 7.18 | 44100, 2 |
| `Neon.ogg` | 464721 | 16.00 | 44100, 2 |
| `NuclearLaunch.ogg` | 65601 | 1.50 | 44100, 2 |
| `Osmium.ogg` | 199313 | 6.32 | 44100, 2 |
| `Oxygen.ogg` | 307299 | 10.73 | 44100, 2 |
| `Platinum.ogg` | 933258 | 23.72 | 44100, 2 |
| `Timer.ogg` | 96320 | 3.64 | 44100, 2 |

All eleven are Vorbis; decoded to 16-bit PCM at their own rate they are 16896699 bytes.
`media/audio/alarms` beneath them holds 15 more, 5898 to 180784 bytes, among them
`Alarm_Beep_01.ogg` to `_03`, `Alarm_Buzzer.ogg` and older versions of the eleven, mono
or stereo at 44.1 or 48 kHz; `ffprobe` refused the copy of `Krypton-old.ogg`.
`ringtones` (111 files), `notifications` and `ui` beside it are 12420, 2236 and 444 KiB.

**`system` has room**: `df` reads 3753456 KiB, 1881460 used and 1871996 free.
**`userdata`** reads 51895076 KiB, 7032088 used and 44862988 free.

**One ALSA card**, `/proc/asound/cards`:
`0 [HI6250hi6555cCA]: HI6250_hi6555c_ - HI6250_hi6555c_CARD`. `/proc/asound/pcm`:

```
00-00: HI6210_hi6555c hi6555c-dai-0 :  : playback 1 : capture 1
00-01: MODEM_hi6555c hi6555c-dai-1 :  : playback 1
00-02: FM_hi6555c hi6555c-dai-2 :  : playback 1
00-03: LPP_hi6555c hi6555c-dai-3 :  : playback 1
00-04: hi3660_hi6403_pb_direct hi6555c-dai-4 :  : playback 1
00-05: lowlatency_hi6555c hi6555c-dai-5 :  : playback 1 : capture 1
00-06: mmap_hi6555c hi6555c-dai-6 :  : playback 1 : capture 1
```

`/dev/snd` holds `controlC0`, `hwC0D6`, a `pcmC0D<n>` node for each of these and `timer`.
The formats a device accepts were not read: TWRP has no `tinyalsa`, and `pcm0p/info`
shows no parameters while nothing has the stream open.

## 2026-11-21 — audio in the stock `kernel` and `system`, for issue 14

Read on the host from the backups under `~/p20-backups/`; the phone was not touched.

**The kernel carries its config.** `kernel-mmcblk0p31-stock.img` is an Android boot image
with a gzip'd `Image`, `Linux version 4.9.319-Abdelhay-KSU`, and `IKCFG_ST` inside it. The
audio it builds in, with nothing as a module:

```
CONFIG_SND_SOC=y                 CONFIG_SND_HI6250_HI6555C=y
CONFIG_SND_SOC_CODEC_HI6555C=y   CONFIG_SND_SOC_CODEC_HISSC=y
CONFIG_HIFI_IPC=y                CONFIG_HIFI_IPC_6250=y
CONFIG_HIFI_MAILBOX=y            CONFIG_HIFI_VIR_DAI=y
CONFIG_HIFI_DSP_ONE_TRACK=y      CONFIG_HISI_HIFI_BB=y
CONFIG_HUAWEI_SMARTPAKIT_AUDIO=y CONFIG_SND_SOC_TAS2560=y
CONFIG_TAS2560_I2C=y             CONFIG_TAS2560_MISC=y
CONFIG_TFA9895=y                 CONFIG_SND_USB_AUDIO=y
```

**The HiFi DSP's firmware is not loaded by Linux.** The kernel's strings name `fw_hifi`,
`hifi.img` and `hifi_check_img_loaded`, and load it only on a reset: `reload hifi by
secos`. The suspend lines above count `hifi:s-2, r-2` under the ROM and `hifi:s-0, r-0`
under our `/init`, which shows no DSP activity but not whether it is loaded.

**The codec has a class-D amplifier of its own**: `CLASSD_EN`, `CLASSD VIR_SWITCH` and
`CLASSD_VOLTAGE_CONFIG` are among its control names, beside `DACL MIXER`, `DACL PGA`,
`EP SWITCH` and `EAR OUT`. `smartpakit`'s strings ask the device tree for `chip_vendor`,
`chip_model`, `pa_num`, `hw_reset` and `rw_sequence`, and take `SMARTPA_SPK_POWER_ON`,
`_UNMUTE` and `_SET_SAMPLERATE` as `ioctl`s. The kernel also names
`HI6250_HI6555c_AK4376_CARD`. Which amplifier feeds the speaker is in the device tree,
which is not backed up.

**`system` is /e/OS 0.23**, `lineage_anne-userdebug 9 PQ3A.190801.002`, read with
`debugfs` from `system-mmcblk0p52-stock.img`. `/system/etc/audio_policy_configuration.xml`
attaches `Speaker` and makes it the default output; `primary out` is
`AUDIO_FORMAT_PCM_16_BIT`, 48000 Hz, stereo, and `offload` takes MP3. `system` holds no
`audio.primary.*.so`, no `mixer_paths*.xml`, no `tinyplay` or `tinymix`: they are on
`vendor`, which is not backed up.

## 2026-11-21 — the speaker from TWRP, our debug `/init` and the ROM, for issue 14

Read off the phone in TWRP, under the debug `/init` built from `main` at issue 79, and
under the ROM with `adb root`. Backed up first under `~/p20-backups/`, all read-only:
`fw_hifi` (`mmcblk0p27`), `dts` (`mmcblk0p35`), `odm` (`mmcblk0p44`) and `vendor`
(`mmcblk0p57`). The only writes were `system`'s `/init`, swapped by
`docs/manual-steps/phone.md`'s steps and put back at the end, and `misc`, cleared. The
player was a throwaway probe of raw ALSA and `hifi_misc` `ioctl`s, not committed.

**The speaker is driven by the codec's own class-D.** In the device tree, `smartpakit`,
`smartpa@4c` (TAS2560) and `tfa9895@34` are all `status = disabled`, and
`dual_smartpa_support` is `false`; the charger node carries `pvdd-classd-supply`. Under
the ROM the HAL logs `pa_num: 1`, `pa_type: 1`, and its `smartpa_coef_init` fails.

**The HAL and its routing are on `vendor` and `odm`.** The HAL is
`/vendor/lib64/hw/audio.primary.hisi.so`; the routes are
`/odm/etc/audio/hissc/mixer_paths_anne.xml`. `controlC0` lists 321 controls, the same
count in TWRP and under the ROM. The speaker's route is the path `media-speaker`, which
includes `init`, `audio-play-nossc` and `speaker`: 15 controls on top of 257 top-level
defaults, 272 in all. Setting them sets 271; `CLASSD_VOLTAGE_CONFIG` refuses `5200` with
`EPERM` under every `/init`, and the speaker sounded without it. The last of the 15:

```
SMT_SIF_EN MIXER SIF_CLASSD_EN   1    SMT_MIXOUT_EAR_PU MIXER DACL   1
CLASSD VIR_SWITCH CLASSD_EN      1    SMT_MIXOUT_EAR_PU MIXER DACR   1
```

**PCM device 0 takes only `S16_LE`, stereo**, with `period_size` 8 to 1920 frames and
`buffer_size` 16 to 3840; 1024 × 4 fails `EINVAL`, 960 × 2 at 48000 Hz plays.

**Played with the route set, by where it ran:**

| Where | DSP init sent | Played | Heard |
|---|---|---|---|
| TWRP | none | 30 s in 240.11 s; `remote mb not ready` | no |
| debug `/init` | none | 6 s in 48.05 s; `snd_pcm_print_timeout` every period | no |
| debug `/init` | `SET_DEVICE` alone | 6 s in 48.03 s; the DSP's watchdog fires at the end | — |
| the ROM, HAL running | the HAL's | 6 s in 6.04 s and 6.05 s | yes |
| debug `/init` | the HAL's, replayed | 6 s in 6.03 s, 0 underruns | yes |

TWRP's boot does not load the DSP's image; a normal boot logs `hifi img be loaded`, under
the ROM and under our `/init` alike. A DSP that is loaded but sent nothing consumes a 20
ms period in about 160 ms.

**The HAL's DSP init**, recorded by `ptrace` on `vendor.huawei.hardware.audio@4.0-service`
from its start, with `audioserver` stopped first: `audioserver`'s `onrestart` restarts the
HAL. Twelve `ioctl`s on `/dev/hifi_misc`, in this order:

| # | `ioctl` | Message | Payload |
|---|---|---|---|
| 0 | `0x40184171` sync | `PLAY_PAUSE_REQ`, `0xdd53` | 8 bytes, 8 back |
| 1 | `0xc0184175` `WRITE_PARAMS` | the parameter block, cleared | 309608 zero bytes |
| 2 | `0xc0184175` `WRITE_PARAMS` | the parameter block, filled | 309608 bytes, 54844 non-zero |
| 3 | `0xc0184175` `WRITE_PARAMS` | the same, sealed | `aa55aa55` over the first 4, 4 more at the end |
| 4 | `0xc00c4170` async | `0xdd9c`, cvaa mono | `0` |
| 5–6 | `0x4018417c` | USB audio queries | 12 bytes, 300 back |
| 7 | `0xc00c4170` async | `SET_DEVICE`, `0xdd96` | device `2`, the speaker; mask `0` |
| 8–9 | `0xc00c4170` async | `SET_PARA`, `0xdf08` | 16 bytes: `08df0700 01000000 04000000 04000080` |
| 10–11 | `0xc00c4170` async | `SET_SOURCE`, `0xdd95` | `0` |

An async argument is `{ptr_lo, ptr_hi, len}`; a sync one and `WRITE_PARAMS` add
`{out_lo, out_hi, out_len}`. A message is a `u16` id, two bytes of padding and `u32`
values. The HAL builds the parameter block from `odm`'s
`/odm/etc/audio/{dts,mbdrc,modem,sws}/*_anne_normal.xml`, logged as `ap_param_init`,
`clear_hifi_nv` and `copy_algo_param_to_hifi`.

**Replayed under the debug `/init`**, all but 5–6, every `ioctl` returned 0 and the kernel
logged the same messages as under the ROM. The route set after it, the clip played in real
time and was heard through the speaker. The draw while it plays was not read.

## 2026-11-21 — the alarm list, for issue 80

`alarmd` holds no serial shell, so every reading here was taken from TWRP: the settings
record off `reserved4`, decoded by slot and sequence, and `alarmd`'s lines in
`/sys/fs/pstore/console-ramoops-0`, which holds the boot before TWRP and is garbled in
places.

**The first build** was `23e16ed`, 526896 bytes, sha256
`0642d0672ab584bac3fdcd00896eefaa4d0e467e7990074c6518970044bc2fb9`, copied over `/init` on
`system` at about 11:39 UTC, replacing issue 79's `bcd39a5e…bf6d`. The operator added two
alarms from the slider, removed one and made the other active, and the record at
sequence 12 held what the panel showed. **A stray touch then emptied the list**: pstore
showed both alarms removed at 11:45:54 UTC, about 3 s after a key press lit the panel,
while the phone was being gripped for TWRP with a thumb on the crosses at the right
edge, and sequence 13 was saved with no alarms. This is why a cross now removes only on a
second tap.

**The second build** was `ba17a76`, 527008 bytes, sha256
`a85fca970ae7322ab98c0fa34bff7d5831468d4238bdc089bf2c6e2bb3c011e4`, copied over `/init` at
13:41 UTC. `init.rom` still hashed `e8e2768e…78ef`, `misc` read zero, and the RTC read the
Mac's UTC to the second before `adb reboot` at 13:41:59 UTC.

**The list survives a reboot.** The operator added two alarms, saw a first tap turn a
cross red and a tap elsewhere clear it, removed one with two taps and made the other
active. pstore read `alarms [07:00], active none`, then `alarms [07:00], active 07:00`
and `settings saved`; sequence 14 held 07:00, every day, armed, with the CRC good. After
`adb reboot` the operator saw 07:00 still listed and lit.

**The list survives a power-off, and the active alarm rings.** The operator added 15:10
local and made it active, pulled the cable and turned the panel off. The phone powered
itself off, came back up on the RTC, and rang and vibrated at 15:10 local, 14:10 UTC;
Power dismissed it, and the panel still listed 07:00 and 15:10. From TWRP, sequence 15
held 07:00 unarmed and 15:10 armed, both every day, CRC good. `console-ramoops-0` was
absent, the power-off having cleared the RAM it lives in. The image was left on the
phone.

## 2026-11-21 — the power-off toggle, for issue 83

`alarmd` was built from `3edef46`, 527472 bytes, sha256
`da4dc63c1fca18267514fb687b223c5c5edc4e7755e8042badaf6973ad5c867d`. From TWRP it was copied
over `/init` on `system`, replacing issue 80's `a85fca97…11e4`. `init.rom` still hashed
`e8e2768e…78ef`, `misc`'s command field read zero, and the RTC read 1795270900, the Mac's
UTC to the second, before `adb reboot` at 14:21:44 UTC, with the battery full on the
cable.

Before it, `reserved4` held issue 80's version-1 records: sequence 15 in slot 1, with 07:00
unarmed and 15:10 armed, both every day. The key reset into TWRP emptied pstore after each
run, so nothing below comes from `alarmd`'s log; the readings are the operator's and the
record's.

**With the toggle off, the phone stayed suspended past the window.** The settings screen
opened with the switch lit and its knob right, the toggle on as a version-1 record loads.
The operator tapped it dim, pulled the cable and turned the panel off, and after at least
seven minutes a press showed the time at once, with no boot. Slot 0 then held sequence 16
in format version 2, payload `100e0000 78000000 00 02 07007f00 0f0a7f01`: the offset, the
window, the toggle off, and both alarms as before, with the CRC good and the rest of the
slot zero.

**The toggle survived a reboot.** After `adb reboot` at 14:39:41 UTC the operator saw the
switch still dim.

**With the toggle on, the phone powered off.** The operator tapped the switch lit, pulled
the cable and turned the panel off; a press at least four minutes later booted the phone,
as only a power-on from off does. Slot 1 held sequence 17, version 2, with the toggle byte
`01` and the alarms unchanged, CRC good. The image was left on the phone.

## 2026-11-21 — the DSP init and the speaker route on `system`, for issue 91

Under the ROM, booted from TWRP with `system`'s `/init` swapped for `init.rom` by `mv` and
ours kept as `init.ours` (`da4dc63c…c867d`). `dsp-recorder` was built from `3e0658f` and
pushed to `/data/local/tmp`; nothing else on the phone was written under the ROM.

**The HAL opens `/dev/hifi_misc` only once `audioserver` connects.** With `audioserver`
stopped and the HAL started under the recorder, the recorder waited out its 30 s with 0
`ioctl`s; started HAL first, then `audioserver`, it saw all twelve.

**A slow tracer gets the HAL restarted.** With the recorder polling `waitpid` with
`WNOHANG`, `audioserver` aborted on its 5 s `TimeCheck` and the HAL ended after 6
`ioctl`s. Waiting blocking, with a watchdog thread for the deadline, every recording
finished.

**Messages 8–11 race.** Seven recordings had the twelve `ioctl`s of the issue 14 reading,
messages 0–7 the same byte for byte. Five carried 8–11 as `SET_PARA`, `SET_PARA`,
`SET_SOURCE`, `SET_SOURCE`; two as `SET_PARA`, `SET_SOURCE`, `SET_PARA`, `SET_SOURCE`, with
the same bodies, `08df0700 01000000 04000000 04000080` and `95dd0000 00000000`.
`dsp_init.py` writes both `SET_PARA` first; the two recordings of the final run, one of
each order, then agreed.

**The speaker route** is 257 defaults and 15 `media-speaker` controls, read from
`/etc/audio/hissc/mixer_paths_anne.xml` in `~/p20-backups/odm-mmcblk0p44-2026-11-21.img`.

**Pushed from TWRP** by `system_push.py` to `system/etc/alarmd/`, mode 644: on the
partition `/etc` is a link to `/system/etc`, and a first push to `etc/alarmd` failed on it.

| File | Bytes | sha256 |
|---|---|---|
| `dsp-init.bin` | 928990 | `bd239d63c82bdf09c88df740407550e142d8822ee0b43324bfcfea9a53b41efa` |
| `speaker-route.tsv` | 10067 | `f522fb9512b7bfe82de84a81f6dd0b9d7a22e3b475b69dcf910b57ede6c709b7` |

Then `init` was moved to `init.rom` and `init.ours` to `init`; read back on a fresh
mount, `init` hashed `da4dc63c…c867d`, `init.rom` `e8e2768e…78ef` and `init.alarmd`
`3d555c02…e94f`. `misc`'s first 32 bytes were zeroed, and the phone was left in TWRP.

## 2026-11-21 — the timer screen, for issue 81

`alarmd` was built from `30a592a`, 534784 bytes, sha256
`c459b465ef1bf077a5f98b97f1fefb8b076a918d0cf9324354160611048b472f`. From TWRP it was copied
over `/init` on `system`, replacing issue 83's `da4dc63c…c867d`. `init.rom` still hashed
`e8e2768e…78ef`, `misc`'s first 32 bytes read zero, and the RTC read 1795275548, the Mac's
UTC to the second, before `adb reboot` at 15:39:14 UTC, with the battery full on the cable.

Before it, `reserved4` held issue 83's sequence 17 in slot 1: version 2, the toggle on,
07:00 unarmed and 15:10 armed. The readings below are the operator's, the record's, and
`alarmd`'s lines in `console-ramoops-0`, garbled in places; kernel time 0 was 1795275554.

**A version-2 record loads with no timers.** The timer screen opened with dim dashes beside
a dim play and both wheels at 00, and the log read `timers []`.

**The saved timers are kept in version 3.** The operator saved 01:00, then 05:00. Slot 0
held sequence 18 with 01:00 alone; slot 1 then held sequence 19, version 3, payload
`100e0000 78000000 01 02 07007f00 0f0a7f01 02 0100 0500`: the alarms and toggle as before,
then two timers, with the CRC good and the rest of the slot zero.

**A running timer held the phone suspended and rang at its end.** A 01:00 timer woke a
suspend armed for 18:19:18 UTC and lit the panel. Then the operator started 05:00, logged
`running until 1795285506`, 18:25:06 UTC, pulled the cable and turned the panel off. At
18:22:10 UTC, two minutes after `settings saved`, `alarmd` resumed from a suspend armed for
the window's end and suspended again rather than powering off. At 9952 s, 18:25:06, the
panel lit; the log read `Ringing` with `running until none`, and the operator saw it amber
and felt the vibrator. Power dismissed it at 9996 s.

**The saved timers survived a power-off.** With the cable out, the operator turned the
panel off at 10006 s. pstore ends at 10014 s with the eMMC timing out and the modem's
power-management notifier, as at a shutdown, and holds no later boot. A press of Power
booted the phone, and the timer screen listed 01:00 and 05:00, with 01:00 on the wheels.

**The saved timers survived a reboot.** After `adb reboot` from TWRP at 18:36:33 UTC the
operator saw 01:00 and 05:00 still listed. The image was left on the phone.

## 2026-11-21 — how long the panel stays lit, for issue 97

`alarmd` was built from `45cf3cb`, 535696 bytes, sha256
`1c15b1828e90908bf6858c69f7d2d60a97c56288ba91dddbcad09d82f0a4499b`. From TWRP it was copied
over `/init` on `system`, replacing issue 81's `c459b465…472f`. `init.rom` still hashed
`e8e2768e…78ef`, `misc`'s first 32 bytes read zero, and the RTC matched the Mac's UTC
before `adb reboot` at 20:53:40 UTC.

Before it, `reserved4` held issue 81's sequence 19 in slot 1, version 3. `alarmd` holds no
serial shell, so the readings are the operator's, the record's, and `alarmd`'s lines in
`console-ramoops-0`, garbled in places, from the boot before each key reset into TWRP.

**A version-3 record loads lit for 10 s, and each choice times the panel.** The operator
reset by the keys once more, and pstore holds the boot from about 20:59:25 UTC. The
settings row stepped through every choice and wrapped to 10 s, more than once. With 10 s
set the panel went off 10.4 s after the last change, and with 120 s about 123 s after it,
with no key between. The lit time is saved in version 4: slot 0 held sequence 20 ending
`7800`, 120 s, and slot 1 sequence 21 ending `1e00`, 30 s, after the timers.

**The 30 s survived a reboot.** After `adb reboot` at 21:13:00 UTC the boot logged
`settings loaded` and no change, and the operator saw `00:30` on the settings screen. A
press of Power lit the panel at 43.4 s and it went off at 84.82 s, 41 s, with no key
between; a touch restarts the time and is logged only when it changes something, and no
cause was read.

**A press keeps the panel lit for 30 s.** After `adb reboot` at 21:17:22 UTC the boot lit
the panel at 13.66 s and it went off at 44.02 s. With the phone lying untouched, a press
of Power at 128.44 s lit it at 129.20 s, and it went off at 159.54 s, 30.3 s later with no
key between; the operator timed about 30 s. `init` still hashed `1c15b182…499b`, and the
image was left on the phone after `adb reboot` at 21:23:01 UTC.

## 2026-11-21 — an alarm sound on `system`, for issue 87

From TWRP 3.7.0 over `adb`, with `scripts/alarm_sound.py` from `e805df4`. `ffmpeg` 9.0.2
converted `sounds/ringtone.mp3` to `out/sounds/ringtone.wav`: 5760044 bytes, a bare
44-byte header and 5760000 bytes of 16-bit 48 kHz stereo PCM, sha256
`cb3a132041de70b4b09cf058c99f7b1169ca5aaa17239fefc9b90564c58d8836`.

**The ROM's alarms before**, by `sha256sum` on a read-only mount of `mmcblk0p52`, their
sizes as in issue 82's reading:

| File | sha256 |
|---|---|
| `Argon.ogg` | `7f4cab5b…6846` |
| `Carbon.ogg` | `ce04cf92…6b96` |
| `CyanAlarm.ogg` | `8f8e7477…fd90` |
| `Helium.ogg` | `45d0fe39…08f6` |
| `Krypton.ogg` | `762ba856…6faf` |
| `Neon.ogg` | `9354b5dc…c721` |
| `NuclearLaunch.ogg` | `d0befea3…5898` |
| `Osmium.ogg` | `2c050bd2…0fdd` |
| `Oxygen.ogg` | `2a698c8e…b8b1` |
| `Platinum.ogg` | `c2050379…6b86` |
| `Timer.ogg` | `7f440c55…86ac` |

**The first run pushed it**: `/mnt/sys/system/media/audio/alarms/ringtone.wav: 5760044
bytes, sha256 cb3a1320…8836`, and `11 files beside them unchanged`.

**The second run wrote nothing**: it printed `already there`, 5760044 bytes and the same
sha256, and the same 11 unchanged.

**A different sound under the same name was refused**: a 1 s 880 Hz tone as
`ringtone.flac`, converted to 192044 bytes, exited 1 with `is already there … nothing
written`, and `system` was unmounted after.

**The ROM's alarms after**, read the same way, hashed as before, all eleven. `ringtone.wav`
reads 5760044 bytes, mode 644, with the host's sha256, and no `.part` file is left. `df`
reads 3753456 KiB, 1888036 used and 1865420 free.

## 2026-11-22 — the speaker rings the alarm, for issue 92

Read under the debug `/init` `7a27e4dc…e7f8` and then under `alarmd`, both on `system`,
swapped from TWRP; `init.rom` stayed at `e8e2768e…78ef` and `init.alarmd` at
`3d555c02…e94f`, and `system/etc/alarmd/` held issue 91's `dsp-init.bin` `bd239d63…1efa`
and `speaker-route.tsv` `f522fb95…09b7` beside issue 87's `ringtone.wav` `cb3a1320…8836`.
The players were throwaway probes of the `speaker` crate, not committed. Times in the
kernel log lines are its clock.

**The route writes `Off` to a boolean.** `HISSC_S1_MST_SLV` is a `BOOLEAN` control and the
route gives it `Off`, as `mixer_paths_anne.xml` does; the adapter now reads `Off` and `On`,
in any case, as 0 and 1 on a boolean, and nowhere else. With it, all 272 values resolve.

**Four controls read 1 where the route writes 0.** `HBF1I_EN` to `HBF4I_EN`, numids 1 to
4, read 1 two seconds into a stream after the route had written 0, and the write's
encoding was checked against the kernel's. The rings below were heard with them so.

**A stream started straight after the DSP init plays at an eighth of its speed.** Under the
debug `/init`, a fresh boot, the replay and then the probe's start within milliseconds:
6 s of the clip took 48.02 s, each 20 ms period about 160 ms, nothing was heard but a
click at the start and stop, the DSP's `sochifi watchdog` fired about 110 ms after the
stream ended, and a second stream in that boot was dead. Neither booting the ROM first, as
issue 14 did, nor replaying issue 14's recording of the HAL changed it: 48.02 s again.

**The gap between the init and the stream decides it.** Issue 14's heard run had, by
chance, about 5 s between its replay and its play, from two commands typed apart.

| Gap after the replay | 6 s of the clip took | Watchdog | Heard |
|---|---|---|---|
| a few ms | 48.02 s | yes | no |
| 10 s | 6.04 s | no | yes |
| 2 s | 6.03 s | no | — |
| a second play, same boot | 6.03 s | no | — |

The adapter replays the init when it opens, at start-up, and a start within 5 s of it
waits out the rest. Under the debug `/init`, a probe that opened and then started twice,
5 s each and 10 s apart, moved `hw_ptr` from 63360 to 160320 over 25.78 to 27.81 s and
from 62400 to 159360 over 40.84 to 42.87 s: 96960 frames in 2.03 s, about 47800 a
second. Every start and stop returned `Ok`, no watchdog fired, and both rings were heard.

**The draw while it rings**, the same probe run again in that boot, so a second replay into
a DSP that had played, with the panel lit and `enable_hiz` 1 under
`/sys/class/hw_power/charger/charge_data/` so the battery carried the phone. `current_now`
of `/sys/class/power_supply/Battery`, in mA, negative for a discharge:

| When | `current_now` |
|---|---|
| before the first start | −348 to −365 |
| ringing, both rounds | −390 to −398 |
| after each stop | −351 to −363 |

About 40 mA while it rings, and back to the baseline at the stop. `enable_hiz` was written 0
after, and `current_now` read 177. No watchdog fired.

**`alarmd` rang it.** Built from `4148cfc`, 575920 bytes, sha256
`dda6d763bcfccea70edc7e11b8421c44ddb81b6096b19ac52481191aa64f5041`, it was copied over
`/init` from TWRP, with the debug `/init` kept as `init.debug`. The RTC read 1795351268,
one second behind the Mac's UTC, `misc` was zeroed, and `adb reboot` ran at 12:41:20 UTC.
The log below is from pstore, read in TWRP the next morning after a key reset.

- **After a cold boot.** Booted by `Other`, toggle off, on the cable, an alarm at 13:45
  at UTC+1. The panel went dark at 65 s and the phone suspended until 12:45:00 UTC; it
  rang at 223 s, through the speaker with the vibration and the amber face, and a Volume
  press snoozed it. The snooze rang again from a suspend armed for 1795352040, and a press
  of Power at 763.34 s made it `Quiet`.
- **After a suspend.** Off the cable, an alarm at 14:06: resumed from a suspend armed for
  1795352760 at 1479.61 s, rang, and a press of Power at 1482.75 s made it `Quiet`. The
  operator heard both, and reported each as planned.

**After a power-on alarm**, on 2026-11-23, after `alarmd` was rebooted from TWRP. Twice the
operator turned the toggle on, set an alarm about eight minutes ahead, pulled the cable and
turned the panel off; each time the phone powered off, booted by itself and rang through
the speaker, and Power stopped it. pstore was empty after both runs, so these readings are
the operator's and `reserved4`'s:

| Run | Slot written before the power-off | Slot written after the boot |
|---|---|---|
| first | sequence 25: toggle on, 11:51 armed, 14:06 | sequence 26: toggle **off**, the alarms the same |
| second | sequence 27: toggle on, 12:08 armed | none |

After the first run the operator found the toggle off on opening settings, having not
touched it; after the second it was still on. Only a tap on the toggle's row turns it off
once the settings load, so the first run's change is unexplained here.

The image was left on the phone, with the RTC on the Mac's UTC.

## 2026-11-23 — glyph rasterising on the Kirin 659, for issue 95

From TWRP 3.7.0_9-0 over `adb`, about 12:50 UTC. A prototype built outside the repository
with `alarmd`'s release profile for `aarch64-unknown-linux-musl` filled a 1080×2280 XRGB
frame black, then drew `19:37` and `06:45` in Inter at weight 300, and three tab icons and
the battery from Material Symbols, blended white over black. The fonts were the two
subsets of ADR 0016, D2 and D3: 2792 bytes, sha256 `ca8c81fb…9cec`, and 2904 bytes,
`b58cfc19…157d`. Each binary ran from `/tmp`, pinned to cpu0 with `taskset 01`, averaged
over 50 frames, three runs each, and was deleted afterwards.

Both clusters are Cortex-A53, part `0xd03`. The kernel's `CONFIG_CPU_FREQ_DEFAULT_GOV` is
`PERFORMANCE`, which `alarmd` leaves as it is (ADR 0007), so cpu0 runs at 1709 MHz under
our `/init` as under TWRP.

| Binary | Bytes | sha256 | Fill | Fill and glyphs |
|---|---|---|---|---|
| no font | 344280 | — | 1.65 ms | — |
| `ttf-parser` and `ab_glyph_rasterizer` | 403008 | `b9c49b64…931f` | 1.65 ms | 14.52–14.54 ms |
| `fontdue` | 426496 | `e5891014…76e2` | 1.65 ms | 15.14–15.15 ms |

The glyph boxes covered 351391 pixels; 110725 came out lit, 13221 of them grey, the same
counts as on the Mac. **The glyphs cost 12.9 ms per frame** at 1709 MHz.

With cpu0 and cpu4 set to `powersave`, cpu0 ran at 480 MHz and the same frame took
44.57–44.62 ms over a 3.28 ms fill, 41.3 ms for the glyphs. Both were set back to
`performance` afterwards.

**Rasterising is two thirds of the cost.** A second binary, `d75d732b…733e`, rasterised ten
digits at the alarm's size, 165712 box pixels, in 4.33 ms, and blended them in 2.19 ms:
26 ns and 13 ns per box pixel.

## 2026-11-24 — the figures in Inter Light, for issue 104

From TWRP 3.7.0_9-0 over `adb`, about 13:00 UTC. Two throwaway binaries, built outside the
repository with `alarmd`'s release profile for `aarch64-unknown-linux-musl`, called
`render` at 1080×2280 for the alarm screen with four saved alarms, 19:37 and 06:45 active.
Each ran from `/tmp`, pinned to cpu0 with `taskset 01`, averaged over 50 frames, three runs
each, and was deleted afterwards. cpu0's governor read `performance` at 1709 MHz.

| `alarm-core` at | Binary sha256 | `render` |
|---|---|---|
| `c3c8d1a`, seven segments | `fce96eb2…ea27` | 9.17–9.28 ms |
| `67e3711`, Inter Light | `9205c0ac…b5e7` | 30.10–30.17 ms |

**The figures cost about 21 ms per frame**, and `render` stays under ADR 0016, D7's 33 ms,
so its glyph cache stays rejected. D7 estimated 28 ms.

**`alarmd` at `67e3711` is 632680 bytes**, sha256
`89567298061c03ece5172e61a0215710a21b19198f928ca56c36e043a91d551f`; `main` at `c3c8d1a`
builds to 578096, so the type adds 54584 bytes. It was copied over `/init` on `system` at
13:07 UTC, replacing issue 92's `dda6d763…f5041`. `init.rom` still hashed `e8e2768e…78ef`,
`misc`'s first 32 bytes read zero, and the RTC read 1795525656 against the Mac's
1795525658, before `adb reboot` at 13:07:42 UTC.

**The figures are Inter Light on all three screens.** The operator saw the clock, the
active alarm, the wheels, the saved alarms, the battery's percentage, the timer and the
lit time drawn thin with smooth edges, nothing overlapping, and taps on the wheels, a saved
alarm and the tabs landing as before. The operator photographed the alarm, timer and
settings screens; the photos are kept off the repository. The image was left on the phone.

## 2026-11-24 — the icons in Material Symbols, for issue 105

**`alarmd` at `d2e9976` is 635048 bytes**, sha256
`746f420864d88a7609fb59f58ad6c9a1f833c396dbe719b432232af5db154559`, 2368 bytes more than
issue 104's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
104's `89567298…d551f`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795527416 against the Mac's 1795527418, before `adb reboot`
at 13:37:06 UTC.

**The icons are Material Symbols on all three screens, and every tap lands.** The operator
saw the tab bar, the smaller battery, the power symbol, the switch, the sun, the wheels'
arrows, the `+`, the crosses, the play and the stop drawn thin with smooth edges, and taps
on the arrows, the `+`, a saved alarm and its cross, the timer's start row, the switch's
row, the lit time's row and the tabs landing as before. No photos were taken; the
operator takes them once the interface is final. The image was left on the phone.

## 2026-11-24 — the wheels' arrows step the right way, for issue 108

**`alarmd` at `bc97446` is 635048 bytes**, sha256
`eb817803912cefa23970e80cd2c84426f0002f989cb61e76debff6d1d94f3387`, the same size as issue
105's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue 105's
`746f4208…4559`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes read
zero, and the RTC read 1795528188 against the Mac's 1795528191, before `adb reboot` at
13:49:54 UTC.

**A tap on an up arrow counts up and on a down arrow counts down, on both screens.** The
operator tapped the hour's up arrow and the minute's down arrow on the alarm screen, and
the minutes' and seconds' arrows on the timer screen, and each stepped the way its arrow
points; a drag up still counted up. The image was left on the phone.

## 2026-11-24 — only Power ends a ring, for issue 113

**`alarmd` at `5cbe5f9` is 634824 bytes**, sha256
`72f13c7cc0fce04edf9e272a2208696924b0996368c93778be3e51b50bf6bf68`, 224 bytes less than
issue 108's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
108's `eb817803…3387`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795529833 against the Mac's 1795529836, before `adb reboot`
at 14:17:20 UTC.

**Volume Up and Volume Down leave a ring ringing, and Power dismisses it.** The operator
set an alarm a minute or two ahead and let it ring, then pressed Volume Up and Volume Down
a few times each: the screen stayed amber, the speaker kept looping and the vibrator kept
pulsing. Power ended the ring. The image was left on the phone.

## 2026-11-24 — the settings screen reboots into TWRP, for issue 118

**`alarmd` at `f4a7168` is 634440 bytes**, sha256
`2f38bdfe9f8384d92d4dba49ce0b4c246921fea8097603e24dba86564751e35e`, 384 bytes less than
issue 113's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
113's `72f13c7c…bf68`. `init.rom` still hashed `e8e2768e…78ef` and `misc`'s first 32 bytes
read zero before `adb reboot`.

**A first tap on the recovery row turns it red, and a second reboots into TWRP.** The
operator opened the settings screen and tapped the wrench row twice; TWRP came up on USB
at 14:55:16 UTC. Its pstore held `rebooting into recovery` as `alarmd`'s last line, 5 s
after `panel lit`, and `misc`'s first 32 bytes still read zero: `reboot(2)` into recovery
writes no BCB. An `adb reboot` from TWRP then brought no TWRP back within 150 s and left
nothing on USB, `alarmd` running again. The image was left on the phone.

## 2026-11-26 — a press inside a suspend's entry, for issue 117

**In TWRP, a press while the kernel enters a suspend leaves no press able to wake it.**
With #118's `alarmd` (`2f38bdfe…e35e`) on `/init` and the cable in, a script suspended
the phone at each new `down` interrupt of Power, `echo mem` with the RTC armed 40 s
ahead, as `alarmd` does when Power darkens the panel. The operator pressed quickly, about
five times a second. Suspends 1 to 3 were woken by a press within 1 to 2 s. In suspend
4, `power key press interrupt!` at 711.16 s and 711.37 s came while the kernel was still
entering it, were handled as plain interrupts and did not abort it. The `down` counter in
`/proc/interrupts` then held at 63 through about 39 s of presses, until `wake up irq num:
78, irq name: RTC0` at 750.51 s. A read of `/sys/power/wakeup_count` blocked on the cable:
`charge_wakelock` and `usb_wake_lock` were active.

**`alarmd` at `0c039d9` is 634664 bytes**, sha256
`74b284163341a284e642c55077d6124e01653ea4a881ff614083bce7deee3aad`. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
118's `2f38bdfe…e35e`. `init.rom` still hashed `e8e2768e…78ef` and `misc`'s first 32
bytes read zero before `adb reboot` at 09:48:33 UTC.

**Quick presses no longer hang it, on the cable or off it.** The operator pressed quickly,
then waited and pressed, on the cable and then off it, and used the recovery row; pstore
held the boot. A run of 18 presses from 129.42 s to 138.25 s toggled the panel 18 times
with no suspend between them. Presses woke a suspend and lit the panel at 129.42 s
(`panel lit` 130.19 s), 317.81 s (318.11 s), 366.34 s (366.64 s), 443.26 s (443.55 s) and
529.14 s (529.43 s), each before the RTC's armed instant; the last came 10.8 s after Power
dismissed a ring and darkened the panel at 518.36 s. This kernel printed no `wake up irq`
line for them.

**The RTC fired inside a suspend's entry once, and its wake was lost.** A 5 s timer
started at 470.76 s, due at 1795686984; Power darkened the panel at 471.80 s and `alarmd`
suspended about 2 s later, armed for it. `[hisi_soc_rtc_interrupt]RTC Interrupt Comes` at
474.51 s came before `hisi_pm_enter ++` at 474.85 s, and the phone slept until a press at
487.82 s; the timer rang at 490.19 s. A second 5 s timer, armed about 2 s ahead, woke on
time at 510.59 s. `alarmd` since stays awake for a wake due within 5 s.

**`alarmd` at `4650b91` is 635032 bytes**, sha256
`a859af311e4183cef531e96959ff66bddc6fa0fd1024fcfae361c41dd79bb236`, copied over `/init` in
the same way, replacing `74b28416…3aad`, before `adb reboot` at 10:02:47 UTC.

**A timer due within 5 s of the panel going off rings on time.** The operator started a
5 s timer and pressed Power at once, twice, then a 10 s timer followed by a run of quick
presses, and used the recovery row; pstore held the boot. The timers were set at
111.45 s, 126.63 s and 144.26 s; Power darkened the panel at 112.53 s, 128.31 s and, after
ten presses from 144.67 s, at 149.27 s; the panel lit for each at 116.75 s, 132.52 s and
154.48 s, with no `hisi_pm_enter` between, and Power ended each ring. A press at 95.43 s
had woken the suspend begun at 45.10 s and lit the panel at 96.17 s. The image was left
on the phone.

## 2026-11-26 — the lit icons in a blue-to-purple accent, for issue 121

**`alarmd` at `553b7c8` is 635816 bytes**, sha256
`0d9316ed8077d97375124c4d03bcbb6be23f6e40668125b1b55434cc7d816a47`, 784 bytes more than
issue 117's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
117's `a859af31…b236`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795690033 against the Mac's 1795690038, before `adb reboot`
at 10:47:20 UTC.

**Every lit icon runs from blue `#57a2f3` to purple `#6e41d2`, and the end colours stay.**
The operator saw the open tab on all three screens, the alarm's `+`, the timer's play and
stop, and the settings switch drawn in the accent, each dim one in grey, the figures in
white and a marked cross in red, and judged the two end colours right on the panel. No
photos were taken. The image was left on the phone.

## 2026-11-26 — the volume keys set the ring's volume, for issue 110

**`alarmd` at `0084fd9` is 637672 bytes**, sha256
`709c4f44bb6d30b9700515b8dd76959bb78559ae862555ec3f2f4f549aec9adb`, 1 856 bytes more than
issue 121's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
121's `0d9316ed…6a47`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795691564 against the Mac's 1795691568, before `adb reboot`
at 11:12:49 UTC.

**The volume keys show the panel at the left edge and move its fill, ringing or not.** The
operator saw the panel and its fill follow each key on a lit panel, and heard a ring grow
quieter and louder as it played. The first boot loaded the version-4 record at 15, and
the first key logged `volume 14` at 29.18 s; quiet, the keys stepped it to 6. A 5 s timer
rang from about 40.7 s, and the keys took it to 1 by 42.21 s, to 15 by 49.39 s and back to
4, with no log line between `volume 1` and `volume 15` other than the steps. Power ended
the ring at 64.95 s at volume 5. `settings saved` came at 99.98 s, before the recovery row's
reboot.

**The level survives a reboot.** `reserved4`'s first slot read version 5, sequence 34, 26
bytes, its volume byte 5; the second kept the version-4 slot, sequence 33. After
`adb reboot` at 11:15:35 UTC, the first Volume Down on the lit panel showed 4 and logged
`volume 4` at 40.98 s. The image was left on the phone.

## 2026-11-26 — the timer tab's list and running stages, for issue 111

**`alarmd` at `399aae6` is 648376 bytes**, sha256
`0220ee94851a748139eb1b07cc05984e2bb2e9b797dd12c30db9336639c06d67`, 10 704 bytes more than
issue 110's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
110's `709c4f44…9adb`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795693099 against the Mac's 1795693103, before `adb reboot`
at 11:38:24 UTC.

**A saved timer played from the list rings at its end and Power returns to the list.** The
operator saw the list with each saved timer's play and cross, the running stage's ring
empty as the time ran down, and the list again after each dismissal. A 00:05 timer played
at 161.26 s logged `running until 1795693266`; the speaker started at 167.05 s and the
`Ringing` line followed at 169.11 s, the speaker's start before it as in issue 110. Power
ended it at 199.53 s. A 00:10 timer played at 210.30 s rang from 220.51 s, and Power at
222.71 s ended it. Neither was paused before it rang.

**Pause holds the time left, resume continues it, and the X returns to the list.** With
00:05 and 00:10 removed by their crosses and 01:19 added, its play at 248.68 s logged
`running until 1795693427`; the pause at 249.74 s logged `running paused 01:18`, the
resume at 251.51 s `running until 1795693429`, the end moved on by the time held. A second
pause at 254.57 s logged `running paused 01:15`, and the X at 255.30 s `running none`; the
operator saw the list at once. `settings saved` came at 273.45 s, before the recovery row's
reboot. No photos were taken. `misc` was cleared again and the phone rebooted into the
image at 11:44:17 UTC, where it was left.

## 2026-11-26 — the ring's sound and vibration toggles, for issue 115

**`alarmd` at `84c48b0` is 649656 bytes**, sha256
`550a45596640bcd5987f7b768aff258f0ac89eef4064ec93dbf46e1217d16ebc`, 1 280 bytes more than
issue 111's. From TWRP 3.7.0_9-0 it was copied over `/init` on `system`, replacing issue
111's `0220ee94…6d67`. `init.rom` still hashed `e8e2768e…78ef`, `misc`'s first 32 bytes
read zero, and the RTC read 1795695953 against the Mac's 1795695957, before `adb reboot`
at 12:25:58 UTC.

**The settings screen shows a sound row and a vibration row, each turning its output
alone.** The operator saw both rows with their switches between the lit time and recovery,
and reported that every toggle worked as it should. A timer with the sound off, started at
709.04 s, logged `Ringing, … sound off, vibration on` at 717.92 s, and Power at 720.50 s
ended it. One with the vibration off, started at 722.50 s, rang at about 732.4 s; the
speaker codec's `hac_gpio_switch` line came at 732.03 s, and Power at 734.24 s ended it. One
with both off, started at 738.86 s, rang at about 749.4 s with no codec line, the face amber
alone, and Power at 751.57 s ended it. The sound-off ring opened no codec either.

**A ring that starts and ends within one turn of the loop logs no `Ringing` line.** With the vibrator
off, no pulse shortens the loop's wait, so the vibration-off and both-off rings began and
were ended by Power within one turn, and `change` logged only `running until` to
`running none`. The power key's interrupts and the codec line above date each ring.

**The toggles survive a reboot.** `settings saved` came at 781.95 s, before the recovery
row's reboot at 872.18 s. `reserved4`'s second slot read version 6, sequence 37, 28 bytes,
its volume, sound and vibration bytes 6, 0 and 0; the first kept issue 111's version-5
slot, sequence 36. After `adb reboot` at 12:42:06 UTC, the operator saw both switches off.
No photos were taken. The image was left on the phone.

## 2026-11-26 — the alarm screen's ring and list, for issue 127

**The alarm screen draws in 30.9 ms a frame.** A bench outside the repository, sha256
`468fd992306cd69cef9377be430abd3d056dd8c21d1c55b6898af18cd4acfcb7`, rendered the board at
1080×2280 with five alarms, four armed and one with its arc, in three runs of 50 frames
under `taskset 01` from TWRP, cpu0 `performance` at 1709000 kHz: 31.14, 30.89 and 30.86 ms.
The editor took 15.38 to 15.53 ms. With the ring drawn from its outline the same board took
82 ms; `c8dbdec` draws the band and the arc analytically, in one pass.

**`alarmd` at `c8dbdec` is 666656 bytes**, sha256
`ebe4f44bbbbc06d51e661388ff7d9f9a9199c4573a454f80a052041f572e506d`. From TWRP 3.7.0_9-0 it
was copied over `/init` on `system`, replacing issue 115's `550a4559…6ebc`. `init.rom` still
hashed `e8e2768e…78ef`, `misc`'s first 32 bytes were zeroed, and the RTC read 1795704504
against the Mac's 1795704508, before `adb reboot` at 14:48:33 UTC.

**Alarms are armed together, each with its days and its own switch.** A line is logged only
when a tap changes something. At 55.91 s `alarms [10:26 MTWTFSS on, 12:08 MTWTFSS on,
15:49 once on, 15:49 MTWTFSS on], next 1795771560` listed a one-off armed beside three
daily alarms; at 82.36 s `12:08 MTWTF--` was armed for workdays alone, `next 1795777680`,
Friday 11:08 UTC. At 153.43 s `[10:26 MTWTFSS on, 12:08 MTWTF-- on, 15:53 MTWTFSS on]` gave
`next 1795704780`, 14:53 UTC that day. The phone `resumed from a suspend armed for
1795704780` at 271.06 s, and at 278.75 s `15:53 MTWTFSS on` stood with `next 1795791180`,
the same time on Friday. Sound and vibration were off, so the ring logged no `Ringing`
line, as issue 115 found.

**The editor's `check` re-armed a saved alarm.** The operator found that opening a disarmed
alarm and saving it switched it on. `f4d6856` keeps a saved alarm's switch and arms only a
new one. That `alarmd` is 666640 bytes, sha256
`42f18b9ab81e9a1cfe8114838ce57d50422a880fc8de8ac806b9070d77ade603`, copied over `/init` the
same way: `init.rom` unchanged, `misc` zeroed, the RTC 1795705309 against 1795705314, and
`adb reboot` at 15:01:54 UTC. Its boot loaded the settings at 13.62 s, and at 19.82 s a tap
logged `[10:26 MTWTFSS off, 12:08 MTWTF-- on, 15:53 MTWTFSS off]`, the three alarms as the
first boot had left them. The operator reported that a disarmed alarm saved from the editor
stayed off and that the screen worked as intended.

**Not read off the phone:** a one-off ringing and being disarmed, an alarm on a single
weekday other than that day, and two armed alarms surviving a reboot together in one log.
The operator chose to close the issue on the readings above; the host covers those three in
`two_alarms_due_at_once_ring_once_and_each_is_rearmed_or_disarmed` and
`every_saved_alarm_is_restored_with_its_days_and_switch_and_the_soonest_rings_next`. No
photos were taken. The phone was left in TWRP with `f4d6856`'s image on `/init`.

## 2026-11-26 — a timer in hours, minutes and seconds, for issue 128

**`reserved4` held version-6 timers before the update.** Read from TWRP, slot 0 was
version 6, sequence 40, and slot 1 version 6, sequence 39, both with the timers `00:10` and
`01:19` in two bytes each.

**`alarmd` at `8ebc158` is 672640 bytes**, sha256
`298e0e7252f007c9971d1c4a9206e97b7262e4ac4d37334592eb80e7a4890efc`. From TWRP 3.7.0_9-0 it
was copied over `/init` on `system`, replacing issue 127's `42f18b9a…e603`. `init.rom` still
hashed `e8e2768e…78ef`, `misc`'s first 32 bytes were zeroed, and the RTC read 1795706486
against the Mac's 1795706490, before `adb reboot` at 15:21:33 UTC.

**A version-6 slot loads and is saved as version 7.** The boot loaded the settings at
13.65 s. `00:00:10` played at 135.58 s, 15:23:45 UTC, logged `running until 1795706635`,
was paused at `00:08` and ended by the X at 138.15 s. `settings saved` came at 153.23 s,
before the recovery row's reboot: slot 1 then read version 7, sequence 41, 34 bytes, its
timers `0:00:10` and `0:01:19` in three bytes each. No timer of an hour was added in this
run, and its log wrote each timer as `MM:SS`, `timers [00:10, 01:19]`; issue 131 logs
`HH:MM:SS`.

**`alarmd` at `ebcd89d` is 672704 bytes**, sha256
`d75aef55fdb5ecd15546301eb6ba8636b2fc7a6ae6abe733d08c5dc3ab67aa46`, copied over `/init` the
same way, replacing `298e0e72…0efc`: `init.rom` unchanged, `misc` zeroed, the RTC
1795707005 against 1795707009, and `adb reboot` at 15:30:09 UTC.

**A timer set to `1:00:05` on the wheels is saved, sorted, and counts down in hours.** At
26.30 s the list logged `timers [00:00:10, 00:01:19, 01:00:05]`. `01:00:05` played at
36.01 s logged `running until 1795710646`, 3605 s on, with 48.76 s at 15:30:54 UTC; paused
at 41.26 s it logged `running paused 00:59:59`, and the X at 41.99 s `running none`.
`00:00:10` played at 48.76 s logged `running until 1795707064`, and Power at 61.25 s ended
its ring, 2.5 s after its end; sound and vibration were off, so no `Ringing` line came, as
issue 115 found. `settings saved` came at 65.96 s: slot 0 read version 7, sequence 42,
37 bytes, its timers `0:00:10`, `0:01:19` and `1:00:05`. The operator reported each step
done on the panel: the old timers listed as `HH:MM:SS`, the three wheels, the ring's time
left, and the amber screen's `00:00:10`. No photos were taken.

**Not read off the phone:** the hour wheel's wrap from 23 to 00 and the minute wheel's from
59 to 00. The host covers both in
`the_timer_wheels_step_hours_minutes_and_seconds_and_wrap_within_each`. The phone was left
in TWRP with `ebcd89d`'s image on `/init`.
