# 17. Put the timer in a ring, label the settings, and scroll the lists

- **Status:** Accepted
- **Date:** 2026-11-27
- **Issue:** #143

## Context

Issue 127 gave the alarm screen a hero element, a hierarchy within each row, and a list
that opens a row on a tap. Issue 143 asks why the timer list and the settings screen fall
behind it. It also asks that the alarm and timer lists scroll like settings, so they can
hold more than their five alarms and four timers.

Every screen was rendered at the panel's 1080×2280 through `face::render` at `70fce6f`.
The proposals were drawn by a copy of `face` outside the repository and rendered beside
them. The findings are in issue 143's *Scope*. In short:

- **The timer list has no hero.** The picker and the saved rows have the same size and
  weight. The content fills the top 40 %. Every length reads `HH:MM:SS`, and every row
  carries a `✕`.
- **A timer cannot be started without saving it**, so with four saved no new length
  can run at all. A tap on a saved row's figures does nothing.
- **Settings are icons without words.** The rows start 35 % down the panel, and the
  values `00:10` and `01:00` are drawn at the editor's full figure size. Nothing is
  grouped, recovery sits among the others, and the battery is not shown.
- **The sound list pages behind a `⌄`.**
- **Scrolling costs no record change.** ADR 0011 counts alarms and timers in one byte
  each, at 4 and 3 bytes apiece, in a 4 068-byte payload.

What bounds the answer:

- **ADR 0016's subsets.** Every letter is in Inter's subset already. Every icon below is
  one of the 22 already in the Material Symbols subset, and a circle is drawn with
  `glyph::ring`.
- **The 33 ms frame.**
- **A pure `render`.**
- **Three input kinds:** a tap, a vertical drag and three keys.
- **Timers have no names.**

## Decisions

### D1. The timer tab's picker sits in a ring at the alarm clock's place

The ring has the alarm ring's position and size (`board`), drawn in `DIM`. The picker's
`HH:MM:SS` sits inside it at three quarters of the board's unit, with its wheels' chevrons
above and below each pair. Two badges sit on the ring's lower arc, each a circle drawn
with `glyph::ring` and an icon inside it:

- **Lower right: `▷`.** It starts the picked length. It is `ACCENT` unless the length is
  zero.
- **Lower left: save or delete.**
  - **`+`** when the picked length is not saved. It is `ACCENT` while the list has room
    and the length is not zero, and `DIM` otherwise.
  - **The bin** when the picked length is saved. D3 covers it.

The saved lengths run as rows below the ring, at `board`'s row pitch, as the alarms do
below theirs. A vertical drag on a pair still turns its wheel (`ringer::Grab`).

### D2. `▷` on the ring runs a length without saving it; a full list only stops a save

Starting and saving are separate. `▷` on the ring starts a `Running` from the picker's
length, whether or not it is saved. A full list dims `+` and refuses the save, but never
stops a start. The running stage does not change.

### D3. A tap on a saved row loads its length into the ring; deleting lives on the ring

- **A row is its length and `▷`.** The row's `▷` starts that length at once, as
  `Play(index)` does today. No row carries a `✕`.
- **A tap anywhere else on the row loads its length into the picker.** The ring's lower
  left then shows the bin.
- **Deleting takes two taps on the bin**, as a row's `✕` does today:
  - the first tap marks it, and it turns `WARNING`;
  - the second tap removes the length;
  - any other touch clears the mark.

### D4. A length is shown as `M:SS`, or `H:MM:SS` from an hour, except on the picker

- **The picker keeps `HH:MM:SS`**, because each wheel needs its column. The leading
  zero pairs are drawn `DIM`, so `00:25:00` reads as 25 minutes.
- **Saved rows** draw the short form at `board.figure()`, right-aligned on the width of
  `0:00:00`, so the colons line up.
- **The running countdown** draws the short form larger than today, scaled to the ring:
  - `30/46` of the dial's unit under an hour;
  - `22/46` from an hour.

  The length it counts down from sits below it in `DIM`, at half the date's height.
- **The ringing halo** shows the length in the short form.

### D5. Settings are labelled rows in three groups, starting near the top

The rows start three units below the top, under the battery (D6). They are in three
groups, each under a small `DIM` header:

| Group | Row | Detail (`DIM`, second line) | Control |
|---|---|---|---|
| Alarm | Sound | the sound's name | switch |
| | Vibration | | switch |
| | Fade in | | value |
| Screen | Stays lit | | value |
| Power | Power off when idle | until the next alarm | switch |
| | Restart to recovery | | `↻` |

How a row is drawn:

- **Layout.** The icon leads the row at four units. The label follows at the date's
  height, and the control sits at the right edge.
