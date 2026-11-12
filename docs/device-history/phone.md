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
