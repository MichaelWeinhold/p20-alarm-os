# 18. Name the project for the bedside clock, and lead the README with its screens

- **Status:** Accepted; D4 amended with a shorter README by issue 168
- **Date:** 2026-11-27
- **Issue:** #141

## Context

Issue 141 asks for a better name, description and README, in a state the operator can
point a job application at. The repository must not look as if it was built for that.
The findings are in issue 141's *Scope*. In short:

- **The name says one feature and too much system.** `p20-alarm-os` names the alarm, but
  the image also has a timer, settings, a battery display and a low-battery light. It
  says *OS*, but the project is one program as `/init` on the vendor kernel. ADR 0002
  chose that over building a system.
- **The name is used beyond GitHub.**
  - Every kernel-log line starts with `p20-alarm-os: ` (`pid1::kmsg::PREFIX`).
  - The debug `/init`'s USB gadget names itself `p20-alarm-os`.
  - Tests assert both strings, and `docs/manual-steps/phone.md` greps pstore for the
    prefix.
  - It is also in `Cargo.toml`'s `repository`, `pyproject.toml`'s package name, the
    README's badge and `docs/manual-steps/github.md`'s `gh` calls.
  - About ten readings in `docs/device-history/` carry it.
- **The description says alarm clock only.** Two topics are wrong: `initramfs`
  contradicts ADR 0006, and `postmarketos` was rejected by ADR 0002.
- **The README shows the process before the product.**
  - It has no picture.
  - Its first screen is a 20-row status table of decisions.
  - Two of its rows are stale: *one daily alarm*, and *the speaker is parked*.

What bounds the answer:

- **Nothing is added only to impress.** Every change also serves someone running,
  reading or maintaining the project.
- **Nothing read off the phone is committed** (`.gitignore`, `checks / secrets`). A
  photo of the phone is the operator's own picture, not a device file, but none exists
  yet.
- **`face::render` is a pure function of `Size` and `Face`.** A host render is the exact
  frame the phone draws, pixel for pixel, at the panel's 1080×2280.
- **ADR 0001, D7.** A picture needs no new package if the host tooling encodes it.

## Decisions

### D1. The project is renamed `p20-bedside-clock`

The candidates, against three tests: does it say what the thing is, does it cover more
than the alarm, and does it claim more than the project does?

| Name | Says what it is | Covers more than the alarm | Claims no more than it does |
|---|---|---|---|
| **`p20-bedside-clock`** | the phone, and what it became | yes, a clock with alarms and a timer | yes |
| `p20-bedside-os` | the phone, and that it replaces Android | yes | no: one program, not an OS |
| `nightstand` | only with the description beside it | yes | yes, but it is unsearchable and generic |
| `anne-clock` | only to someone who knows the codename | yes | yes |
| `p20-alarm-os`, kept | the phone | no | no |

**`p20-bedside-clock`** passes all three. The description (D3) says that Android is
replaced, which is the interesting part, so the name does not have to.

### D2. One name everywhere, the history excepted

The rename reaches every place the name is used:

- **On the phone.**
  - `pid1::kmsg::PREFIX` becomes `<3>p20-bedside-clock: `.
  - The gadget's `manufacturer` and `serialnumber` become `p20-bedside-clock`, so the
    Mac sees `/dev/cu.usbmodemp20_bedside_clock1`.
  - Their tests change with them.
- **Procedures.** `docs/manual-steps/phone.md` greps for `p20-(alarm-os|bedside-clock)`.
  An image built before the rename may still be on the phone or be rolled back to, so
  both names must match. ADR 0009's mention of the prefix gets an amendment line.
- **Repository.**
  - `Cargo.toml`'s `repository`.
  - `pyproject.toml`'s `p20-bedside-clock-tools`, and `uv.lock` with it.
  - `tests/test_check_dependencies.py`.
  - The `gh` calls in `docs/manual-steps/github.md`.
- **Not changed: `docs/device-history/`.** A reading is recorded as the phone printed it.
- **Outside the repository**, the operator's own step:
  - the GitHub rename, which redirects the old URL;
  - the local checkout's directory, if it is renamed. The agent client's per-project
    memory is keyed on that path and moves with it.

### D3. The description says what it is, what it does and how

> A bedside clock made from an old Huawei P20 Lite: the stock Linux kernel, and one
> static Rust program as `/init` in place of Android. Alarms, a timer, and deep sleep
> between uses.

The topics are `rust`, `embedded-linux`, `aarch64`, `musl`, `framebuffer`,
`alarm-clock`, `huawei` and `pid1`. `initramfs`, `postmarketos` and `android-device`
come off.

### D4. The README leads with the product, then how it works, then how it is worked

In this order:

1. **Title, one sentence, the screens (D5).** At most one badge: the pipeline's.
2. **What it does.** A short paragraph, not a checklist:
   - up to 12 alarms with weekdays, a fade-in and a chosen sound;
   - a timer that runs a length without saving it;
   - the panel lit only when touched;
   - a suspend, then power off until the RTC switches the phone on for the alarm;
   - the charge held at 80 %, and the LED blinking on a low battery.
3. **How it works.** The boot diagram, `alarmd` as PID 1, and `alarm-core` kept free of
   hardware.
