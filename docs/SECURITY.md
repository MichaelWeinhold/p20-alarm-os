# Security and safety

This repository writes to a phone and is public. The risks are of two kinds: breaking the
phone, and publishing something that should not be.

## Bricking

| Risk | What holds it |
|---|---|
| Writing the wrong partition | A soft brick is accepted on this test phone and needs no approval; the partitions that would hard-brick it are never written, erased or formatted. `AGENTS.md`, *Device safety*; ADR 0005, D4. |
| Losing the way back | Every partition is backed up off-repository before its first write; `fastboot` and everything before it are never touched, so flashing a backup from fastboot restores the phone. |
| A boot that never draws | The debug initramfs (P2) comes before anything else boots; Huawei refuses a tethered `fastboot boot`, so every try writes `kernel` and the rollback is the stock image written back (ADR 0005). |
| Relocking the bootloader | Never done. A relocked bootloader with a modified image is a state that may not recover. |
| `oeminfo` / `nvme` | Hold the phone's identity and calibration. Never read into the repository, never written. |

## What must never be published

- **Anything read off the phone** — `boot.img`, the kernel, device trees, firmware, partition
  dumps. They carry vendor code that is not ours to redistribute, and a dump can carry the
  IMEI, serial numbers or personal data. `.gitignore` lists them, and `checks / secrets`
  fails the build if one is tracked.
- **Secrets** — none are needed; `.env` and key files are ignored anyway.
- **Personal identity in commits** — commits are authored with the GitHub noreply address.

GitHub secret scanning and push protection are on for this repository; they catch a token
pasted into a source line, which the `secrets` job does not.

## Licensing

The vendor kernel is GPL-2.0 and stays where it is, on the phone. When an image built here
is distributed, the kernel's corresponding source is the tree ADR 0004 names, and that is
named beside the release. The code in this repository is MIT.

## Supply chain

- Actions are pinned to commit SHAs; runners to a named Ubuntu image.
- `uv.lock` and `Cargo.lock` are committed, and CI runs `--locked`.
- A new package needs its `## Dependency:` section (ADR 0001, D7); on the phone, `std` first.
- Dependabot opens weekly bumps for uv, cargo and the actions.

## The program on the phone

`alarmd` runs as PID 1 with every privilege the kernel has. It opens no network socket in
the release image. The debug image's SSH server listens only on the USB gadget interface
and is never in a `production` image.
