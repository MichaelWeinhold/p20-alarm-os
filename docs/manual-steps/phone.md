# The phone

**State.** The phone runs an unofficial LineageOS build for Android 9
(`0.23-p-20250919-UNOFFICIAL-anne`, `userdebug`) with the kernel `4.9.319-Abdelhay-KSU-`.
The bootloader is unlocked. The boot image lives in the partition named `kernel`
(`mmcblk0p31`), and stays stock; there is no `boot` partition. Our `/init` goes on
`system` (`mmcblk0p52`), from TWRP. → ADR 0004 and 0006.

The stock image of `kernel` is backed up off the repository, at
`~/p20-backups/kernel-mmcblk0p31-stock.img`, with the sha256 below.

**Reproduce.** On the phone: *Developer options → USB debugging* on, and the Mac's
accessory prompt and the phone's debugging prompt both allowed.

```bash
adb root
mkdir -p ~/p20-backups
adb exec-out 'dd if=/dev/block/mmcblk0p31 bs=1048576 2>/dev/null' \
  > ~/p20-backups/kernel-mmcblk0p31-stock.img
```

⚠️ `adb shell su` fails: KernelSU grants `su` only to apps; `adb root` works on this
build.

⚠️ The `dd` reads the partition; never swap `if=` and `of=`.

**Verify.**

```bash
adb shell sha256sum /dev/block/mmcblk0p31
shasum -a 256 ~/p20-backups/kernel-mmcblk0p31-stock.img
# both: fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
```

**Undo.** `adb unroot`.

**Fastboot.** The bootloader's *Fastboot & Rescue mode* is reached from the ROM:

```bash
adb reboot bootloader
fastboot devices
```

⚠️ The Mac gets answers from only one `fastboot` invocation per USB enumeration, and a
second one hangs; re-enumerate the phone before every invocation, with `brew install
libusb` done once:

```bash
uv run python scripts/device_preflight.py --reset
timeout 10 fastboot oem lock-state info
```

⚠️ Several commands chained in one invocation all answer, as in `fastboot getvar
max-download-size getvar max-download-size`; `fastboot oem` sends every word after it as
one command.

⚠️ `getvar all`, `getvar product` and `getvar unlocked` answer `invalid command`; `oem
get-product-model`, `oem get-build-number`, `oem lock-state info` and `getvar
max-download-size` answer.

⚠️ `fastboot boot` answers `invalid command`: nothing is tried without writing it.
→ ADR 0005, D1.

**Fastboot without adb.** With the cable in, hold Power until the screen goes black, then
hold Volume Down until *Fastboot & Rescue mode* shows.

**Flash.** Every write, a rollback included, follows the `flashing-the-phone` skill. The
preflight checks the model, both lock states, the backup and the image's size, prints the
image's sha256, and ends on a USB reset; compare the sha256 with the one just built, then
write:

```bash
uv run python scripts/device_preflight.py <image>
timeout 120 fastboot flash kernel <image> reboot
```

⚠️ Write only after the preflight exits 0, with no other `fastboot` invocation between
them.

⚠️ `Warning: skip copying kernel image avb footer (kernel partition size: 0 …)` is
expected: the bootloader reports no partition size, so the host checks the size itself.

**Roll back.** The same write, with the backup:

```bash
uv run python scripts/device_preflight.py ~/p20-backups/kernel-mmcblk0p31-stock.img
timeout 120 fastboot flash kernel ~/p20-backups/kernel-mmcblk0p31-stock.img reboot
adb wait-for-device && adb root
adb shell sha256sum /dev/block/mmcblk0p31
# both: fe6e372eaf56b4358905443e3c475bde05851d7acd1cadbce395a376b4986a84
```

**TWRP**, the way back when fastboot cannot be reached: hold Power until black, then
Volume Up and Power together; it works from eRecovery too. `adb devices` lists it as
`recovery`, with a root shell; `adb reboot` returns to the ROM.

⚠️ From TWRP, write no partition on ADR 0005, D4's hard-brick list. → ADR 0005, D3 and D4.