4. **What was hard.** Brought up to date: the speaker rings after a replay of the HAL's
   DSP init (ADR 0015), debugging is blind, and the modem holds the deep-sleep vote.
5. **Running it on your own P20 Lite.** As today.
6. **How this repository is worked.** As today, shortened to its bullets and ADR 0001.
   It is part of what the project is, so it stays and is stated plainly.
7. **Documentation, Development, License.** As today.

**The status table leaves the README.** `docs/ROADMAP.md` already lists each phase,
each research and its ADR, and what is parked. The README links it in one line. The
table's two stale rows stop being a risk, because no second list of the status is kept.

**Amended by issue 168**: the operator chose a shorter README. Steps 4 and 6, the
documentation table and the line linking `docs/ROADMAP.md` are dropped, and step 2 is one
compact paragraph. What they said stays in ADR 0001, 0002, 0009, 0010, 0012 and 0015,
and in `docs/ROADMAP.md`.

### D5. The screens are host renders, kept in `docs/images/` and tied to the fixtures

- **What is drawn.** The alarm, timer and settings tabs side by side, scaled to a
  third, each with rounded corners and a thin border so it reads on GitHub's light
  theme. They are about 1224×808 px and 140 KB. Each uses the same `Face` as the
  fixture tests for that screen.
- **How it is made, with no new package.**
  - An example in `alarm-core`, `screens`, renders the three frames and composes them.
    It writes raw RGBA.
  - `scripts/render_screens.py` runs it and encodes the PNG with the standard
    library's `zlib`.
  - The same run writes a 1280×640 social preview (D6).
- **How it stays current.** The script writes `docs/images/screens.sha256` beside the
  picture. It holds the hash of each fixture the picture depends on:
  - `face-sheet.txt`;
  - `face-timer.txt`;
  - `face-settings.txt`.

  A pytest compares those hashes against the fixtures. A change that re-records one of
  them fails the checks until the picture is rendered again.
- **A photo of the phone on the nightstand** can join the renders later, as the
  operator's own picture. D5 leaves room for it under `docs/images/`.

### D6. The social preview is the screens, set by hand

GitHub's social preview is uploaded only through the repository's settings page; no API
sets it. `docs/manual-steps/github.md` records the upload of D5's 1280×640 preview as
state, with how to repeat it.

### D7. The tone stays plain

- **No** badge wall, emoji bullets, marketing words ("blazing", "powerful", "modern"),
  or a "features" checklist with ticks.
- **No section addressed to recruiters.** No "skills demonstrated", no tech-stack logos.
- The README says what is on the phone and what is not, as `docs/device-history/` does.
  It names nothing that is only decided as built.
- It keeps the voice the docs already have: short declarative sentences, the reasons
  linked to their ADR.

### D8. The follow-ups run rename, renders, then README

1. **The rename (D1 to D3).** It goes first, so the README is written once with its
   final URLs.
2. **The renders (D5).** They do not depend on the rename and can run beside it.
3. **The README and the social preview (D4, D6, D7).** Blocked by both.

## Rejected alternatives

**Keep `p20-alarm-os`.** It names one feature and says OS. *Reopens if* the rename's
on-device change proves to break a recorded procedure that the grep in D2 cannot cover.

**`p20-bedside-os`.** The project is one program on the vendor kernel. Calling it an OS
is the kind of claim D7 rules out. *Reopens if* the image grows beyond one program as
`/init`.

**A coined name such as `nightstand`.** Nobody can find it, and it needs the description
to explain it. *Reopens if* the project outgrows the P20 Lite, so the phone's name no
longer belongs in it.

**Keep the old name on the phone and rename only the repository.** The kernel log and
the gadget would carry a name that appears nowhere else. *Reopens if* a second image
lineage needs the old prefix kept.

**Photos instead of renders.** None exist, and a render is the exact frame the phone
draws. *Reopens when* the operator takes one; it is added beside the renders, not
instead of them.

**Render the picture in CI and compare it pixel for pixel.** That needs `cargo` inside
pytest, or a new job, to catch what the fixture hashes already catch. *Reopens if* the
picture drifts from the screens without a fixture changing.

**Keep the status table in the README.** It is a second list of what `ROADMAP.md`
records. Two of its rows have already gone stale. *Reopens if* `ROADMAP.md` stops
showing status per phase.

**Pillow for the PNG.** The standard library's `zlib` writes an RGBA PNG in a few lines.
A package fails ADR 0001, D7's first question. *Reopens if* the pictures need resampling
or text that the example cannot draw.

## Consequences

- **Kernel-log greps change.** Lines from images built after the rename start with
  `p20-bedside-clock: `. The manual steps match both prefixes. The recorded history keeps
  the old one.
- **The debug shell's device path changes** to `/dev/cu.usbmodemp20_bedside_clock1`
  once a debug `/init` built after the rename is on the phone.
- **Every fixture change on the three list screens renders the picture again** in the
  same PR. The pytest enforces it.
- **`docs/ROADMAP.md` becomes the only place the status is kept.** The README links it.
- **The follow-ups are filed under this issue**: the rename, the renders, and the README
  with the social preview.
