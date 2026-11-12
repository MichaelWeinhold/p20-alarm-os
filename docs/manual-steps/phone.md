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
Volume Up and Power together. `adb devices` lists it as `recovery`, with a root shell;
`adb reboot` returns to the ROM.

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

**Open.** What the boot-fail monitor counts is read when an image of ours first fails. →
ADR 0005, D5.

The readings: [docs/device-history/phone.md](../device-history/phone.md).