⚠️ `adb reboot recovery` from the ROM leaves `boot-recovery` in `misc`'s bootloader control
block, and every boot then starts TWRP. Clear its 32-byte command field from TWRP; `misc`
(`mmcblk0p20`) is backed up at `~/p20-backups/misc-mmcblk0p20-boot-recovery.img`:

```bash
adb shell 'dd if=/dev/zero of=/dev/block/by-name/misc bs=32 count=1'
adb reboot
```

**The image's ramdisk is never loaded.** The bootloader reads only the kernel from
`kernel`, and the ramdisk from a partition named `ramdisk`, which this phone does not have;
recovery mode loads one, from `recovery_ramdisk`. → ADR 0006.

**Our `/init` on `system`.** From TWRP; `system` (`mmcblk0p52`) is backed up at
`~/p20-backups/system-mmcblk0p52-stock.img`, 3909091328 bytes, sha256
`4d61f9e3a12ddcba2fdd425b5661510aa604c254ae5d33353d2ce5c525412047`. → ADR 0006, D2 and D3.

```bash
shasum -a 256 <binary>
adb push <binary> /tmp/init
adb shell 'mkdir -p /mnt/sys && mount -o rw /dev/block/mmcblk0p52 /mnt/sys'
adb shell 'mv /mnt/sys/init /mnt/sys/init.rom && cp /tmp/init /mnt/sys/init && chmod 755 /mnt/sys/init'
adb shell 'sha256sum /mnt/sys/init /mnt/sys/init.rom && umount /mnt/sys'
adb shell 'dd if=/dev/zero of=/dev/block/by-name/misc bs=32 count=1'
adb reboot
```

⚠️ `/mnt/sys/init.rom` must hash to
`e8e2768ea78c453caf783281cfe835b95fc4f72f73928775d4f30609761978ef` before the reboot.

⚠️ TWRP's `/tmp` is emptied by every reboot; push the binary again in each session.

⚠️ `/init` starts with file descriptors 0 to 2 closed and no `/dev/null`. A binary built
with Rust's start-up aborts before `main`, the kernel panics, and the next boot starts
eRecovery. Choose nothing there; reach TWRP with the keys.

**Read it.** Back in TWRP, the kernel log of the boot before is in pstore; after a cold
boot, Huawei's `dfx` partition still holds it:

```bash
adb shell 'grep p20-alarm-os /sys/fs/pstore/console-ramoops-0'
adb exec-out 'dd if=/dev/block/by-name/dfx 2>/dev/null' > <scratch>/dfx.bin
strings -n 6 <scratch>/dfx.bin | grep -E 'jump to execute /init|Kernel panic|p20-alarm-os'
```

**Undo.**

```bash
adb shell 'mount -o rw /dev/block/mmcblk0p52 /mnt/sys && rm /mnt/sys/init && mv /mnt/sys/init.rom /mnt/sys/init'
adb shell 'sha256sum /mnt/sys/init && umount /mnt/sys'
adb reboot
# e8e2768ea78c453caf783281cfe835b95fc4f72f73928775d4f30609761978ef
```

**The power probe**, `crates/init-probe`, swapped in as above. It stops a charge at 80 %
through the battery adapter, writes `enable_charger` 1 in place of a plug-in, stops it
again, writes 1 back, logs the battery and charger nodes after each step, and reboots
into TWRP after about 20 s. → ADR 0008, D3.

```bash
cargo build --release --locked -p init-probe --target aarch64-unknown-linux-musl
adb shell 'grep -a -E "p20-alarm-os: (power|charge)" /sys/fs/pstore/console-ramoops-0'
```

⚠️ `status` can lag a write to `enable_charger` by more than 5 s; read `enable_charger`
and `current_now` beside it.

⚠️ The kernel log ends a record at a NUL, so a node read into it is stripped of control
characters first; a line that stops short means one was not.

⚠️ `/proc/device-tree` is readable only as root, so read the charging limits from TWRP:
`tr '\0' '\n' < /proc/device-tree/SCUDATL_2900_39k_4400/temp_para`.

**The boot-fail monitor.** One `/init` that panicked the kernel was enough for the next
boot to start eRecovery. → ADR 0005, D5.

The readings: [docs/device-history/phone.md](../device-history/phone.md).
