# 16. Rasterise light type and icons on the phone

- **Status:** Accepted; D2 amended for the figure height by issue 104, D3 for the battery
  glyph by issue 105, for the recovery row by issue 118 and for the volume by issue 110
- **Date:** 2026-11-23
- **Issue:** #95

## Context

`alarm_core::face` draws every digit as seven filled segments, and every icon as
rectangles or character masks, on whole multiples of a layout unit with no
anti-aliasing. On the phone the operator asked for icons and timers that are less bold,
with a modern size and type, and less pixelated. Issue 95 asks how glyphs get into the
image, which typeface under which licence, what grey levels cost per frame on the
Kirin 659, how the icons are drawn, what each option costs in binary size, and what the
host fixtures become.

The facts it starts from, at `95127fe`:

- **`render(size, face)` lays out at any size.** The unit is
  `width * 9 / 10 / 21`, 46 px on the 1080×2280 panel and 4 px on the 108×228 fixtures.
  The clock, the alarm, the status line and the tab bar each scale off it.
- **`Frame` is a `Vec<Colour>`** that is only ever filled with whole rectangles. The
  `framebuffer` adapter takes nothing but 32 bpp XRGB8888 and page-flips.
- **`alarmd` is 535 696 bytes**, built for release, `opt-level = "s"`, LTO, stripped.
- **The nine fixtures under `crates/alarm-core/testdata/`** map each pixel to one
  character by its exact colour.

A prototype outside the repository, built with the same release profile for
`aarch64-unknown-linux-musl`, drew `19:37`, `06:45`, the three tab icons and the battery
onto a 1080×2280 frame:

| Variant | Stripped binary | Over no font |
|---|---|---|
| no font | 344 280 bytes | — |
| `ttf-parser` and `ab_glyph_rasterizer`, both fonts embedded | 403 008 bytes | +58 728 |
| `fontdue`, both fonts embedded | 426 496 bytes | +82 216 |

13 221 of the frame's 110 725 lit pixels came out grey, the same on the Mac and on the
phone. The phone's reading, from TWRP, is in `docs/device-history/phone.md`, *glyph
rasterising on the Kirin 659*:

- **The glyphs cost 12.9 ms per frame** on cpu0 at 1709 MHz, over a 1.65 ms fill.
  `fontdue` costs 0.6 ms more.
- **1709 MHz is what `alarmd` runs at.** The kernel's default governor is `performance`,
  and `alarmd` leaves it (ADR 0007). Forced to `powersave`, cpu0 at 480 MHz took 41.3 ms.
- **Rasterising is two thirds of it**: 26 ns per pixel of a glyph's box, and blending
  13 ns.

## Decisions

### D1. `face` rasterises the glyphs at the unit it lays out with

Fonts are compiled into `alarmd` with `include_bytes!`, and `face` rasterises each glyph
from its outline at the size the layout asks for. `ttf-parser`, with default features
off and `std` on, reads the outline. `ab_glyph_rasterizer` turns it into coverage. Both
are pure computation with no device behind them, so they live in `alarm-core`
(architecture rule 1). No adapter changes (architecture rule 6).

**Rasterising on the phone keeps `render` working at any size.** Bitmaps rendered on the
host would have to be committed for every unit the layout derives: the panel's three
text sizes alone come to 1.04 MB of raw alpha, twice today's binary. The fixtures' sizes
come on top of that.

The two crates come in by ADR 0001, D7, each with its section in the follow-up's pull
request:

| | `ttf-parser` 0.25.1 | `ab_glyph_rasterizer` 0.1.10 |
|---|---|---|
| Replaces | the seven-segment table and the character masks | `fill`'s rectangles, for glyphs |
| Publisher | the HarfBuzz organisation, Yevhenii Reizner its author | Alex Butler, `ab_glyph`'s author |
| Licence | MIT or Apache-2.0 | Apache-2.0 |
| Pulls in | nothing, with default features off | nothing, with `std` on |

### D2. The type is Inter at weight 300, subset to the figures `face` draws

The variable `InterVariable.ttf` from Inter 4.1 is instanced at `wght=300` and `opsz=32`,
then subset to `0`–`9`, `:`, `%` and `-` with no hinting and no layout features. The
result is 2 792 bytes. Inter is under the SIL Open Font License 1.1 and reserves no font
name, so the instanced subset keeps its name. It is committed with `OFL.txt` beside it.

A figure's height is the digit height the layout already gives, `DIGIT_HEIGHT_UNITS`
units. Inter's default figures are proportional, `1` at 703 units of advance and `0` at
1217. So each glyph is centred in the `DIGIT_UNITS`-wide cell it fills today, and the
time does not shift as it changes.

**Amended by issue 104**: at `DIGIT_HEIGHT_UNITS`, seven units, `4` is 5.29 units wide and
`0` 4.95, past the five-unit pitch, so `04` overlaps. A figure is six units tall,
centred in the seven-unit cell, which leaves `44` a 0.46-unit gap; the cells and `target`
do not change. `:` and `-` are centred on the figures' middle, as Inter's case forms
would place them with the layout features dropped.

### D3. The icons are Material Symbols Outlined, drawn the same way

The variable Material Symbols Outlined font is instanced at `FILL=0`, `GRAD=0`, `opsz=48`
and `wght=300`, so its strokes match the type's weight. It is subset to fourteen icons:

- `alarm`, `hourglass_empty`, `settings`, `battery_full`;
- `power_settings_new`, `light_mode`, `add`, `close`;
- `keyboard_arrow_up`, `keyboard_arrow_down`, `play_arrow`, `stop`;
- `toggle_on`, `toggle_off`.

