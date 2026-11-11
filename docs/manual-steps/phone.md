# The phone

**State.** The phone runs an unofficial LineageOS build for Android 9
(`0.23-p-20250919-UNOFFICIAL-anne`, `userdebug`) with the kernel `4.9.319-Abdelhay-KSU-`.
The bootloader is unlocked. The boot image lives in the partition named `kernel`
(`mmcblk0p31`); there is no `boot` partition. → ADR 0004.

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

**Open.** How an image is tried and flashed, and how the phone is rolled back, is issue 7.

The readings: [docs/device-history/phone.md](../device-history/phone.md).
