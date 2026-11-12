# 6. Put our `/init` on `system`, and leave `kernel` stock

- **Status:** Accepted
- **Date:** 2026-11-12
- **Issue:** #33
- **Supersedes:** ADR 0004, D3; amends ADR 0002 and ADR 0005

## Context

ADR 0004, D3 assumed the bootloader adds `skip_initramfs` only because the stock image in
`kernel` carries no ramdisk, so an image with one would run its `/init`. Issue 17's try
falsified it on 2026-11-12: an image with our ramdisk booted the ROM, `skip_initramfs`
still on the command line and no marker in the kernel log. The readings are in
`docs/device-history/phone.md`, *an initramfs in `kernel`* and *what a normal boot runs as
`/init`*.

- **The bootloader reads only the kernel from `kernel`**, `kernel_size` bytes at the first
  page. On a normal boot it loads the ramdisk from a partition named `ramdisk`, which this
  phone does not have, gets 0 bytes, and appends `skip_initramfs`.
- **The bootloader's parameters come after the image header's**, among them
  `root=PARTUUID=<system>` and `init=/init`. For both, the kernel keeps the last value.
- **The kernel mounts `system`** (`mmcblk0p52`, 3909091328 bytes, ext4) read-only as `/`,
  with no `dm=` verity table, and runs its `/init`, which is the ROM's.
  `androidboot.veritymode=enforcing` is read only by Android's init.
- **Recovery mode does load a ramdisk**, from `recovery_ramdisk`, into the same kernel,
  with no `skip_initramfs`, and the `boot-recovery` command in `misc` sends every boot
  there until it is cleared.

## Decisions

### D1. The kernel runs our binary as `/init` on `system`

A normal power-on, cold or warm, runs whatever is at `/init` on `system`, so that is where
our binary goes. `kernel` keeps the stock image byte for byte (ADR 0004, D1); nothing of
ours is written there. No ramdisk is involved, and `skip_initramfs` is harmless.

### D2. The first try renames the ROM's `/init` and puts ours in its place

From TWRP, with `system` backed up under `~/p20-backups/` first: mount `system`, rename
`/init` to `/init.rom`, push the probe as `/init` with mode 0755, and reboot. The probe
writes its marker to `/dev/kmsg` and reboots into the bootloader; the marker is read from
`/sys/fs/pstore` in TWRP, and `/init.rom` is renamed back. The ROM's other files stay on
`system` until our image needs the room.

**Two assumptions, which the try proves or disproves.** The kernel mounts `system`
without verity, so a changed file is not refused. File descriptors 0 to 2 are the console
the kernel opens before it mounts `system`, so Rust's start-up check has nothing to
reopen. A try that shows no marker is read for either of them first.

### D3. `system` is written from TWRP, and its way back is TWRP

`system` is the only partition our image writes: the rename and the push are a write to
it, from TWRP's root shell. The way back is the rename undone, or the
backup written back with `dd` from TWRP. Fastboot, `kernel` and `recovery_ramdisk` are not
touched, so both ways back of ADR 0005 stay. ADR 0005, D4's hard-brick list does not
name `system`.

### D4. Our root is `system`, read-only, until a decision says otherwise

The kernel mounts it without `rw`, so a power cut cannot corrupt it. Whether `system`
later holds an ext4 of ours alone, and how the host builds it, is decided when the image
outgrows one binary beside the ROM's files.

## Rejected alternatives

**Our ramdisk in the image in `kernel`** — ADR 0004, D3. The bootloader never reads it.
*Reopens if* a bootloader other than `Chipset-chicago9.1.0.6(1GA4)_FASTBOOT` loads the
ramdisk of the image in `kernel`.

**Renaming `skip_initramfs` inside the kernel** — ADR 0004, D3's fallback. With 0 bytes
loaded there is nothing for the kernel to unpack. *Reopens with* the alternative above.

**A partition named `ramdisk`, added to the partition table.** It rewrites `ptable`, on
ADR 0005, D4's hard-brick list. *Reopens* never.

**`init=` or `root=` pointed elsewhere from the image header.** The bootloader's values
come after the header's and win. *Reopens if* the header's command line ever comes last.

**Our ramdisk in `recovery_ramdisk`, booted through `misc`'s `boot-recovery`.** It is the
one path that runs a ramdisk today, but it removes TWRP, the second way back, while ours
is there; it rests on `misc` being honoured on a cold power-on, which is unread; and
`recovery_vendor` may be loaded beside it. *Reopens if* the kernel refuses our `/init` on
`system`.

**An initramfs built into a rebuilt kernel** (`CONFIG_INITRAMFS_SOURCE`). Everything would
stay in `kernel`, at ADR 0004, D4's cost of a cross toolchain and a kernel nobody has
booted here. *Reopens on* D4's own conditions, or if neither `system` nor
`recovery_ramdisk` runs our `/init`.

**All of `system` replaced by a filesystem of ours, now.** It costs the ROM and a 3.6 GiB
write for a probe of one binary. *Reopens* under D4.

## Consequences

- **`AGENTS.md`'s device-safety rule changes**: `kernel` stays stock, and the image's
  `/init` goes on `system`. ADR 0002's "only `boot` is written" and its initramfs, ADR
  0004, D2's initramfs row and D4's rename condition, and ADR 0005's `/eng` consequence are
  corrected under this issue.
- **Issue 17 runs D2's try**, and the `initramfs.py` builder it carried is not needed for
  it.
- **The `flashing-the-phone` skill and the device preflight cover only `kernel`.** A write
  to `system` follows D2 and D3 until a skill covers it.
- **The ROM does not start while our `/init` is in place.** Readings that need it wait for
  the rename back; Android 9's init re-executes `/init` after loading its SELinux policy,
  so ours cannot simply hand over to it.
- **Huawei's boot-fail monitor** still runs from kernel init (ADR 0005, D5); ours never
  creates `/eng`.