The result is 2 904 bytes, under Apache-2.0, committed with its `LICENSE` beside it. Every
icon goes through D1's rasteriser, so the icons take no code of their own.

**The battery's bars and its red stay drawn by `face`**, inside the icon's outline, since
the font has no glyph for a one-, two- or three-bar gauge in a warning colour.

**Amended by issue 105**: `battery_full` is solid at `FILL=0`, so no bar shows inside it;
`battery_android_0`, an empty horizontal outline, replaces it, and the subset is 3 000
bytes. Each icon's em square fills the box `face` gives it, so every stroke is the same
45/960 of its em. The status line is an eighth of the digit's unit, half what it was, so
the battery is about 61 by 32 pixels on the panel, the size of a phone's own.

**Amended by issue 118**: `build` and `restart_alt` join the subset for the settings
screen's recovery row, sixteen icons in 3 336 bytes.

**Amended by issue 110**: `volume_up` joins the subset for the volume panel, seventeen
icons in 3 468 bytes.

### D4. The subsets are made by a host script, `scripts/subset_fonts.py`

It instances and subsets with `fontTools`, a dev dependency by ADR 0001, D7, from the
upstream files and the glyph lists above. A test asserts that each committed subset maps
exactly the code points `face` draws. Adding an icon is one line in the script and a
rerun.

### D5. A glyph is blended over the pixel already in the frame

For each pixel with coverage `a`, `face` writes `(glyph·a + under·(255 − a)) / 255` per
channel, in integers, where `under` is the colour already at that pixel. Most glyphs sit
on a solid background, but the knob sits on the switch, so the blend reads the frame
rather than assuming `BACKGROUND`. A pixel with no coverage is skipped, and one with full
coverage takes the glyph's colour without arithmetic. `Frame` stays a
`Vec<Colour>` of XRGB, and the `framebuffer` adapter does not change.

### D6. The fixtures map each pixel to its nearest palette colour

`picture` maps each pixel to the character of the palette colour nearest to it, by
squared distance over the three channels, rather than by exact colour. The fixtures stay
pictures in characters, a diff still reads as a picture, and an anti-aliased edge falls
to the side it is mostly on. One test asserts that a rendered clock has grey pixels, so
the anti-aliasing cannot silently stop. All nine fixtures are regenerated in the
follow-up that changes the glyphs.

### D7. `render` rasterises every frame, with no glyph cache

The fullest screen, the alarm screen with four saved alarms, puts about 680 000 pixels
of glyph boxes on the panel: the clock, the active alarm, the wheels, the list and the
icons. By the reading's 39 ns per pixel that is about 28 ms per frame, and a drag on a
wheel redraws once per step. That is fast enough to follow a finger, and `render` stays
a function of its size and its `Face`, with nothing kept between calls.

A cache would hold each glyph's coverage per size, and leave blending alone, about 10 ms.
It means state that `render` does not have today, owned by `alarmd` and passed in.

## Rejected alternatives

**Glyphs rendered on the host and committed as alpha bitmaps.** 1.04 MB raw for the
panel's text sizes, before the icons and the fixtures' sizes, and `render` would stop
working at any other size. *Reopens if* `render` is fixed to one panel size and the
bitmaps, run-length coded, come in under D1's 58 728 bytes.

**`fontdue`.** It rasterises as well, but costs 23 488 bytes more than D1's pair and
pulls in `hashbrown` for its glyph cache, which `face` does not need. *Reopens if* a
string of text needs layout and kerning, which `face` does not draw.

**A rasteriser of our own**, over outlines exported to Rust by the host script. It drops
the two crates for some 300 lines of curve flattening and coverage accumulation, which
`ab_glyph_rasterizer` already is, an 11 KB crate. *Reopens if* either crate stops
being maintained or its licence changes.

**`tiny-skia` or `zeno`**, full path renderers. They draw the shapes `face` needs, but
bring stroking, transforms and paints it does not. *Reopens if* `face` needs strokes
drawn as paths rather than glyphs, such as a dial.

**Seven segments drawn thinner.** Issue 80 shipped this as the stopgap. The strokes are
still whole pixels on a unit grid, so the slanted strokes and the icons stay stepped.
*Reopens if* a cached frame still lags a drag on the phone.

**A glyph cache from the start.** D7's estimate leaves a drag fast enough without one.
*Reopens if* the type's follow-up reads the alarm screen above 33 ms a frame on the
phone, or a drag visibly lags.

**Another typeface.** Roboto Flex and IBM Plex Sans are also under the OFL and come in
light weights, but neither was rendered here. Inter was chosen for its tall, open figures
at weight 300. *Reopens if* the operator prefers another face on the phone; D4's script
then takes another upstream file.

**An icon set drawn as SVG strokes**, such as Lucide. It needs an SVG path parser on the
phone, or a converter on the host, for a set Material Symbols already covers as a font.
*Reopens if* an icon `face` needs is missing from Material Symbols.

## Consequences

- **The type is a follow-up under this issue**, issue 104: D1, D2, D4, D5, D6 and D7
  for the digits, the colon, the dashes and `%`, with the fixtures regenerated. It reads
  the alarm screen's time per frame on the phone, which checks D7's estimate.
- **The icons are a second follow-up**, issue 105, waiting for the first: D3 for the tab
  bar, the battery, the power symbol, the sun, the switch, the wheels' arrows, `+`, the
  cross, play and stop.
- **`alarmd` grows by about 59 KB**, about 11 %.
- `ARCHITECTURE.md`'s row for `face` changes with each follow-up, not with this ADR.
