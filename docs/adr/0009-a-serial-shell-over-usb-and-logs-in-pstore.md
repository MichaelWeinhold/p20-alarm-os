# 9. A serial shell over USB, and the log in pstore

- **Status:** Accepted
- **Date:** 2026-11-12
- **Issue:** #15
- **Amends:** ADR 0002's consequence on the serial console

## Context

The phone has no serial console, so a boot that fails is a black screen. P2 promises a
lifeline before anything else boots: "USB gadget networking and SSH, or `ramoops`". Issue
15 asks which, now that ADRs 0004 to 0006 have settled what it stands on:

- Our program is `/init` on `system`, which the kernel mounts read-only; there is no
  initramfs (ADR 0006).
- There is no tethered boot. Every try is a write to `system` from TWRP (ADR 0005, D1;
  ADR 0006, D3).
- The kernel's only USB Ethernet function is RNDIS, and macOS has no RNDIS driver
  (ADR 0004, D2).

The readings are in `docs/device-history/phone.md`, *a shell and a log without a
console*, and the entries for issues 6 and 17 before it.

## Decisions

### D1. The shell travels over a CDC-ACM serial gadget

The debug `/init` mounts `configfs` on `/config` and builds a gadget with one `acm`
function on UDC `hisi-usb-otg`. The phone's end is `/dev/ttyGS0`; the Mac's is
`/dev/cu.usbmodem*`. Both directions were read on this kernel and this Mac, with no
driver installed, from an `acm` function added beside the ROM's adb (*a CDC-ACM function
beside adb, in the ROM*). The kernel has `USB_CONFIGFS_ACM` built in, so it costs no
rebuild and no package on either side.

**That a gadget with `acm` alone enumerates under our `/init` the same way is assumed**:
in the ROM it sat beside `ffs.adb`, on the ROM's descriptors. Issue 9 proves it.

### D2. The shell is the ROM's `mksh`, and exiting it reboots into TWRP

The debug `/init` stays PID 1. It mounts `devtmpfs`, `proc`, `sysfs`, `pstore` and a
`tmpfs` on `/mnt`, then starts `/system/bin/sh` as a child with `/dev/ttyGS0` as its
descriptors 0 to 2 and `PATH=/system/bin`. `toybox` there gives `ls`, `cat`, `dmesg`,
`dd`, `base64` and the rest. When the shell exits, `/init` reboots into recovery; a
shell that dies is started again.

These are the ROM's files, already on `system` and never copied into the repository or
into an image of ours. Run in a `chroot` of `system` from TWRP, with no Android init and
no property area, `sh`, `toybox` and `adbd --version` all ran (*the ROM's shell without
Android*). No package comes in, so ADR 0001, D7 is not engaged, and nothing GPL is
distributed (`docs/SECURITY.md`, *Licensing*).

`gdbserver64` and `strace` are on `system` too. A second `acm` port carries
`gdbserver64`, should a source-level debugger be wanted; it is not built until then.

### D3. The log is the kernel log, read back from pstore, and from `dfx` after an early panic

`/init` writes each line it logs to `/dev/kmsg`, prefixed `p20-alarm-os:`. It needs no
transport to be read later:

| What ended the boot | Where its log is | Read from |
|---|---|---|
| a reboot, or a panic once the boot counted as good | pstore: `console-ramoops-0`, and `dmesg-ramoops-0` for a panic | the debug shell's `/sys/fs/pstore`, or TWRP's |
| a panic in `/init`, which starts eRecovery (ADR 0005, D5) | `dfx`, `mmcblk0p41`; reaching TWRP with the keys empties pstore | TWRP, `dd if=/dev/block/by-name/dfx` |
| a hang | nothing yet | the keys, TWRP, and `dfx` |

pstore has 512 KiB for the console, 128 KiB for `pmsg` and three 128 KiB oops records. A
sysrq panic in the ROM came back by itself in 39 s with all three kept and no eRecovery
(*a panic after boot*). `dfx` is only ever read. `pmsg` survives too, and stays unused
until a log of ours outgrows the kernel's.

### D4. The kernel needs no rebuild for it