- **Values are words**, right-aligned at the label's height: `10 s`, `30 s`, `1 min`,
  `2 min`, `5 min`, and for fade in also `15 s` and `Off`. A tap cycles them as today.
- **The Sound row opens the sound list** on a tap of its label. Its switch turns the
  speaker on and off.
- **Recovery is last and `DIM`.** Its first tap marks it, and the row then reads `Tap
  again to restart` in `WARNING`.

The six rows fit the panel without scrolling. Issue 139's scroll stays for smaller sizes.

### D6. Settings shows the battery

The status sits at the top right, as on the other two tabs. `drawn` stops leaving it out
on `Screen::Settings`.

### D7. The sound list scrolls instead of paging

- **The header row is `✕` and the word `Sound`.** It does not scroll.
- **Below it, every sound is a labelled row**, with `✓` on the chosen one.
- **The list scrolls with D8's gesture.**

`Listing::more`, `SOUNDS_SHOWN` and the `⌄` go away. `Listing` carries every sound, and
only the rows on screen are drawn.

### D8. The alarm and timer lists scroll under a fixed ring, up to 12 rows each

- **Only the rows scroll.**
  - The clock ring with its `+`, and the timer ring with its badges, stay put.
  - Rows are clipped one unit above `list_top` and at the tab bar.
  - The row cut off at the bottom edge is the hint that more follow.
- **A scroll starts only on a touch in the list's region.**
  - A touch on the ring keeps its meaning: the `+`, or a wheel's `Grab`.
  - In the list, the touch is a `Swipe`, as on settings. A row's tap waits for the lift
    and is dropped once the finger has moved half a row.
- **The cap is 12 alarms and 12 timers.** `SAVED_ALARMS` and `SAVED_TIMERS` become 12.
  - That is two screens of rows below the ring.
  - The record is unchanged, and `VERSION` stays 9.
  - Only the rows on screen are drawn, so a frame costs what it does today.
- **`reach` is computed per screen** from the list's length.
- **The scroll returns to 0** when the panel goes dark or the tab changes, as it does
  on settings today.
- **After a save, the scroll brings the saved row into view.**

## Rejected alternatives

**Names or labels on timers.** A timer is only its length. *Does not reopen:* this is
the operator's standing rule.

**The whole screen scrolls, ring and all.**
- The ring stops being a hero.
- On the timer tab, a drag on the ring would need to be told apart from a wheel's.

*Reopens if* a list needs more than about six rows visible at once.

**Auto-saved recent lengths, with the oldest evicted.** This is the other answer to a
full list. It needs a last-used stamp per timer, which changes ADR 0011's record. *Reopens
if* the operator wants recents and the record changes version for another reason.

**The picker as its own screen**, opened by `+` like the alarm editor. It adds a step to
the tab's most common action, starting a length. *Reopens if* the wheels, at three
quarters of the board's unit, are too small to drag reliably on the phone.

**`▷` and `✕` placed symmetrically when paused.** It was rendered. It moves `▷` away from
where `⏸` was. Today a pause and a resume are the same spot tapped twice. *Reopens if* the off-centre pair reads as a mistake on the phone.

**Paging the lists**, as the sound list does today. Issue 143 found its `⌄` not
discoverable. *Reopens if* a drag proves unreliable on the panel.

**No cap, up to the record's 255.** A list of that length cannot practically be dragged
through, and nothing asks for it. *Reopens if* 12 fill up in use.

**Delete by long press or by swiping a row sideways.** Both need a new input kind.
*Reopens if* a long press or a horizontal swipe arrives for another reason.

**A scrollbar.** It is a thin mark with no touch target, and the cut-off row already
says the list goes on. *Reopens if* a full list on the phone hides that it scrolls.

## Consequences

- **No new glyphs.** ADR 0016 D2 and D3 need no amendment, and `subset_fonts.py` does
  not rerun. The circles are `glyph::ring`.
- **`ringer` gains a start from the picker's length.** `Target::Entry` on the timer tab
  loads a length, and the delete moves from the rows to the ring.
- **The fixtures of the timer, settings and sound-list screens are regenerated.** So
  are the alarm screen's fixtures wherever a list scrolls.
- **An image rolled back to one built before the cap rises** truncates the lists to 5
  and 4 on its first save. The rows past those are lost.
- **The follow-ups are filed under this issue:**
  - the short length format (D4);
  - the timer ring (D1 to D3);
  - the scrolled lists and the cap (D8), after the ring;
  - labelled settings with the battery (D5, D6);
  - the scrolled sound list (D7), after the settings.
- `ARCHITECTURE.md`'s row for `face` changes with each follow-up, not with this ADR.