| Candidate | The running kernel |
|---|---|
| a serial gadget | `USB_CONFIGFS_ACM=y`, `USB_F_ACM=y` |
| `adbd` over FunctionFS | `USB_CONFIGFS_F_FS=y` |
| pstore, with oops dumps and `pmsg` | `PSTORE_RAM=y`, `PSTORE_CONSOLE=y`, `PSTORE_PMSG=y`, `dump_oops=1` |
| a panic on demand | `MAGIC_SYSRQ=y`, enabled by default |
| `netconsole` | **off** |
| the console on the panel | **off**: `VT` is off, so there is no `FRAMEBUFFER_CONSOLE` |
| USB Ethernet the Mac drives | **off**: no `NCM`, `ECM` or `EEM` function |

Every candidate this ADR picks is in. ADR 0004, D4's conditions for a rebuild stand.

### D5. The loop is a host script on the serial port, under the phone's rules

A host script in `scripts/` opens `/dev/cu.usbmodem*` raw with the standard library's
`termios`, sends one command with an end marker, reads to the marker or a timeout, and
prints what came back with the shell's exit status. An agent runs it like any other
command, so a reading enters the session as its output.

- **An agent runs anything over it without asking**, as `AGENTS.md`, *Device safety*,
  allows for `adb`. The shell is root, so the hard-brick list holds for every `dd` and
  every write under `/dev/block`.
- **A binary for a try goes over the link**, `base64` into the `tmpfs` on `/mnt`, and
  runs there. Only a new `/init` needs TWRP's round trip.
- **The ROM's gadget is never changed from the ROM.** Android's init owns it there, and a
  changed one left the phone off USB until it was restarted from fastboot.

### D6. The lifeline is its own binary, and never on `system` in `production`

The debug `/init` is its own crate, built `#![no_main]` like every `/init` (ADR 0006,
D2). `alarmd` holds no gadget code and starts no shell, so a release image opens no
link. The promotion checklist gains a line: `/init` on `system` hashes to the `alarmd`
built from the commit being promoted. How the shell rides beside `alarmd` once P3 needs
both is decided then.

## Rejected alternatives

**SSH or telnet over RNDIS.** The kernel has RNDIS, but macOS does not drive it, and the
third-party driver for it is believed not to load on Apple Silicon. SSH also brings in
Dropbear, a package and a GPL binary. *Reopens if* a Linux host does the debugging.

**SSH over NCM or ECM.** macOS drives both, but the kernel has neither function, and a
rebuild waits on ADR 0004, D4. *Reopens if* the kernel is rebuilt for another reason, or
the serial link turns out too slow for the loop.

**`adbd` over FunctionFS.** The ROM's `adbd` starts without Android, and `adb push` and
`pull` would beat `base64` over serial. But it expects Android's init to bind the gadget
once it sets `sys.usb.ffs.ready`, and by Android 9's source it drops root unless
`ro.secure` is 0, a property nothing would set. Serving was not tried. *Reopens if*
moving files over the serial link holds the loop up.

**`netconsole`.** Not in the kernel, and it needs a network link the Mac can use.
*Reopens with* NCM or ECM above.

**The kernel console on the panel.** `VT` is off, so there is no `fbcon`, and the panel
is not lit under our `/init` until the display adapter takes it over (issue 37). *Reopens
if* the kernel is rebuilt with `VT`.

**A hardware UART on the board's test points.** The command line names `console=ttyS0`
and the kernel has the PL011 console, but it means opening the phone, finding pads, and a
1.8 V adapter. *Reopens if* a boot dies before `/init` and leaves nothing in pstore or
`dfx`.

**A static BusyBox, or a shell of our own in Rust.** The ROM's `mksh` and `toybox` do
the job, and nothing new is built or fetched. *Reopens when* the ROM's files leave
`system` (ADR 0006, D4).

## Consequences

- **Issue 9 is rescoped**: a debug `/init` that serves the ROM's shell on a serial gadget
  and reboots into TWRP when it exits. It proves D1's assumption.
- **A host script for the loop** is issue 42, after issue 9.
- **`docs/SECURITY.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md` and `README.md`** say
  serial shell where they said SSH, in this change.
- **The flashing skill's step 5** reads an image of ours over the shell, as part of
  issue 42.
