use crate::alarm::{Alarm, Weekdays};
use crate::battery::{Gauge, Level};
use crate::glyph::{self, Align, Icon, Place, Square};
use crate::ringer::Outputs;
use crate::sound::{SoundName, Volume};
use crate::time::{Countdown, SECONDS_PER_DAY, TimeOfDay, Weekday};

pub const BACKGROUND: Colour = Colour(0x00_0000);
pub const FOREGROUND: Colour = Colour(0xff_ffff);
pub const WARNING: Colour = Colour(0xff_3b30);
pub const DIM: Colour = Colour(0x63_6366);
pub const ACCENT: Gradient = Gradient {
    from: Colour(0x57_a2f3),
    to: Colour(0x6e_41d2),
};

const UNITS_ACROSS: usize = 21;
const DIGIT_UNITS: usize = 4;
const DIGIT_HEIGHT_UNITS: usize = 7;
const FIGURE_HEIGHT_UNITS: usize = 6;
const MARK_GAP_UNITS: usize = 2;
const MARK_HEIGHT_UNITS: usize = 2;
const SYMBOL_UNITS: usize = 13;
const SYMBOL_GAP_UNITS: usize = 2;
const PERCENT_UNITS: usize = 4;
const FIGURES: usize = 3;
const TAB_UNITS: usize = 9;
const TAB_MARGIN_UNITS: usize = 4;
const SIDE_UNITS: usize = 7;
const SIDE_GAP_UNITS: usize = 3;
const ROW_UNITS: usize = 8;
const SECTION_GAP_UNITS: usize = 3;
const COLON_UNITS: usize = 2 * (DIGIT_UNITS + 1);
const MINUTE_UNITS: usize = COLON_UNITS + 2;
const COUNTDOWN_UNITS: usize = 2 * MINUTE_UNITS + 2 * DIGIT_UNITS + 1;
const ARROW_UNITS: usize = 2;
const ARROW_GAP_UNITS: usize = 2;
const ICON_UNITS: usize = 7;
const RING_UNITS: usize = 21;
const CONTROL_UNITS: usize = 5;
const CONTROL_GAP_UNITS: usize = 3;
const FACE_UNITS: usize = 15;
const DATE_UNITS: usize = 2;
const LIST_UNITS: usize = UNITS_ACROSS + 2 * (SIDE_GAP_UNITS + SIDE_UNITS);
const LABEL_GAP_UNITS: usize = 2;
const DAY_UNITS: usize = 3;
const DIAGONAL_PER_MILLE: usize = 707;
const BATTERY_UNITS: usize = 14;
const ICON_EM: usize = 960;
const BARS: [usize; 3] = [150, 359, 568];
const BAR_WIDTH: usize = 164;
const BAR_TOP: usize = 350;
const BAR_HEIGHT: usize = 260;
const VOLUME_HALVES: usize = 4;
const VOLUME_ICON_HALVES: usize = 3;
const VOLUME_TRACK_HALVES: usize = 24;
const WIDEST_LENGTH: &str = "0:00:00";
const COUNTDOWN_PARTS: usize = 46;
const MINUTES_PARTS: usize = 30;
const HOURS_PARTS: usize = 22;

pub const SAVED_ALARMS: usize = 12;
pub const SAVED_TIMERS: usize = 12;
const BOARD_ROWS: usize = 5;
const SHEET_ROWS: usize = 4;

const SETTINGS: [Target; 7] = [
    Target::PowerOff,
    Target::LitTime,
    Target::Sound,
    Target::SoundList,
    Target::Vibration,
    Target::Ramp,
    Target::Recovery,
];
const NAME_UNITS: usize = 3;
pub const SOUNDS_SHOWN: usize = 6;
const TIME_FIELDS: [Field; 2] = [Field::Hours, Field::Minutes];
const COUNTDOWN_FIELDS: [Field; 3] = [Field::Hours, Field::Minutes, Field::Seconds];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gradient {
    pub from: Colour,
    pub to: Colour,
}

impl Gradient {
    fn at(self, along: usize, span: usize) -> Colour {
        let step = (along.min(span) * usize::from(u8::MAX))
            .checked_div(span)
            .unwrap_or(0);
        mixed(self.to, self.from, u8::try_from(step).unwrap_or(u8::MAX))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ink {
    Flat(Colour),
    Accent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
    Alarm,
    Timer,
    Settings,
}

impl Screen {
    pub const ALL: [Self; 3] = [Self::Alarm, Self::Timer, Self::Settings];

    fn icon(self) -> Icon {
        match self {
            Self::Alarm => Icon::Alarm,
            Self::Timer => Icon::Timer,
            Self::Settings => Icon::Settings,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Listed {
    pub alarm: Alarm,
    pub armed: bool,
    pub until: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Face {
    pub time: TimeOfDay,
    pub weekday: Weekday,
    pub day: u8,
    pub alarms: Vec<Listed>,
    pub until: Option<i64>,
    pub editing: Option<Alarm>,
    pub rang: Option<TimeOfDay>,
    pub marked: Option<usize>,
    pub ringing: bool,
    pub battery: Gauge,
    pub screen: Screen,
    pub power_off: bool,
    pub lit_seconds: i64,
    pub ramp_seconds: i64,
    pub outputs: Outputs,
    pub recovery_marked: bool,
    pub timers: Vec<Countdown>,
    pub countdown: Countdown,
    pub running: Option<Running>,
    pub rung: Option<Countdown>,
    pub volume: Option<Volume>,
    pub sound: Option<SoundName>,
    pub listing: Option<Listing>,
    pub scroll: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Listing {
    pub sounds: Vec<SoundName>,
    pub chosen: Option<usize>,
    pub more: bool,
}

impl Face {
    fn addable(&self) -> bool {
        self.alarms.len() < SAVED_ALARMS
    }

    fn timer_addable(&self) -> bool {
        self.timers.len() < SAVED_TIMERS
            && !self.countdown.is_zero()
            && !self.timers.contains(&self.countdown)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Running {
    pub left: Countdown,
    pub length: Countdown,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    List,
    Running,
    Editing,
    Choosing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Hours,
    Minutes,
    Seconds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Above,
    Picked,
    Below,
}

impl Row {
    const ALL: [Self; 3] = [Self::Above, Self::Picked, Self::Below];

    #[must_use]
    pub fn step(self) -> i8 {
        match self {
            Self::Above => 1,
            Self::Picked => 0,
            Self::Below => -1,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Above => 0,
            Self::Picked => 1,
            Self::Below => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Wheel(Field, Row),
    Add,
    Entry(usize),
    Start,
    Play(usize),
    Pause,
    Close,
    Switch(usize),
    Day(Weekday),
    Done,
    Delete,
    PowerOff,
    LitTime,
    Sound,
    SoundList,
    Choose(usize),
    More,
    Vibration,
    Ramp,
    Recovery,
    Tab(Screen),
    Blank,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    size: Size,
    pixels: Vec<Colour>,
}

impl Frame {
    #[must_use]
    pub fn filled(size: Size, colour: Colour) -> Self {
        Self {
            size,
            pixels: vec![colour; size.width * size.height],
        }
    }

    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }

    #[must_use]
    pub fn row(&self, y: usize) -> &[Colour] {
        let start = (y * self.size.width).min(self.pixels.len());
        let end = (start + self.size.width).min(self.pixels.len());
        &self.pixels[start..end]
    }

    fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, colour: Colour) {
        let right = (x + width).min(self.size.width);
        let bottom = (y + height).min(self.size.height);
        for row in y.min(bottom)..bottom {
            let start = row * self.size.width;
            self.pixels[start + x.min(right)..start + right].fill(colour);
        }
    }

    fn blend(&mut self, x: usize, y: usize, colour: Colour, alpha: u8) {
        if x >= self.size.width || y >= self.size.height || alpha == 0 {
            return;
        }
        let pixel = &mut self.pixels[y * self.size.width + x];
        *pixel = if alpha == u8::MAX {
            colour
        } else {
            mixed(colour, *pixel, alpha)
        };
    }
}

fn mixed(over: Colour, under: Colour, alpha: u8) -> Colour {
    let above = u32::from(alpha);
    let below = u32::from(u8::MAX) - above;
    Colour([16, 8, 0].into_iter().fold(0, |sum, shift| {
        let top = (over.0 >> shift) & 0xff;
        let bottom = (under.0 >> shift) & 0xff;
        sum | ((top * above + bottom * below) / 0xff) << shift
    }))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    unit: usize,
    status_unit: usize,
    symbol: (usize, usize),
    alarm_unit: usize,
    tab_unit: usize,
    bar_top: usize,
}

fn layout(size: Size) -> Option<Layout> {
    let unit = size.width * 9 / 10 / UNITS_ACROSS;
    let alarm_unit = unit / 2;
    let status_unit = (unit / 8).max(1);
    let status_width =
        (FIGURES * (DIGIT_UNITS + 1) + PERCENT_UNITS + SYMBOL_GAP_UNITS + SYMBOL_UNITS)
            * status_unit
            + unit;
    let tab_unit = alarm_unit / 2;
    let bar_height = (TAB_UNITS + 2 * TAB_MARGIN_UNITS) * tab_unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    if tab_unit == 0 || height > size.height || bar_height > size.height {
        return None;
    }
    let top = (size.height - height) / 2;
    let mark_top = top + height + MARK_GAP_UNITS * unit;
    let alarm_top = mark_top + (MARK_HEIGHT_UNITS + MARK_GAP_UNITS) * unit;
    let bar_top = size.height - bar_height;
    if alarm_top + DIGIT_HEIGHT_UNITS * alarm_unit > bar_top
        || status_width > size.width
        || unit + DIGIT_HEIGHT_UNITS * status_unit > top
    {
        return None;
    }
    Some(Layout {
        unit,
        status_unit,
        symbol: (size.width - unit - SYMBOL_UNITS * status_unit, unit),
        alarm_unit,
        tab_unit,
        bar_top,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arrangement {
    Picker,
    Rows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sheet {
    unit: usize,
    left: usize,
    side: usize,
    far: usize,
    wheel_top: usize,
    list_top: usize,
}

impl Sheet {
    fn wheel_row(self, row: Row) -> usize {
        self.wheel_top + row.index() * ROW_UNITS * self.unit
    }

    fn list_row(self, index: usize) -> usize {
        self.list_top + index * ROW_UNITS * self.unit
    }

    fn setting_row(self, index: usize) -> usize {
        self.wheel_top + index * ROW_UNITS * self.unit
    }

    fn band(self, top: usize, y: usize) -> bool {
        band(self.unit, top, y)
    }

    fn wheels(self, y: usize) -> bool {
        let bottom = self.wheel_row(Row::Below) + (DIGIT_HEIGHT_UNITS + 1) * self.unit;
        y + self.unit >= self.wheel_top && y < bottom
    }

    fn days(self) -> (usize, usize) {
        (self.left, self.far + SIDE_UNITS * self.unit - self.left)
    }
}

fn centred_in(unit: usize, figure: usize) -> usize {
    (DIGIT_HEIGHT_UNITS * unit).saturating_sub(DIGIT_HEIGHT_UNITS * figure) / 2
}

fn band(unit: usize, top: usize, y: usize) -> bool {
    y + unit / 2 >= top && y + unit / 2 < top + ROW_UNITS * unit
}

fn sheet(size: Size, layout: &Layout, arrangement: Arrangement) -> Option<Sheet> {
    let unit = layout.alarm_unit;
    let clock_top = 3 * layout.unit;
    let active_top = clock_top + DIGIT_HEIGHT_UNITS * layout.unit + 2 * unit;
    let (wheel_top, columns) = match arrangement {
        Arrangement::Picker => (clock_top, 2),
        Arrangement::Rows => (
            active_top + (DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit,
            1,
        ),
    };
    let list_top = wheel_top + (2 * ROW_UNITS + DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit;
    let list_bottom = list_top + (SHEET_ROWS * ROW_UNITS - 1) * unit;
    let across = (UNITS_ACROSS + columns * (SIDE_GAP_UNITS + SIDE_UNITS)) * unit;
    if list_bottom > layout.bar_top || across > size.width {
        return None;
    }
    let left = (size.width - across) / 2;
    let beside = left + (UNITS_ACROSS + SIDE_GAP_UNITS) * unit;
    Some(Sheet {
        unit,
        left,
        side: beside,
        far: beside + (columns - 1) * (SIDE_UNITS + SIDE_GAP_UNITS) * unit,
        wheel_top,
        list_top,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Dial {
    unit: usize,
    centre: usize,
    top: usize,
    controls: usize,
    label: usize,
}

impl Dial {
    fn close(self) -> usize {
        self.centre.saturating_sub((CONTROL_UNITS + 2) * self.unit)
    }
}

fn dial(size: Size, layout: &Layout) -> Option<Dial> {
    let unit = layout.unit;
    let margin = SECTION_GAP_UNITS * unit;
    let height = (RING_UNITS + CONTROL_GAP_UNITS + CONTROL_UNITS) * unit;
    let room = layout.bar_top.checked_sub(margin + height)?;
    if RING_UNITS * unit > size.width {
        return None;
    }
    let top = margin + room / 2;
    Some(Dial {
        unit,
        centre: size.width / 2,
        top,
        controls: top + (RING_UNITS + CONTROL_GAP_UNITS) * unit,
        label: DATE_UNITS * layout.alarm_unit / 2,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Halo {
    unit: usize,
    centre: usize,
    top: usize,
    power: usize,
}

impl Halo {
    fn size(self) -> usize {
        RING_UNITS * self.unit
    }

    fn figure(self) -> usize {
        self.unit * 3 / 4
    }

    fn small(self) -> usize {
        self.unit / 3
    }

    fn clock(self) -> usize {
        (self.top + self.size() / 2).saturating_sub(DIGIT_HEIGHT_UNITS * self.figure() / 2)
    }

    fn rung(self) -> usize {
        self.clock() + DIGIT_HEIGHT_UNITS * self.figure() + self.unit / 2
    }

    fn icon(self) -> usize {
        ICON_UNITS * self.unit / 2
    }
}

fn halo(size: Size, layout: &Layout) -> Option<Halo> {
    let unit = layout.unit;
    let ring = RING_UNITS * unit;
    let gap = CONTROL_GAP_UNITS * unit;
    let icon = ICON_UNITS * unit / 2;
    let room = size.height.checked_sub(ring + gap + icon)?;
    if ring > size.width {
        return None;
    }
    let top = room / 2;
    Some(Halo {
        unit,
        centre: size.width / 2,
        top,
        power: top + ring + gap,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Board {
    unit: usize,
    centre: usize,
    top: usize,
    size: usize,
    left: usize,
    right: usize,
    list_top: usize,
    bottom: usize,
}

impl Board {
    fn middle(self) -> usize {
        self.top + self.size / 2
    }

    fn add(self) -> (usize, usize) {
        let along = (self.size - self.unit) / 2 * DIAGONAL_PER_MILLE / 1_000;
        (self.centre + along, self.middle() + along)
    }

    fn save(self) -> (usize, usize) {
        let (x, y) = self.add();
        ((2 * self.centre).saturating_sub(x), y)
    }

    fn picker(self) -> Sheet {
        let unit = self.unit * 3 / 4;
        let left = self.centre.saturating_sub(COUNTDOWN_UNITS * unit / 2);
        let picked = self.middle().saturating_sub(DIGIT_HEIGHT_UNITS * unit / 2);
        let side = left + COUNTDOWN_UNITS * unit;
        Sheet {
            unit,
            left,
            side,
            far: side,
            wheel_top: picked.saturating_sub(ROW_UNITS * unit),
            list_top: self.list_top,
        }
    }

    fn row(self, index: usize) -> usize {
        self.list_top + index * ROW_UNITS * self.unit
    }

    fn clip(self) -> usize {
        self.list_top - self.unit
    }

    fn shown(self, index: usize, scroll: usize) -> Option<usize> {
        let top = self.row(index).checked_sub(scroll)?;
        (top + ROW_UNITS * self.unit > self.clip() && top < self.bottom).then_some(top)
    }

    fn listed(self, rows: usize, y: usize, scroll: usize) -> Option<usize> {
        if y < self.clip() {
            return None;
        }
        (0..rows).find(|&index| band(self.unit, self.row(index), y + scroll))
    }

    fn reach(self, rows: usize) -> usize {
        (self.row(rows).saturating_sub(self.unit)).saturating_sub(self.bottom)
    }

    fn into_view(self, index: usize, scroll: usize) -> usize {
        let lowest = (self.row(index) + DIGIT_HEIGHT_UNITS * self.unit).saturating_sub(self.bottom);
        scroll.max(lowest).min(index * ROW_UNITS * self.unit)
    }

    fn cut(self, frame: &mut Frame) {
        let size = frame.size();
        frame.fill(0, 0, size.width, self.clip(), BACKGROUND);
        frame.fill(
            0,
            self.bottom,
            size.width,
            size.height - self.bottom,
            BACKGROUND,
        );
    }

    fn switch(self) -> usize {
        self.right - ICON_UNITS * self.unit
    }

    fn figure(self) -> usize {
        row_figure(self.unit)
    }

    fn label(self) -> usize {
        (self.unit * 4 / 3).max(1)
    }
}

fn row_figure(unit: usize) -> usize {
    unit * 2 / 3
}

fn board(size: Size, layout: &Layout) -> Option<Board> {
    let unit = layout.alarm_unit;
    let top = 3 * layout.unit;
    let ring = FACE_UNITS * layout.unit;
    let list_top = top + ring + 2 * layout.unit;
    let across = LIST_UNITS * unit;
    if list_top + (BOARD_ROWS * ROW_UNITS - 1) * unit > layout.bar_top
        || across > size.width
        || ring > size.width
    {
        return None;
    }
    let left = (size.width - across) / 2;
    Some(Board {
        unit,
        centre: size.width / 2,
        top,
        size: ring,
        left,
        right: left + across,
        list_top,
        bottom: layout.bar_top,
    })
}

fn glyph(
    frame: &mut Frame,
    character: char,
    (x, y): (usize, usize),
    across: usize,
    unit: usize,
    colour: Colour,
) {
    let align = match character {
        ':' | '-' => Align::Middle,
        _ => Align::Baseline,
    };
    let place = Place {
        centre2: 2 * x + across,
        middle2: 2 * y + DIGIT_HEIGHT_UNITS * unit,
        height: FIGURE_HEIGHT_UNITS * unit,
        align,
    };
    glyph::rasterise(character, place, |left, top, alpha| {
        frame.blend(left, top, colour, alpha);
    });
}

fn digit(frame: &mut Frame, x: usize, y: usize, unit: usize, value: u8, colour: Colour) {
    let character = char::from(b'0' + value % 10);
    glyph(frame, character, (x, y), DIGIT_UNITS * unit, unit, colour);
}

fn pair(frame: &mut Frame, (x, y): (usize, usize), unit: usize, value: u8, colour: Colour) {
    digit(frame, x, y, unit, value / 10, colour);
    digit(
        frame,
        x + (DIGIT_UNITS + 1) * unit,
        y,
        unit,
        value % 10,
        colour,
    );
}

fn colon(frame: &mut Frame, at: (usize, usize), unit: usize, colour: Colour) {
    glyph(frame, ':', at, unit, unit, colour);
}

fn of_day(time: TimeOfDay) -> [u8; 2] {
    [time.hour(), time.minute()]
}

fn of_countdown(countdown: Countdown) -> [u8; 3] {
    [countdown.hours(), countdown.minutes(), countdown.seconds()]
}

fn short(countdown: Countdown) -> String {
    let (hours, minutes, seconds) = (countdown.hours(), countdown.minutes(), countdown.seconds());
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn leading_zeros(value: &[u8]) -> usize {
    value
        .iter()
        .take(value.len().saturating_sub(1))
        .take_while(|&&pair_value| pair_value == 0)
        .count()
}

fn time(
    frame: &mut Frame,
    at: (usize, usize),
    unit: usize,
    value: Option<TimeOfDay>,
    colour: Colour,
) {
    match value {
        Some(time) => figures(frame, at, unit, &of_day(time), colour),
        None => dashes(frame, at, unit, colour),
    }
}

fn figures(frame: &mut Frame, at: (usize, usize), unit: usize, value: &[u8], colour: Colour) {
    faded(frame, at, unit, value, 0, colour);
}

fn faded(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    value: &[u8],
    dimmed: usize,
    colour: Colour,
) {
    for (index, &pair_value) in value.iter().enumerate() {
        let start = x + index * MINUTE_UNITS * unit;
        if index > 0 {
            let shown = if index <= dimmed { DIM } else { colour };
            colon(frame, (start - 2 * unit, y), unit, shown);
        }
        let shown = if index < dimmed { DIM } else { colour };
        pair(frame, (start, y), unit, pair_value, shown);
    }
}

fn advance(character: char) -> usize {
    if character == ':' { 2 } else { DIGIT_UNITS + 1 }
}

fn written_width(text: &str, unit: usize) -> usize {
    text.chars().map(advance).sum::<usize>().saturating_sub(1) * unit
}

fn written(frame: &mut Frame, (x, y): (usize, usize), unit: usize, text: &str, colour: Colour) {
    let mut left = x;
    for character in text.chars() {
        if character == ':' {
            colon(frame, (left, y), unit, colour);
        } else {
            glyph(
                frame,
                character,
                (left, y),
                DIGIT_UNITS * unit,
                unit,
                colour,
            );
        }
        left += advance(character) * unit;
    }
}

fn dashes(frame: &mut Frame, (x, y): (usize, usize), unit: usize, colour: Colour) {
    colon(frame, (x + COLON_UNITS * unit, y), unit, colour);
    for figure in [
        0,
        DIGIT_UNITS + 1,
        MINUTE_UNITS,
        MINUTE_UNITS + DIGIT_UNITS + 1,
    ] {
        glyph(
            frame,
            '-',
            (x + figure * unit, y),
            DIGIT_UNITS * unit,
            unit,
            colour,
        );
    }
}

fn percent_sign(frame: &mut Frame, x: usize, y: usize, unit: usize, colour: Colour) {
    glyph(frame, '%', (x, y), (PERCENT_UNITS + 1) * unit, unit, colour);
}

fn em(start2: usize, offset: usize, size: usize) -> usize {
    (start2 * ICON_EM + 2 * offset * size) / (2 * ICON_EM)
}

fn battery_symbol(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    gauge: Gauge,
    colour: Colour,
) {
    let size = BATTERY_UNITS * unit;
    let centre2 = 2 * x + SYMBOL_UNITS * unit;
    let middle2 = 2 * y + DIGIT_HEIGHT_UNITS * unit;
    centred(
        frame,
        Icon::Battery,
        (centre2, middle2),
        size,
        Ink::Flat(colour),
    );
    let bars = match gauge.level {
        Level::Low => 1,
        Level::Middle => 2,
        Level::Full => 3,
    };
    let (left2, top2) = (centre2 - size, middle2 - size);
    let top = em(top2, BAR_TOP, size);
    let bottom = em(top2, BAR_TOP + BAR_HEIGHT, size);
    for &bar in BARS.iter().take(bars) {
        let left = em(left2, bar, size);
        let right = em(left2, bar + BAR_WIDTH, size);
        frame.fill(left, top, right - left, bottom - top, colour);
    }
    let Some(capacity) = gauge.capacity else {
        return;
    };
    let sign = x - (SYMBOL_GAP_UNITS + PERCENT_UNITS) * unit;
    percent_sign(frame, sign, y, unit, colour);
    let figures: Vec<u8> = capacity
        .value()
        .to_string()
        .bytes()
        .map(|byte| byte - b'0')
        .collect();
    let advance = (DIGIT_UNITS + 1) * unit;
    let mut left = sign - figures.len() * advance;
    for figure in figures {
        digit(frame, left, y, unit, figure, colour);
        left += advance;
    }
}

fn centred(
    frame: &mut Frame,
    icon: Icon,
    (centre2, middle2): (usize, usize),
    size: usize,
    ink: Ink,
) {
    let square = Square {
        centre2,
        middle2,
        size,
    };
    glyph::icon(icon, square, |x, y, alpha| {
        frame.blend(x, y, inked(ink, square, x, y), alpha);
    });
}

fn inked(ink: Ink, square: Square, x: usize, y: usize) -> Colour {
    match ink {
        Ink::Flat(colour) => colour,
        Ink::Accent => {
            let left = square.centre2.saturating_sub(square.size) / 2;
            let top = square.middle2.saturating_sub(square.size) / 2;
            ACCENT.at(
                x.saturating_sub(left) + y.saturating_sub(top),
                2 * square.size,
            )
        }
    }
}

fn badge(frame: &mut Frame, (x, y): (usize, usize), unit: usize, icon: Icon, ink: Ink) {
    let size = ICON_UNITS * unit;
    let mask = Square {
        centre2: 2 * x,
        middle2: 2 * y,
        size: size + unit,
    };
    glyph::ring(mask, mask.size / 2, (1, 1), |x, y, alpha, _| {
        frame.blend(x, y, BACKGROUND, alpha);
    });
    let circle = Square {
        size: size * 5 / 6,
        ..mask
    };
    glyph::ring(circle, (unit / 3).max(1), (1, 1), |x, y, alpha, _| {
        frame.blend(x, y, inked(ink, circle, x, y), alpha);
    });
    centred(frame, icon, (2 * x, 2 * y), size * 4 / 5, ink);
}

fn symbol(frame: &mut Frame, icon: Icon, (x, y): (usize, usize), unit: usize, ink: Ink) {
    let size = ICON_UNITS * unit;
    centred(frame, icon, (2 * x + size, 2 * y + size), size, ink);
}

fn switch(frame: &mut Frame, (right, y): (usize, usize), unit: usize, on: bool) {
    let icon = if on { Icon::On } else { Icon::Off };
    symbol(
        frame,
        icon,
        (right - ICON_UNITS * unit, y),
        unit,
        accent(on),
    );
}

fn settings_screen(frame: &mut Frame, sheet: Sheet, bar_top: usize, face: &Face, rows: &[Target]) {
    let height = ROW_UNITS * sheet.unit;
    for (index, &row) in rows.iter().enumerate() {
        let Some(top) = sheet.setting_row(index).checked_sub(face.scroll) else {
            continue;
        };
        if top + height > sheet.wheel_top && top < bar_top {
            setting(frame, sheet, top, row, face);
        }
    }
    let size = frame.size();
    frame.fill(0, 0, size.width, sheet.wheel_top, BACKGROUND);
    frame.fill(0, bar_top, size.width, size.height - bar_top, BACKGROUND);
}

fn setting(frame: &mut Frame, sheet: Sheet, top: usize, row: Target, face: &Face) {
    let unit = sheet.unit;
    let right = sheet.side + SIDE_UNITS * unit;
    let at = (sheet.left, top);
    let plain = Ink::Flat(FOREGROUND);
    match row {
        Target::PowerOff => {
            symbol(frame, Icon::Power, at, unit, plain);
            switch(frame, (right, top), unit, face.power_off);
        }
        Target::LitTime => {
            symbol(frame, Icon::Light, at, unit, plain);
            length(frame, (right, top), unit, face.lit_seconds);
        }
        Target::Sound => {
            symbol(frame, Icon::Volume, at, unit, plain);
            switch(frame, (right, top), unit, face.outputs.sound);
        }
        Target::SoundList => sound_name(frame, sheet, top, face),
        Target::Vibration => {
            symbol(frame, Icon::Vibration, at, unit, plain);
            switch(frame, (right, top), unit, face.outputs.vibration);
        }
        Target::Ramp => {
            symbol(frame, Icon::Ramp, at, unit, plain);
            if face.ramp_seconds > 0 {
                length(frame, (right, top), unit, face.ramp_seconds);
            } else {
                switch(frame, (right, top), unit, false);
            }
        }
        Target::Recovery => {
            let ink = Ink::Flat(if face.recovery_marked {
                WARNING
            } else {
                FOREGROUND
            });
            symbol(frame, Icon::Recovery, at, unit, ink);
            symbol(
                frame,
                Icon::Restart,
                (right - ICON_UNITS * unit, top),
                unit,
                ink,
            );
        }
        _ => {}
    }
}

fn length(frame: &mut Frame, (right, top): (usize, usize), unit: usize, seconds: i64) {
    let shown = Countdown::of(seconds);
    figures(
        frame,
        (right - UNITS_ACROSS * unit, top),
        unit,
        &[shown.minutes(), shown.seconds()],
        FOREGROUND,
    );
}

fn sound_name(frame: &mut Frame, sheet: Sheet, top: usize, face: &Face) {
    let unit = sheet.unit;
    let right = sheet.side + SIDE_UNITS * unit;
    let (shown, colour) = face
        .sound
        .as_ref()
        .map_or(("None", DIM), |sound| (sound.stem(), FOREGROUND));
    name(frame, sheet, top, shown, colour);
    symbol(
        frame,
        Icon::Down,
        (right - ICON_UNITS * unit, top),
        unit,
        Ink::Flat(FOREGROUND),
    );
}

fn listing_screen(frame: &mut Frame, sheet: Sheet, listing: &Listing) {
    let unit = sheet.unit;
    let right = sheet.side + SIDE_UNITS * unit;
    let top = sheet.setting_row(0);
    symbol(
        frame,
        Icon::Close,
        (sheet.left, top),
        unit,
        Ink::Flat(FOREGROUND),
    );
    if listing.more {
        symbol(
            frame,
            Icon::Down,
            (right - ICON_UNITS * unit, top),
            unit,
            Ink::Flat(FOREGROUND),
        );
    }
    for (index, sound) in listing.sounds.iter().take(SOUNDS_SHOWN).enumerate() {
        let top = sheet.setting_row(index + 1);
        name(frame, sheet, top, sound.stem(), FOREGROUND);
        if listing.chosen == Some(index) {
            symbol(
                frame,
                Icon::Check,
                (right - ICON_UNITS * unit, top),
                unit,
                Ink::Accent,
            );
        }
    }
}

fn name(frame: &mut Frame, sheet: Sheet, top: usize, text: &str, colour: Colour) {
    let unit = sheet.unit;
    let height = NAME_UNITS * unit;
    let room = sheet.side.saturating_sub(sheet.left + unit);
    line(
        frame,
        fitted(text, height, room),
        (
            sheet.left,
            top + (DIGIT_HEIGHT_UNITS + NAME_UNITS) * unit / 2,
        ),
        height,
        colour,
    );
}

fn fitted(text: &str, height: usize, room: usize) -> &str {
    (0..=text.len())
        .rev()
        .filter_map(|end| text.get(..end))
        .find(|shown| glyph::width(shown, height) <= room)
        .unwrap_or("")
}

fn wheels(frame: &mut Frame, sheet: Sheet, picked: &[u8], figure: usize, dimmed: usize) {
    let unit = sheet.unit;
    let y = sheet.wheel_row(Row::Picked);
    faded(
        frame,
        (sheet.left, y + centred_in(unit, figure)),
        figure,
        picked,
        dimmed,
        FOREGROUND,
    );
    let above = 2 * (y - (ARROW_UNITS + ARROW_GAP_UNITS) * unit) + ARROW_UNITS * unit;
    let below = 2 * (y + (DIGIT_HEIGHT_UNITS + ARROW_GAP_UNITS) * unit) + ARROW_UNITS * unit;
    let size = ICON_UNITS * unit;
    for index in 0..picked.len() {
        let start = index * MINUTE_UNITS * figure;
        let centre2 = 2 * (sheet.left + start) + (2 * DIGIT_UNITS + 1) * figure;
        centred(frame, Icon::Up, (centre2, above), size, Ink::Flat(DIM));
        centred(frame, Icon::Down, (centre2, below), size, Ink::Flat(DIM));
    }
}

fn shade(on: bool) -> Colour {
    if on { FOREGROUND } else { DIM }
}

fn accent(on: bool) -> Ink {
    if on { Ink::Accent } else { Ink::Flat(DIM) }
}

fn timer_ring(frame: &mut Frame, board: Board, face: &Face) {
    let square = Square {
        centre2: 2 * board.centre,
        middle2: 2 * board.middle(),
        size: board.size,
    };
    glyph::ring(square, board.unit, (0, 1), |x, y, band, _| {
        frame.blend(x, y, DIM, band);
    });
    let sheet = board.picker();
    let picked = of_countdown(face.countdown);
    wheels(frame, sheet, &picked, sheet.unit, leading_zeros(&picked));
    badge(
        frame,
        board.add(),
        board.unit,
        Icon::Play,
        accent(!face.countdown.is_zero()),
    );
    let (icon, ink) = if face.timers.contains(&face.countdown) {
        let mark = if face.marked.is_some() { WARNING } else { DIM };
        (Icon::Delete, Ink::Flat(mark))
    } else {
        (Icon::Add, accent(face.timer_addable()))
    };
    badge(frame, board.save(), board.unit, icon, ink);
}

fn timer_row(frame: &mut Frame, board: Board, top: usize, length: Countdown) {
    let unit = board.unit;
    let figure = board.figure();
    let right = board.left + written_width(WIDEST_LENGTH, figure);
    let text = short(length);
    written(
        frame,
        (
            right.saturating_sub(written_width(&text, figure)),
            top + centred_in(unit, figure),
        ),
        figure,
        &text,
        FOREGROUND,
    );
    symbol(frame, Icon::Play, (board.switch(), top), unit, Ink::Accent);
}

const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn day_name(day: Weekday) -> &'static str {
    WEEKDAY_NAMES[day as usize]
}

fn short_name(day: Weekday) -> &'static str {
    day_name(day).get(..3).unwrap_or_default()
}

fn initial(day: Weekday) -> char {
    day_name(day).chars().next().unwrap_or(' ')
}

fn days(repeat: Weekdays) -> String {
    if repeat == Weekdays::EVERY_DAY {
        return "Daily".to_owned();
    }
    let chosen: Vec<Weekday> = Weekday::ALL
        .into_iter()
        .filter(|&day| repeat.contains(day))
        .collect();
    if chosen.len() <= 3 {
        chosen
            .into_iter()
            .map(short_name)
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        chosen
            .into_iter()
            .map(|day| initial(day).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn until(seconds: i64) -> String {
    let minutes = (seconds.max(0) + 59) / 60;
    let (days, hours, minutes) = (minutes / (24 * 60), minutes / 60 % 24, minutes % 60);
    if days > 0 {
        format!("Alarm in {days}d {hours}h")
    } else if hours > 0 {
        format!("Alarm in {hours}h {minutes}m")
    } else {
        format!("Alarm in {minutes}m")
    }
}

fn line(frame: &mut Frame, text: &str, at: (usize, usize), height: usize, colour: Colour) {
    glyph::write(text, at, height, |x, y, alpha| {
        frame.blend(x, y, colour, alpha);
    });
}

fn clock_face(frame: &mut Frame, board: Board, face: &Face) {
    let unit = board.unit;
    let square = Square {
        centre2: 2 * board.centre,
        middle2: 2 * board.middle(),
        size: board.size,
    };
    let left = board.centre.saturating_sub(board.size / 2);
    let minutes = SECONDS_PER_DAY / 60;
    let share = face.until.map_or((0, minutes), |until| {
        ((until.clamp(1, SECONDS_PER_DAY) + 59) / 60, minutes)
    });
    glyph::ring(square, unit, share, |x, y, band, arc| {
        frame.blend(x, y, DIM, band);
        if arc > 0 {
            let colour = ACCENT.at(
                x.saturating_sub(left) + y.saturating_sub(board.top),
                2 * board.size,
            );
            frame.blend(x, y, colour, arc);
        }
    });
    let figures_top = board.middle() + unit - DIGIT_HEIGHT_UNITS * unit / 2;
    time(
        frame,
        (
            board.centre.saturating_sub(UNITS_ACROSS * unit / 2),
            figures_top,
        ),
        unit,
        Some(face.time),
        FOREGROUND,
    );
    let date = format!("{} {}", day_name(face.weekday), face.day);
    let height = DATE_UNITS * unit;
    let across = glyph::width(&date, height);
    line(
        frame,
        &date,
        (board.centre.saturating_sub(across / 2), figures_top - unit),
        height,
        DIM,
    );
    let (x, y) = board.add();
    let size = ICON_UNITS * unit;
    let mask = Square {
        centre2: 2 * x,
        middle2: 2 * y,
        size: size + unit,
    };
    glyph::ring(mask, mask.size / 2, (1, 1), |x, y, alpha, _| {
        frame.blend(x, y, BACKGROUND, alpha);
    });
    centred(
        frame,
        Icon::AddCircle,
        (2 * x, 2 * y),
        size,
        accent(face.addable()),
    );
}

fn alarm_row(frame: &mut Frame, board: Board, top: usize, listed: Listed) {
    let unit = board.unit;
    let figure = board.figure();
    let colour = shade(listed.armed);
    time(
        frame,
        (
            board.left,
            top + (DIGIT_HEIGHT_UNITS * unit).saturating_sub(DIGIT_HEIGHT_UNITS * figure) / 2,
        ),
        figure,
        Some(listed.alarm.time),
        colour,
    );
    let left = board.left + UNITS_ACROSS * figure + LABEL_GAP_UNITS * unit;
    let middle = top + DIGIT_HEIGHT_UNITS * unit / 2;
    let height = board.label();
    let gap = unit / 2;
    line(
        frame,
        &days(listed.alarm.repeat),
        (left, middle - gap),
        height,
        colour,
    );
    if let Some(seconds) = listed.until {
        line(
            frame,
            &until(seconds),
            (left, middle + gap + height),
            height,
            DIM,
        );
    }
    switch(frame, (board.right, top), unit, listed.armed);
}

fn alarm_screen(frame: &mut Frame, board: Board, face: &Face) {
    for (index, &listed) in face.alarms.iter().take(SAVED_ALARMS).enumerate() {
        if let Some(top) = board.shown(index, face.scroll) {
            alarm_row(frame, board, top, listed);
        }
    }
    board.cut(frame);
    clock_face(frame, board, face);
}

fn editor(frame: &mut Frame, sheet: Sheet, alarm: Alarm, marked: bool) {
    let unit = sheet.unit;
    wheels(frame, sheet, &of_day(alarm.time), sheet.unit, 0);
    let top = sheet.wheel_row(Row::Picked);
    symbol(frame, Icon::Check, (sheet.side, top), unit, Ink::Accent);
    let mark = if marked { WARNING } else { DIM };
    symbol(frame, Icon::Delete, (sheet.far, top), unit, Ink::Flat(mark));
    let (left, across) = sheet.days();
    let middle2 = 2 * sheet.list_row(0) + DIGIT_HEIGHT_UNITS * unit;
    for (index, day) in Weekday::ALL.into_iter().enumerate() {
        let chosen = alarm.repeat.contains(day);
        let place = Place {
            centre2: 2 * left + (2 * index + 1) * across / Weekday::ALL.len(),
            middle2,
            height: DAY_UNITS * unit,
            align: Align::Baseline,
        };
        glyph::rasterise(initial(day), place, |x, y, alpha| {
            let colour = if chosen {
                ACCENT.at(x.saturating_sub(left), across)
            } else {
                DIM
            };
            frame.blend(x, y, colour, alpha);
        });
    }
}

fn list_stage(frame: &mut Frame, board: Board, face: &Face) {
    for (index, &length) in face.timers.iter().take(SAVED_TIMERS).enumerate() {
        if let Some(top) = board.shown(index, face.scroll) {
            timer_row(frame, board, top, length);
        }
    }
    board.cut(frame);
    timer_ring(frame, board, face);
}

fn running_stage(frame: &mut Frame, dial: Dial, running: Running) {
    let unit = dial.unit;
    let size = RING_UNITS * unit;
    let square = Square {
        centre2: 2 * dial.centre,
        middle2: 2 * dial.top + size,
        size,
    };
    let left = dial.centre.saturating_sub(size / 2);
    let share = (running.left.total(), running.length.total());
    glyph::ring(square, unit / 2, share, |x, y, band, arc| {
        frame.blend(x, y, DIM, band);
        if arc > 0 {
            let colour = ACCENT.at(
                x.saturating_sub(left) + y.saturating_sub(dial.top),
                2 * size,
            );
            frame.blend(x, y, colour, arc);
        }
    });
    let parts = if running.left.hours() > 0 {
        HOURS_PARTS
    } else {
        MINUTES_PARTS
    };
    let figure = unit * parts / COUNTDOWN_PARTS;
    let left_text = short(running.left);
    let figures_top = (dial.top + size / 2).saturating_sub(DIGIT_HEIGHT_UNITS * figure / 2);
    written(
        frame,
        (
            dial.centre
                .saturating_sub(written_width(&left_text, figure) / 2),
            figures_top,
        ),
        figure,
        &left_text,
        FOREGROUND,
    );
    let length = short(running.length);
    let across = glyph::width(&length, dial.label);
    line(
        frame,
        &length,
        (
            dial.centre.saturating_sub(across / 2),
            figures_top + DIGIT_HEIGHT_UNITS * figure + 2 * dial.label,
        ),
        dial.label,
        DIM,
    );
    let control = CONTROL_UNITS * unit;
    let middle2 = 2 * dial.controls + control;
    let icon = if running.paused {
        Icon::Play
    } else {
        Icon::Pause
    };
    centred(
        frame,
        icon,
        (2 * dial.centre, middle2),
        control,
        Ink::Accent,
    );
    if running.paused {
        centred(
            frame,
            Icon::Close,
            (2 * dial.close(), middle2),
            control,
            Ink::Flat(FOREGROUND),
        );
    }
}

fn ringing_screen(frame: &mut Frame, halo: Halo, face: &Face) {
    let unit = halo.unit;
    let size = halo.size();
    let square = Square {
        centre2: 2 * halo.centre,
        middle2: 2 * halo.top + size,
        size,
    };
    let left = halo.centre.saturating_sub(size / 2);
    glyph::ring(square, unit / 2, (1, 1), |x, y, _, arc| {
        let colour = ACCENT.at(
            x.saturating_sub(left) + y.saturating_sub(halo.top),
            2 * size,
        );
        frame.blend(x, y, colour, arc);
    });
    let figure = halo.figure();
    let clock = halo.clock();
    let icon = if face.rung.is_some() {
        Icon::Timer
    } else {
        Icon::Alarm
    };
    let symbol = halo.icon();
    centred(
        frame,
        icon,
        (
            2 * halo.centre,
            (2 * clock).saturating_sub(2 * unit + symbol),
        ),
        symbol,
        Ink::Accent,
    );
    time(
        frame,
        (halo.centre.saturating_sub(UNITS_ACROSS * figure / 2), clock),
        figure,
        Some(face.time),
        FOREGROUND,
    );
    let small = halo.small();
    match (face.rung, face.rang) {
        (Some(rung), _) => {
            let text = short(rung);
            written(
                frame,
                (
                    halo.centre.saturating_sub(written_width(&text, small) / 2),
                    halo.rung(),
                ),
                small,
                &text,
                DIM,
            );
        }
        (None, rang) => time(
            frame,
            (
                halo.centre.saturating_sub(UNITS_ACROSS * small / 2),
                halo.rung(),
            ),
            small,
            rang,
            DIM,
        ),
    }
    centred(
        frame,
        Icon::Power,
        (2 * halo.centre, 2 * halo.power + symbol),
        symbol,
        Ink::Flat(DIM),
    );
}

fn timer_screen(frame: &mut Frame, layout: &Layout, board: Board, face: &Face) {
    match (face.running, dial(frame.size(), layout)) {
        (Some(running), Some(dial)) => running_stage(frame, dial, running),
        (Some(_), None) => {}
        (None, _) => list_stage(frame, board, face),
    }
}

fn bar(frame: &mut Frame, layout: &Layout, current: Screen) {
    let width = frame.size().width;
    let across = TAB_UNITS * layout.tab_unit;
    let top = layout.bar_top + TAB_MARGIN_UNITS * layout.tab_unit;
    for (index, screen) in Screen::ALL.into_iter().enumerate() {
        let centre = width * (2 * index + 1) / (2 * Screen::ALL.len());
        centred(
            frame,
            screen.icon(),
            (2 * centre, 2 * top + across),
            across,
            accent(screen == current),
        );
    }
}

fn volume_panel(frame: &mut Frame, unit: usize, volume: Option<Volume>) {
    let Some(volume) = volume else {
        return;
    };
    let half = (unit / 2).max(1);
    let width = VOLUME_HALVES * half;
    let height = (VOLUME_ICON_HALVES + VOLUME_TRACK_HALVES + 3) * half;
    let size = frame.size();
    if width + half > size.width || height > size.height {
        return;
    }
    let top = (size.height - height) / 2;
    frame.fill(half, top, width, height, BACKGROUND);
    let centre = half + width / 2;
    let icon = VOLUME_ICON_HALVES * half;
    centred(
        frame,
        Icon::Volume,
        (2 * centre, 2 * (top + half) + icon),
        icon,
        Ink::Flat(FOREGROUND),
    );
    let track = VOLUME_TRACK_HALVES * half;
    let track_top = top + (VOLUME_ICON_HALVES + 2) * half;
    let left = centre - half / 2;
    frame.fill(left, track_top, half, track, DIM);
    let filled = track * usize::from(volume.level()) / usize::from(Volume::LOUDEST.level());
    for row in track - filled..track {
        frame.fill(left, track_top + row, half, 1, ACCENT.at(row, track));
    }
}

#[must_use]
pub fn render(size: Size, face: &Face) -> Frame {
    drawn(size, face, &SETTINGS)
}

fn drawn(size: Size, face: &Face, rows: &[Target]) -> Frame {
    let mut frame = Frame::filled(size, BACKGROUND);
    let Some(layout) = layout(size) else {
        return frame;
    };
    let picker = sheet(size, &layout, Arrangement::Picker);
    let board = board(size, &layout);
    let shown = match face.screen {
        Screen::Alarm | Screen::Timer => board.is_some(),
        Screen::Settings => false,
    };
    if face.ringing {
        status(&mut frame, &layout, face.battery);
        if let Some(halo) = halo(size, &layout) {
            ringing_screen(&mut frame, halo, face);
        }
        volume_panel(&mut frame, layout.unit, face.volume);
        return frame;
    }
    match (face.screen, face.editing) {
        (Screen::Alarm, Some(alarm)) => {
            if let (Some(sheet), Some(_)) = (picker, board) {
                editor(&mut frame, sheet, alarm, face.marked.is_some());
            }
        }
        (Screen::Alarm, None) => {
            if let Some(board) = board {
                alarm_screen(&mut frame, board, face);
            }
        }
        (Screen::Timer, _) => {
            if let Some(board) = board {
                timer_screen(&mut frame, &layout, board, face);
            }
        }
        (Screen::Settings, _) => {
            if let Some(sheet) = sheet(size, &layout, Arrangement::Rows) {
                match &face.listing {
                    Some(listing) => listing_screen(&mut frame, sheet, listing),
                    None => settings_screen(&mut frame, sheet, layout.bar_top, face, rows),
                }
            }
        }
    }
    if shown {
        status(&mut frame, &layout, face.battery);
    }
    bar(&mut frame, &layout, face.screen);
    volume_panel(&mut frame, layout.unit, face.volume);
    frame
}

fn status(frame: &mut Frame, layout: &Layout, battery: Gauge) {
    let colour = if battery.level == Level::Low {
        WARNING
    } else {
        FOREGROUND
    };
    battery_symbol(frame, layout.symbol, layout.status_unit, battery, colour);
}

#[must_use]
pub fn pitch(size: Size) -> usize {
    layout(size).map_or(1, |layout| layout.unit.max(1))
}

#[must_use]
pub fn reach(size: Size, screen: Screen, listed: usize) -> usize {
    match screen {
        Screen::Alarm | Screen::Timer => layout(size)
            .and_then(|layout| board(size, &layout))
            .map_or(0, |board| board.reach(listed)),
        Screen::Settings => reach_of(size, &SETTINGS),
    }
}

#[must_use]
pub fn into_view(size: Size, index: usize, scroll: usize) -> usize {
    layout(size)
        .and_then(|layout| board(size, &layout))
        .map_or(0, |board| board.into_view(index, scroll))
}

fn reach_of(size: Size, rows: &[Target]) -> usize {
    let Some(layout) = layout(size) else {
        return 0;
    };
    sheet(size, &layout, Arrangement::Rows).map_or(0, |sheet| {
        (sheet.setting_row(rows.len()).saturating_sub(sheet.unit)).saturating_sub(layout.bar_top)
    })
}

fn wheel(sheet: Sheet, fields: &[Field], figure: usize, x: usize, y: usize) -> Target {
    let passed = (1..fields.len())
        .filter(|&index| {
            x >= sheet.left + (COLON_UNITS + (index - 1) * MINUTE_UNITS) * figure + figure / 2
        })
        .count();
    let field = fields.get(passed).copied().unwrap_or(Field::Minutes);
    let row = Row::ALL
        .into_iter()
        .find(|&row| sheet.band(sheet.wheel_row(row), y))
        .unwrap_or(if y < sheet.wheel_top {
            Row::Above
        } else {
            Row::Below
        });
    Target::Wheel(field, row)
}

fn ring_target(board: Board, x: usize, y: usize, scroll: usize) -> Target {
    let unit = board.unit;
    let reach = ICON_UNITS * unit / 2;
    let near = |(centre, middle): (usize, usize)| {
        x.abs_diff(centre) <= reach && y.abs_diff(middle) <= reach
    };
    if near(board.add()) {
        return Target::Start;
    }
    if near(board.save()) {
        return Target::Add;
    }
    let sheet = board.picker();
    if sheet.wheels(y) {
        return wheel(sheet, &COUNTDOWN_FIELDS, sheet.unit, x, y);
    }
    match board.listed(SAVED_TIMERS, y, scroll) {
        Some(index) if x + 3 * unit / 2 >= board.switch() => Target::Play(index),
        Some(index) => Target::Entry(index),
        None => Target::Blank,
    }
}

fn editor_target(sheet: Sheet, x: usize, y: usize) -> Target {
    let unit = sheet.unit;
    if sheet.wheels(y) {
        let picked = sheet.band(sheet.wheel_row(Row::Picked), y);
        return if x + 3 * unit / 2 < sheet.side {
            wheel(sheet, &TIME_FIELDS, unit, x, y)
        } else if !picked {
            Target::Blank
        } else if x + 3 * unit / 2 >= sheet.far {
            Target::Delete
        } else {
            Target::Done
        };
    }
    if !sheet.band(sheet.list_row(0), y) {
        return Target::Blank;
    }
    let (left, across) = sheet.days();
    let index = (x.saturating_sub(left) * Weekday::ALL.len())
        .checked_div(across)
        .unwrap_or(0);
    Weekday::ALL
        .get(index.min(Weekday::ALL.len() - 1))
        .map_or(Target::Blank, |&day| Target::Day(day))
}

fn board_target(board: Board, x: usize, y: usize, scroll: usize) -> Target {
    let unit = board.unit;
    let (centre, middle) = board.add();
    let reach = (ICON_UNITS + 2) * unit / 2;
    if x.abs_diff(centre) <= reach && y.abs_diff(middle) <= reach {
        return Target::Add;
    }
    match board.listed(SAVED_ALARMS, y, scroll) {
        Some(index) if x + 3 * unit / 2 >= board.switch() => Target::Switch(index),
        Some(index) => Target::Entry(index),
        None => Target::Blank,
    }
}

fn dial_target(dial: Dial, x: usize, y: usize) -> Target {
    let unit = dial.unit;
    if y + unit < dial.controls || y >= dial.controls + (CONTROL_UNITS + 1) * unit {
        Target::Blank
    } else if 2 * x < dial.close() + dial.centre {
        Target::Close
    } else {
        Target::Pause
    }
}

fn listing_target(sheet: Sheet, x: usize, y: usize) -> Target {
    if sheet.band(sheet.setting_row(0), y) {
        return if x >= sheet.side {
            Target::More
        } else {
            Target::Close
        };
    }
    (0..SOUNDS_SHOWN)
        .find(|&index| sheet.band(sheet.setting_row(index + 1), y))
        .map_or(Target::Blank, Target::Choose)
}

fn settings_target(sheet: Sheet, rows: &[Target], y: usize, scroll: usize) -> Target {
    if y + sheet.unit / 2 < sheet.wheel_top {
        return Target::Blank;
    }
    (0..rows.len())
        .find(|&index| sheet.band(sheet.setting_row(index), y + scroll))
        .and_then(|index| rows.get(index).copied())
        .unwrap_or(Target::Blank)
}

#[must_use]
pub fn target(
    size: Size,
    screen: Screen,
    stage: Stage,
    x: usize,
    y: usize,
    scroll: usize,
) -> Target {
    let Some(layout) = layout(size) else {
        return Target::Blank;
    };
    if y >= layout.bar_top {
        return Target::Tab(if x < size.width / 3 {
            Screen::Alarm
        } else if x < size.width * 2 / 3 {
            Screen::Timer
        } else {
            Screen::Settings
        });
    }
    match (screen, stage) {
        (Screen::Timer, Stage::Running) => {
            dial(size, &layout).map_or(Target::Blank, |dial| dial_target(dial, x, y))
        }
        (Screen::Alarm, Stage::Editing) => match (
            sheet(size, &layout, Arrangement::Picker),
            board(size, &layout),
        ) {
            (Some(sheet), Some(_)) => editor_target(sheet, x, y),
            _ => Target::Blank,
        },
        (Screen::Alarm, _) => {
            board(size, &layout).map_or(Target::Blank, |board| board_target(board, x, y, scroll))
        }
        (Screen::Timer, _) => {
            board(size, &layout).map_or(Target::Blank, |board| ring_target(board, x, y, scroll))
        }
        (Screen::Settings, Stage::Choosing) => sheet(size, &layout, Arrangement::Rows)
            .map_or(Target::Blank, |sheet| listing_target(sheet, x, y)),
        (Screen::Settings, _) => sheet(size, &layout, Arrangement::Rows)
            .map_or(Target::Blank, |sheet| {
                settings_target(sheet, &SETTINGS, y, scroll)
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{ChargeLimit, Percent};
    use crate::time::Weekday;
    use std::collections::HashSet;
    use std::sync::OnceLock;

    type Outcome = Result<(), Box<dyn std::error::Error>>;

    const SMALL: Size = Size {
        width: 100,
        height: 140,
    };
    const SHEET: Size = Size {
        width: 108,
        height: 228,
    };
    const PANEL: Size = Size {
        width: 1080,
        height: 2280,
    };

    const PALETTE: [(Colour, char); 4] = [
        (BACKGROUND, '.'),
        (FOREGROUND, '#'),
        (WARNING, '!'),
        (DIM, ':'),
    ];

    fn distance(one: Colour, other: Colour) -> u32 {
        [16, 8, 0]
            .into_iter()
            .map(|shift| ((one.0 >> shift) & 0xff).abs_diff((other.0 >> shift) & 0xff))
            .map(|step| step * step)
            .sum()
    }

    fn nearest(colour: Colour) -> char {
        let flat = PALETTE
            .into_iter()
            .min_by_key(|&(palette, _)| distance(colour, palette))
            .map_or('.', |(_, character)| character);
        if flat != '.' && accented(colour) {
            '*'
        } else {
            flat
        }
    }

    fn blended(pixel: Colour, colour: Colour) -> bool {
        let strongest = [16, 8, 0]
            .into_iter()
            .max_by_key(|&shift| (colour.0 >> shift) & 0xff)
            .unwrap_or(0);
        let full = (colour.0 >> strongest) & 0xff;
        let part = (pixel.0 >> strongest) & 0xff;
        let guess = (part * 0xff).checked_div(full).unwrap_or(0);
        (guess.saturating_sub(1)..=guess + 2)
            .filter_map(|alpha| u8::try_from(alpha).ok())
            .any(|alpha| alpha > 0 && mixed(colour, BACKGROUND, alpha) == pixel)
    }

    fn accented(pixel: Colour) -> bool {
        static SHADES: OnceLock<HashSet<Colour>> = OnceLock::new();
        SHADES
            .get_or_init(|| {
                (0..=u8::MAX)
                    .map(|step| ACCENT.at(usize::from(step), usize::from(u8::MAX)))
                    .flat_map(|colour| {
                        (1..=u8::MAX).map(move |alpha| mixed(colour, BACKGROUND, alpha))
                    })
                    .collect()
            })
            .contains(&pixel)
    }

    fn on_track(pixel: Colour) -> bool {
        static SHADES: OnceLock<HashSet<Colour>> = OnceLock::new();
        SHADES
            .get_or_init(|| {
                (0..=u8::MAX)
                    .map(|step| ACCENT.at(usize::from(step), usize::from(u8::MAX)))
                    .flat_map(|colour| {
                        (1..=u8::MAX).flat_map(move |alpha| {
                            [
                                mixed(colour, DIM, alpha),
                                mixed(colour, mixed(DIM, BACKGROUND, alpha), alpha),
                            ]
                        })
                    })
                    .collect()
            })
            .contains(&pixel)
    }

    fn solid(pixel: Colour) -> bool {
        (0..=u8::MAX).any(|step| ACCENT.at(usize::from(step), usize::from(u8::MAX)) == pixel)
    }

    fn picture(frame: &Frame) -> String {
        (0..frame.size().height)
            .map(|y| {
                frame
                    .row(y)
                    .iter()
                    .map(|&colour| nearest(colour))
                    .chain(['\n'])
                    .collect::<String>()
            })
            .collect()
    }

    fn charged(value: u8) -> Result<Gauge, Box<dyn std::error::Error>> {
        Ok(Gauge::of(Percent::new(value)?, ChargeLimit::DEFAULT))
    }

    fn listed(
        hour: u8,
        minute: u8,
        repeat: Weekdays,
        until: Option<i64>,
    ) -> Result<Listed, Box<dyn std::error::Error>> {
        Ok(Listed {
            alarm: Alarm {
                time: TimeOfDay::new(hour, minute)?,
                repeat,
            },
            armed: until.is_some(),
            until,
        })
    }

    fn weekend() -> Weekdays {
        Weekdays::NONE.with(Weekday::Monday).with(Weekday::Sunday)
    }

    fn face(
        armed: bool,
        ringing: bool,
        battery: Gauge,
    ) -> Result<Face, Box<dyn std::error::Error>> {
        let until = armed.then_some(11 * 3_600 + 8 * 60);
        Ok(Face {
            time: TimeOfDay::new(19, 37)?,
            weekday: Weekday::Monday,
            day: 18,
            alarms: vec![
                listed(6, 45, Weekdays::EVERY_DAY, until)?,
                listed(7, 30, weekend(), None)?,
            ],
            until,
            editing: None,
            rang: if armed {
                Some(TimeOfDay::new(6, 45)?)
            } else {
                None
            },
            marked: None,
            ringing,
            battery,
            screen: Screen::Alarm,
            power_off: true,
            lit_seconds: 10,
            ramp_seconds: 60,
            outputs: Outputs::BOTH,
            recovery_marked: false,
            timers: Vec::new(),
            countdown: Countdown::default(),
            running: None,
            rung: None,
            volume: None,
            sound: SoundName::new("ringtone.wav"),
            listing: None,
            scroll: 0,
        })
    }

    fn editing(marked: Option<usize>) -> Result<Face, Box<dyn std::error::Error>> {
        Ok(Face {
            editing: Some(Alarm {
                time: TimeOfDay::new(7, 15)?,
                repeat: weekend(),
            }),
            marked,
            ..face(true, false, charged(80)?)?
        })
    }

    #[test]
    fn the_alarm_screen_shows_the_clock_in_its_ring_the_plus_and_each_alarm_with_its_switch_as_recorded()
    -> Outcome {
        let frame = render(SHEET, &face(true, false, charged(80)?)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-sheet.txt"));
        Ok(())
    }

    #[test]
    fn with_nothing_armed_and_a_full_list_the_ring_and_the_plus_are_dim() -> Outcome {
        let full = Face {
            alarms: unarmed()?,
            ..face(false, false, charged(7)?)?
        };
        let frame = render(SHEET, &full);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-sheet-full.txt")
        );
        Ok(())
    }

    #[test]
    fn an_empty_list_shows_the_ring_alone_and_an_unread_battery_the_low_symbol() -> Outcome {
        let empty = Face {
            alarms: Vec::new(),
            ..face(false, false, Gauge::UNREAD)?
        };
        let frame = render(SHEET, &empty);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-sheet-empty.txt")
        );
        Ok(())
    }

    #[test]
    fn the_editor_shows_the_wheels_the_check_the_cross_and_the_seven_days_as_recorded() -> Outcome {
        let frame = render(SHEET, &editing(None)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-editor.txt"));
        Ok(())
    }

    #[test]
    fn the_arc_shows_the_time_until_the_next_alarm_as_a_share_of_a_day() -> Outcome {
        let accent = |until| -> Result<usize, Box<dyn std::error::Error>> {
            let shown = Face {
                until,
                alarms: Vec::new(),
                ..face(false, false, charged(80)?)?
            };
            Ok(lit(&render(PANEL, &shown))
                .into_iter()
                .filter(|&(_, y, colour)| y < 900 && accented(colour))
                .count())
        };
        let (day, half, none) = (
            accent(Some(SECONDS_PER_DAY))?,
            accent(Some(SECONDS_PER_DAY / 2))?,
            accent(None)?,
        );
        assert!(day > half && half > none);
        assert_eq!(accent(Some(2 * SECONDS_PER_DAY))?, day);
        Ok(())
    }

    #[test]
    fn a_row_shows_its_days_and_the_time_until_it_rings_and_its_switch_lit_only_when_armed()
    -> Outcome {
        let layout = layout(PANEL).ok_or("no layout")?;
        let board = board(PANEL, &layout).ok_or("no board")?;
        let row = |frame: &Frame| -> Vec<Colour> {
            (board.row(0)..board.row(1))
                .flat_map(|y| frame.row(y).to_vec())
                .filter(|&colour| colour != BACKGROUND)
                .collect()
        };
        let armed = render(PANEL, &face(true, false, charged(80)?)?);
        let off = render(PANEL, &face(false, false, charged(80)?)?);
        assert!(row(&armed).contains(&FOREGROUND));
        assert!(row(&armed).iter().any(|&colour| accented(colour)));
        assert!(row(&off).iter().all(|&colour| blended(colour, DIM)));
        assert!(row(&armed).len() > row(&off).len());
        Ok(())
    }

    #[test]
    fn a_label_names_daily_up_to_three_days_or_their_initials() {
        let of = |days: &[Weekday]| {
            days.iter()
                .fold(Weekdays::NONE, |chosen, &day| chosen.with(day))
        };
        assert_eq!(days(Weekdays::EVERY_DAY), "Daily");
        assert_eq!(days(Weekdays::NONE), "");
        assert_eq!(days(of(&[Weekday::Sunday, Weekday::Monday])), "Mon Sun");
        assert_eq!(
            days(of(&[
                Weekday::Tuesday,
                Weekday::Thursday,
                Weekday::Saturday
            ])),
            "Tue Thu Sat"
        );
        assert_eq!(days(Weekdays::WORKDAYS), "M T W T F");
    }

    #[test]
    fn the_time_until_an_alarm_is_rounded_up_to_the_minute() {
        assert_eq!(until(0), "Alarm in 0m");
        assert_eq!(until(1), "Alarm in 1m");
        assert_eq!(until(60), "Alarm in 1m");
        assert_eq!(until(61), "Alarm in 2m");
        assert_eq!(until(11 * 3_600 + 41 * 60 + 1), "Alarm in 11h 42m");
        assert_eq!(until(2 * SECONDS_PER_DAY + 3 * 3_600), "Alarm in 2d 3h");
    }

    #[test]
    fn a_marked_cross_alone_turns_red() -> Outcome {
        let plain = render(SHEET, &editing(None)?);
        let marked = render(SHEET, &editing(Some(0))?);
        let layout = layout(SHEET).ok_or("no layout")?;
        let sheet = sheet(SHEET, &layout, Arrangement::Picker).ok_or("no sheet")?;
        let across = SIDE_UNITS * sheet.unit;
        let top = sheet.wheel_row(Row::Picked);
        let mut turned = false;
        for y in 0..SHEET.height {
            for (x, (&before, &after)) in plain.row(y).iter().zip(marked.row(y)).enumerate() {
                let inside = (sheet.far..sheet.far + across).contains(&x)
                    && (top..top + across).contains(&y);
                if inside && before != BACKGROUND {
                    assert!(blended(before, DIM) && blended(after, WARNING));
                    turned |= after != before;
                } else {
                    assert_eq!(after, before);
                }
            }
        }
        assert!(turned);
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_shows_the_halo_as_recorded() -> Outcome {
        let frame = render(SMALL, &face(true, true, charged(64)?)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-ringing.txt")
        );
        Ok(())
    }

    fn unarmed() -> Result<Vec<Listed>, Box<dyn std::error::Error>> {
        [
            (5, 0),
            (6, 45),
            (7, 30),
            (9, 0),
            (10, 5),
            (12, 0),
            (13, 30),
            (15, 15),
            (17, 0),
            (18, 40),
            (20, 0),
            (22, 10),
        ]
        .into_iter()
        .map(|(hour, minute)| listed(hour, minute, Weekdays::NONE, None))
        .collect()
    }

    fn lengths() -> Result<Vec<Countdown>, Box<dyn std::error::Error>> {
        Ok((1..=12)
            .map(|minutes| Countdown::new(0, minutes, 0))
            .collect::<Result<_, _>>()?)
    }

    fn timer(running: Option<Running>, ringing: bool) -> Result<Face, Box<dyn std::error::Error>> {
        Ok(Face {
            screen: Screen::Timer,
            timers: vec![Countdown::new(0, 3, 0)?, Countdown::new(0, 12, 5)?],
            countdown: Countdown::new(0, 3, 0)?,
            running,
            ..face(true, ringing, charged(80)?)?
        })
    }

    fn running(paused: bool) -> Result<Option<Running>, Box<dyn std::error::Error>> {
        Ok(Some(Running {
            left: Countdown::new(0, 1, 7)?,
            length: Countdown::new(0, 3, 0)?,
            paused,
        }))
    }

    #[test]
    fn a_length_reads_minutes_under_an_hour_and_hours_from_one() -> Outcome {
        assert_eq!(short(Countdown::new(0, 0, 5)?), "0:05");
        assert_eq!(short(Countdown::new(0, 59, 59)?), "59:59");
        assert_eq!(short(Countdown::new(1, 0, 0)?), "1:00:00");
        assert_eq!(short(Countdown::LONGEST), "23:59:59");
        Ok(())
    }

    #[test]
    fn a_written_length_spaces_its_figures_as_the_pairs_do() {
        assert_eq!(written_width("00:00", 3), UNITS_ACROSS * 3);
        assert_eq!(written_width("00:00:00", 3), COUNTDOWN_UNITS * 3);
        assert_eq!(
            written_width(WIDEST_LENGTH, 1),
            COUNTDOWN_UNITS - DIGIT_UNITS - 1
        );
    }

    #[test]
    fn the_picker_dims_its_leading_zero_pairs_but_never_the_last() {
        assert_eq!(leading_zeros(&[0, 25, 0]), 1);
        assert_eq!(leading_zeros(&[0, 0, 30]), 2);
        assert_eq!(leading_zeros(&[0, 0, 0]), 2);
        assert_eq!(leading_zeros(&[1, 0, 0]), 0);
    }

    #[test]
    fn the_list_stage_shows_the_picker_in_a_ring_its_bin_and_each_saved_timer_with_a_play_as_recorded()
    -> Outcome {
        let frame = render(SHEET, &timer(None, false)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-timer.txt"));
        Ok(())
    }

    #[test]
    fn an_unsaved_length_with_a_full_list_shows_a_dim_plus_as_recorded() -> Outcome {
        let full = Face {
            timers: lengths()?,
            countdown: Countdown::new(0, 25, 0)?,
            ..timer(None, false)?
        };
        let frame = render(SHEET, &full);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-timer-unsaved.txt")
        );
        Ok(())
    }

    #[test]
    fn the_running_stage_shows_the_time_left_in_a_part_emptied_ring_and_a_pause_as_recorded()
    -> Outcome {
        let frame = render(SHEET, &timer(running(false)?, false)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-timer-running.txt")
        );
        Ok(())
    }

    #[test]
    fn the_running_stage_shows_the_length_it_counts_from_below_the_time_left() -> Outcome {
        let from = |minutes| -> Result<Face, Box<dyn std::error::Error>> {
            Ok(Face {
                running: Some(Running {
                    left: Countdown::new(0, 1, 7)?,
                    length: Countdown::new(0, minutes, 0)?,
                    paused: false,
                }),
                ..timer(None, false)?
            })
        };
        let dial = layout(PANEL)
            .and_then(|layout| dial(PANEL, &layout))
            .ok_or("no dial")?;
        let middle = dial.top + RING_UNITS * dial.unit / 2;
        let inside = |x: usize, y: usize| {
            x.abs_diff(dial.centre) < 4 * dial.unit && y.abs_diff(middle) < 7 * dial.unit
        };
        let three = render(PANEL, &from(3)?);
        let five = render(PANEL, &from(5)?);
        let changed: Vec<(usize, usize)> = (0..PANEL.height)
            .flat_map(|y| (0..PANEL.width).map(move |x| (x, y)))
            .filter(|&(x, y)| inside(x, y) && three.row(y)[x] != five.row(y)[x])
            .collect();
        assert_ne!(changed, []);
        assert!(changed.iter().all(|&(_, y)| y > middle));
        assert!(
            changed
                .iter()
                .all(|&(x, y)| blended(three.row(y)[x], DIM) || blended(five.row(y)[x], DIM))
        );
        Ok(())
    }

    #[test]
    fn the_paused_stage_shows_a_resume_and_the_x_as_recorded() -> Outcome {
        let frame = render(SHEET, &timer(running(true)?, false)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-timer-paused.txt")
        );
        Ok(())
    }

    #[test]
    fn the_ring_empties_with_the_time_left() -> Outcome {
        let accent = |left| -> Result<usize, Box<dyn std::error::Error>> {
            let shown = Face {
                running: Some(Running {
                    left: Countdown::of(left),
                    length: Countdown::new(0, 3, 0)?,
                    paused: false,
                }),
                ..timer(None, false)?
            };
            Ok(lit(&render(PANEL, &shown))
                .into_iter()
                .filter(|&(_, y, colour)| y < PANEL.height * 9 / 10 && accented(colour))
                .count())
        };
        let (full, half, none) = (accent(180)?, accent(90)?, accent(0)?);
        assert!(full > half && half > none);
        assert!(half.abs_diff(full.midpoint(none)) < full / 20);
        Ok(())
    }

    #[test]
    fn the_running_stage_draws_no_slider_and_no_list() -> Outcome {
        let frame = render(PANEL, &timer(running(false)?, false)?);
        let list = render(PANEL, &timer(None, false)?);
        assert_ne!(frame, list);
        let layout = layout(PANEL).ok_or("no layout")?;
        let dial = dial(PANEL, &layout).ok_or("no dial")?;
        assert!(lit(&frame).iter().all(|&(_, y, _)| {
            y < layout.symbol.1 + 2 * layout.unit
                || (dial.top..dial.controls + CONTROL_UNITS * layout.unit).contains(&y)
                || y >= layout.bar_top
        }));
        Ok(())
    }

    #[test]
    fn a_zero_countdown_dims_both_badges_on_the_ring() -> Outcome {
        let idle = render(
            PANEL,
            &Face {
                countdown: Countdown::new(0, 4, 0)?,
                ..timer(None, false)?
            },
        );
        let zero = render(
            PANEL,
            &Face {
                countdown: Countdown::default(),
                ..timer(None, false)?
            },
        );
        let layout = layout(PANEL).ok_or("no layout")?;
        let board = board(PANEL, &layout).ok_or("no board")?;
        let half = ICON_UNITS * board.unit * 5 / 12;
        let shown = |frame: &Frame, (x, y): (usize, usize)| {
            (y - half..=y + half)
                .flat_map(|row| {
                    (x - half..=x + half)
                        .filter(move |&column| {
                            column.abs_diff(x).pow(2) + row.abs_diff(y).pow(2) <= half * half
                        })
                        .map(move |column| frame.row(row)[column])
                })
                .filter(|&colour| colour != BACKGROUND)
                .collect::<Vec<_>>()
        };
        for badge in [board.add(), board.save()] {
            assert_ne!(shown(&idle, badge), []);
            assert!(shown(&idle, badge).iter().all(|&colour| accented(colour)));
            assert_ne!(shown(&zero, badge), []);
            assert!(
                shown(&zero, badge)
                    .iter()
                    .all(|&colour| blended(colour, DIM))
            );
        }
        Ok(())
    }

    #[test]
    fn a_ringing_timer_shows_its_length_inside_the_halo_as_recorded() -> Outcome {
        let rung = Face {
            rung: Some(Countdown::new(0, 12, 5)?),
            ..timer(None, true)?
        };
        let frame = render(SMALL, &rung);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-timer-ringing.txt")
        );
        assert_ne!(frame, render(SMALL, &face(true, true, charged(80)?)?));
        Ok(())
    }

    #[test]
    fn the_settings_screen_shows_the_toggles_the_lit_time_the_ramp_and_the_recovery_row_as_recorded()
    -> Outcome {
        for (
            power_off,
            (lit_seconds, ramp_seconds),
            (sound, vibration),
            name,
            recovery_marked,
            recorded,
        ) in [
            (
                true,
                (10, 0),
                (true, true),
                SoundName::new("ringtone.wav"),
                false,
                include_str!("../testdata/face-settings.txt"),
            ),
            (
                false,
                (300, 120),
                (false, true),
                None,
                false,
                include_str!("../testdata/face-settings-stays.txt"),
            ),
            (
                true,
                (10, 15),
                (true, false),
                SoundName::new("Carbon.wav"),
                true,
                include_str!("../testdata/face-settings-recovery.txt"),
            ),
        ] {
            let settings = Face {
                screen: Screen::Settings,
                power_off,
                lit_seconds,
                ramp_seconds,
                outputs: Outputs { sound, vibration },
                sound: name,
                recovery_marked,
                ..face(true, false, charged(80)?)?
            };
            assert_eq!(picture(&render(SHEET, &settings)), recorded);
        }
        Ok(())
    }

    #[test]
    fn the_sound_list_shows_its_page_of_names_the_chosen_one_checked_as_recorded() -> Outcome {
        let sounds = ["Argon.wav", "Carbon.wav", "ringtone.wav"]
            .into_iter()
            .filter_map(SoundName::new)
            .collect();
        for (more, recorded) in [
            (false, include_str!("../testdata/face-settings-sounds.txt")),
            (
                true,
                include_str!("../testdata/face-settings-sounds-more.txt"),
            ),
        ] {
            let listing = Face {
                screen: Screen::Settings,
                listing: Some(Listing {
                    sounds: Vec::clone(&sounds),
                    chosen: Some(1),
                    more,
                }),
                ..face(true, false, charged(80)?)?
            };
            assert_eq!(picture(&render(SHEET, &listing)), recorded);
        }
        Ok(())
    }

    #[test]
    fn a_name_too_wide_for_its_row_is_cut_to_what_fits() {
        let height = 30;
        let room = glyph::width("Carbon", height);
        assert_eq!(fitted("Carbon", height, room), "Carbon");
        assert_eq!(fitted("CarbonCarbon", height, room), "Carbon");
        assert_eq!(fitted("Carbon", height, 0), "");
    }

    #[test]
    fn a_tap_across_a_settings_row_is_its_setting_and_elsewhere_blank() {
        let settings = |x, y| target(PANEL, Screen::Settings, Stage::List, x, y, 0);
        assert_eq!(settings(0, 725), Target::PowerOff);
        assert_eq!(settings(540, 800), Target::PowerOff);
        assert_eq!(settings(1079, 908), Target::PowerOff);
        assert_eq!(settings(0, 909), Target::LitTime);
        assert_eq!(settings(540, 1000), Target::LitTime);
        assert_eq!(settings(1079, 1092), Target::LitTime);
        assert_eq!(settings(0, 1093), Target::Sound);
        assert_eq!(settings(540, 1180), Target::Sound);
        assert_eq!(settings(1079, 1276), Target::Sound);
        assert_eq!(settings(0, 1277), Target::SoundList);
        assert_eq!(settings(540, 1364), Target::SoundList);
        assert_eq!(settings(1079, 1460), Target::SoundList);
        assert_eq!(settings(0, 1461), Target::Vibration);
        assert_eq!(settings(540, 1548), Target::Vibration);
        assert_eq!(settings(1079, 1644), Target::Vibration);
        assert_eq!(settings(0, 1645), Target::Ramp);
        assert_eq!(settings(540, 1732), Target::Ramp);
        assert_eq!(settings(1079, 1828), Target::Ramp);
        assert_eq!(settings(0, 1829), Target::Recovery);
        assert_eq!(settings(540, 1920), Target::Recovery);
        assert_eq!(settings(1079, 2012), Target::Recovery);
        assert_eq!(settings(540, 724), Target::Blank);
        assert_eq!(settings(540, 2013), Target::Blank);
        assert_eq!(settings(540, 2092), Target::Blank);
        assert_eq!(
            target(SMALL, Screen::Settings, Stage::List, 50, 20, 0),
            Target::Blank
        );
    }

    #[test]
    fn a_tap_on_the_sound_list_is_close_or_more_on_its_first_row_and_a_name_below() {
        let listing = |x, y| target(PANEL, Screen::Settings, Stage::Choosing, x, y, 0);
        assert_eq!(listing(0, 725), Target::Close);
        assert_eq!(listing(500, 908), Target::Close);
        assert_eq!(listing(1079, 725), Target::More);
        assert_eq!(listing(540, 1000), Target::Choose(0));
        assert_eq!(listing(1079, 1276), Target::Choose(1));
        assert_eq!(listing(0, 1828), Target::Choose(4));
        assert_eq!(listing(540, 1829), Target::Choose(5));
        assert_eq!(listing(1079, 2012), Target::Choose(5));
        assert_eq!(listing(540, 724), Target::Blank);
        assert_eq!(listing(540, 2013), Target::Blank);
        assert_eq!(listing(900, 2200), Target::Tab(Screen::Settings));
    }

    #[test]
    fn each_screen_marks_its_own_tab_and_the_settings_screen_alone_has_no_slider() -> Outcome {
        let mut seen = Vec::new();
        for screen in Screen::ALL {
            let face = Face {
                screen,
                ..face(true, false, charged(80)?)?
            };
            let frame = render(PANEL, &face);
            let marked: Vec<_> = lit(&frame)
                .into_iter()
                .filter(|&(_, y, colour)| y > PANEL.height * 19 / 20 && solid(colour))
                .map(|(x, _, _)| x * Screen::ALL.len() / PANEL.width)
                .collect();
            assert_ne!(marked, []);
            assert!(marked.iter().all(|&third| marked[0] == third));
            assert!(!seen.contains(&marked[0]));
            seen.push(marked[0]);
            assert_eq!(
                lit(&frame)
                    .iter()
                    .any(|&(_, y, colour)| y < PANEL.height * 9 / 10 && colour == DIM),
                screen != Screen::Settings
            );
        }
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_draws_no_bar_below_the_halo() -> Outcome {
        let frame = render(PANEL, &face(true, true, charged(64)?)?);
        let halo = halo(PANEL, &layout(PANEL).ok_or("no layout")?).ok_or("no halo")?;
        assert!(
            (halo.power + halo.icon()..PANEL.height)
                .all(|y| frame.row(y).iter().all(|&colour| colour == BACKGROUND))
        );
        Ok(())
    }

    #[test]
    fn the_halo_is_a_closed_ring_in_the_accent_on_black() -> Outcome {
        let frame = render(PANEL, &face(true, true, charged(64)?)?);
        let halo = halo(PANEL, &layout(PANEL).ok_or("no layout")?).ok_or("no halo")?;
        let (size, band) = (halo.size(), halo.unit / 4);
        let (left, middle) = (halo.centre - size / 2, halo.top + size / 2);
        for (x, y) in [
            (halo.centre, halo.top + band),
            (halo.centre, halo.top + size - band),
            (left + band, middle),
            (left + size - band, middle),
        ] {
            assert!(solid(frame.row(y)[x]));
        }
        assert_eq!(frame.row(middle)[left + size / 8], BACKGROUND);
        Ok(())
    }

    #[test]
    fn the_volume_shows_at_the_left_edge_over_a_ringing_face_as_recorded() -> Outcome {
        let frame = render(
            SHEET,
            &Face {
                volume: Volume::new(10),
                ..face(true, true, charged(80)?)?
            },
        );
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-volume-ringing.txt")
        );
        Ok(())
    }

    #[test]
    fn the_volume_fills_its_track_to_its_level_and_changes_nothing_else() -> Outcome {
        let ringing = face(true, true, charged(80)?)?;
        let plain = render(SHEET, &ringing);
        let half = layout(SHEET).ok_or("no layout")?.unit / 2;
        let track = VOLUME_TRACK_HALVES * half;
        for (level, rows) in [(1, track / 15), (5, track / 3), (15, track)] {
            let frame = render(
                SHEET,
                &Face {
                    volume: Volume::new(level),
                    ..ringing.clone()
                },
            );
            let filled = (0..SHEET.height)
                .flat_map(|y| {
                    frame.row(y)[half..(VOLUME_HALVES + 1) * half]
                        .iter()
                        .copied()
                })
                .filter(|&pixel| solid(pixel))
                .count();
            assert_eq!(filled, rows * half);
            for y in 0..SHEET.height {
                for (x, (&before, &after)) in plain.row(y).iter().zip(frame.row(y)).enumerate() {
                    if x < half || x >= half + VOLUME_HALVES * half {
                        assert_eq!(after, before);
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn a_ringing_face_with_no_alarm_rung_shows_dashes() -> Outcome {
        let none = render(PANEL, &face(false, true, charged(64)?)?);
        let some = render(PANEL, &face(true, true, charged(64)?)?);
        let halo = halo(PANEL, &layout(PANEL).ok_or("no layout")?).ok_or("no halo")?;
        let rung = halo.rung()..halo.rung() + DIGIT_HEIGHT_UNITS * halo.small();
        assert_ne!(none, some);
        for y in 0..PANEL.height {
            if !rung.contains(&y) {
                assert_eq!(none.row(y), some.row(y));
            }
        }
        let inside = halo.centre - halo.size() / 4..halo.centre + halo.size() / 4;
        assert!(
            rung.flat_map(|y| none.row(y)[inside.clone()].iter().copied())
                .filter(|&colour| colour != BACKGROUND)
                .all(|colour| blended(colour, DIM))
        );
        Ok(())
    }

    #[test]
    fn the_clock_is_drawn_with_grey_edges() -> Outcome {
        let frame = render(PANEL, &face(true, false, charged(80)?)?);
        let layout = layout(PANEL).ok_or("no layout")?;
        let board = board(PANEL, &layout).ok_or("no board")?;
        let left = board.centre - UNITS_ACROSS * board.unit / 2;
        let top = board.middle() + board.unit - DIGIT_HEIGHT_UNITS * board.unit / 2;
        let across = UNITS_ACROSS * board.unit;
        let down = DIGIT_HEIGHT_UNITS * board.unit;
        let clock: Vec<Colour> = (top..top + down)
            .flat_map(|y| frame.row(y)[left..left + across].iter().copied())
            .collect();
        assert!(clock.contains(&FOREGROUND));
        assert!(
            clock
                .iter()
                .any(|&pixel| pixel != FOREGROUND && blended(pixel, FOREGROUND))
        );
        Ok(())
    }

    #[test]
    fn a_glyph_blends_over_the_colour_beneath_it() {
        let mut frame = Frame::filled(
            Size {
                width: 1,
                height: 1,
            },
            WARNING,
        );
        frame.blend(0, 0, BACKGROUND, 0);
        assert_eq!(frame.row(0), [WARNING]);
        frame.blend(0, 0, BACKGROUND, 0x80);
        assert_eq!(frame.row(0), [Colour(0x7f_1d17)]);
        frame.blend(0, 0, FOREGROUND, u8::MAX);
        assert_eq!(frame.row(0), [FOREGROUND]);
        frame.blend(1, 0, BACKGROUND, u8::MAX);
        assert_eq!(frame.row(0), [FOREGROUND]);
    }

    #[test]
    fn the_accent_runs_from_its_first_colour_to_its_last_and_no_further() {
        assert_eq!(ACCENT.at(0, 10), ACCENT.from);
        assert_eq!(ACCENT.at(10, 10), ACCENT.to);
        assert_eq!(ACCENT.at(25, 10), ACCENT.to);
        assert_eq!(ACCENT.at(5, 0), ACCENT.from);
        let middle = ACCENT.at(5, 10);
        assert!(accented(middle));
        assert!(![ACCENT.from, ACCENT.to].contains(&middle));
        assert!(accented(ACCENT.from) && accented(ACCENT.to));
        assert!(
            [FOREGROUND, DIM, WARNING]
                .into_iter()
                .all(|colour| !accented(colour) && !accented(mixed(colour, BACKGROUND, 0x80)))
        );
    }

    #[test]
    fn the_open_tab_runs_blue_at_its_top_left_to_purple_at_its_bottom_right() -> Outcome {
        let frame = render(PANEL, &face(true, false, charged(80)?)?);
        let tab: Vec<_> = lit(&frame)
            .into_iter()
            .filter(|&(x, y, colour)| {
                x < PANEL.width / 3 && y > PANEL.height * 19 / 20 && solid(colour)
            })
            .collect();
        let corner = |&(x, y, _): &(usize, usize, Colour)| x + y;
        let first = tab.iter().min_by_key(|lit| corner(lit)).ok_or("no tab")?;
        let last = tab.iter().max_by_key(|lit| corner(lit)).ok_or("no tab")?;
        let channel = |colour: Colour, shift: u32| (colour.0 >> shift) & 0xff;
        assert!(channel(first.2, 8) > channel(first.2, 16));
        assert!(channel(last.2, 16) > channel(last.2, 8));
        Ok(())
    }

    #[test]
    fn every_digit_has_its_own_shape() -> Outcome {
        let mut seen = Vec::new();
        for value in 0..10 {
            let face = Face {
                time: TimeOfDay::new(0, value)?,
                ..face(true, false, charged(50)?)?
            };
            let shape = picture(&render(SHEET, &face));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn every_level_and_capacity_has_its_own_symbol() -> Outcome {
        let mut seen = Vec::new();
        for value in [0, 9, 20, 21, 55, 79, 80, 100] {
            let shape = picture(&render(SHEET, &face(true, false, charged(value)?)?));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn the_wheels_wrap_past_midnight_and_the_hour() -> Outcome {
        let at = |hour, minute| -> Result<Face, Box<dyn std::error::Error>> {
            Ok(Face {
                editing: Some(Alarm::once(TimeOfDay::new(hour, minute)?)),
                ..face(true, false, charged(50)?)?
            })
        };
        assert_ne!(render(SHEET, &at(23, 59)?), render(SHEET, &at(0, 0)?));
        Ok(())
    }

    fn lit(frame: &Frame) -> Vec<(usize, usize, Colour)> {
        let size = frame.size();
        (0..size.height)
            .flat_map(|y| {
                frame
                    .row(y)
                    .iter()
                    .enumerate()
                    .filter(|&(_, &colour)| colour != BACKGROUND)
                    .map(move |(x, &colour)| (x, y, colour))
            })
            .collect()
    }

    #[test]
    fn the_face_fits_the_panel_with_the_battery_in_the_top_right_corner() -> Outcome {
        for (value, colour) in [(7, WARNING), (100, FOREGROUND)] {
            let frame = render(PANEL, &face(true, false, charged(value)?)?);
            assert_eq!(frame.size(), PANEL);
            assert!((0..PANEL.height).all(|y| frame.row(y).len() == PANEL.width));
            let corner = |x: usize, y: usize| x > PANEL.width / 2 && y < 130;
            let (bar, above): (Vec<_>, Vec<_>) =
                lit(&frame).into_iter().partition(|&(_, y, _)| y >= 2093);
            let (status, rest): (Vec<_>, Vec<_>) =
                above.into_iter().partition(|&(x, y, _)| corner(x, y));
            assert!(
                bar.iter()
                    .all(|&(_, y, lit)| y < PANEL.height - 40
                        && (accented(lit) || blended(lit, DIM)))
            );
            assert!(bar.iter().any(|&(_, _, lit)| lit == DIM));
            assert_ne!(status, []);
            assert!(status.iter().all(|&(_, _, lit)| blended(lit, colour)));
            assert!(
                status
                    .iter()
                    .all(|&(x, y, _)| x < PANEL.width - 40 && y > 40)
            );
            assert!(rest.iter().all(|&(_, y, _)| y >= 138));
            assert!(rest.iter().all(|&(x, _, _)| x > 40 && x < PANEL.width - 40));
            let mixed = rest
                .iter()
                .filter(|&&(_, _, lit)| {
                    !(blended(lit, FOREGROUND)
                        || blended(lit, DIM)
                        || accented(lit)
                        || on_track(lit))
                })
                .count();
            assert!(mixed * 1_000 < rest.len());
        }
        Ok(())
    }

    #[test]
    fn the_battery_leaves_the_rest_of_the_screen_as_it_was() -> Outcome {
        let full = render(PANEL, &face(true, false, charged(100)?)?);
        let unread = render(PANEL, &face(true, false, Gauge::UNREAD)?);
        let below = PANEL.height / 10;
        assert!((below..PANEL.height).all(|y| full.row(y) == unread.row(y)));
        Ok(())
    }

    #[test]
    fn a_frame_too_small_for_the_face_stays_blank() -> Outcome {
        let short = Size {
            width: 100,
            height: 64,
        };
        let frame = render(short, &face(true, false, Gauge::UNREAD)?);
        assert_eq!(frame, Frame::filled(short, BACKGROUND));
        assert_eq!(
            target(short, Screen::Alarm, Stage::List, 10, 10, 0),
            Target::Blank
        );
        assert_eq!(
            target(short, Screen::Timer, Stage::List, 10, 10, 0),
            Target::Blank
        );
        Ok(())
    }

    #[test]
    fn a_frame_too_short_for_the_sheet_shows_only_the_bar() -> Outcome {
        let frame = render(SMALL, &face(true, false, charged(80)?)?);
        let timer = Face {
            screen: Screen::Timer,
            ..face(true, false, charged(80)?)?
        };
        assert_eq!(picture(&frame).len(), picture(&render(SMALL, &timer)).len());
        assert!(lit(&frame).iter().all(|&(_, y, _)| y > SMALL.height / 2));
        assert_eq!(
            target(SMALL, Screen::Alarm, Stage::List, 50, 20, 0),
            Target::Blank
        );
        Ok(())
    }

    #[test]
    fn a_tap_on_the_alarm_screen_is_the_plus_a_row_or_its_switch() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y, 0);
        assert_eq!(alarm(775, 718), Target::Add);
        assert_eq!(alarm(672, 615), Target::Add);
        assert_eq!(alarm(878, 821), Target::Add);
        assert_eq!(alarm(671, 718), Target::Blank);
        assert_eq!(alarm(775, 822), Target::Blank);
        assert_eq!(alarm(540, 400), Target::Blank);
        assert_eq!(alarm(540, 908), Target::Blank);
        assert_eq!(alarm(300, 909), Target::Entry(0));
        assert_eq!(alarm(815, 1092), Target::Entry(0));
        assert_eq!(alarm(816, 1000), Target::Switch(0));
        assert_eq!(alarm(300, 1093), Target::Entry(1));
        assert_eq!(alarm(1079, 1736), Target::Switch(4));
        assert_eq!(alarm(0, 1828), Target::Entry(4));
        assert_eq!(alarm(540, 1829), Target::Entry(5));
        assert_eq!(alarm(540, 2092), Target::Entry(6));
    }

    #[test]
    fn a_tap_on_a_scrolled_list_hits_the_row_drawn_under_it_and_the_ring_stays_put() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y, 1012);
        assert_eq!(alarm(775, 718), Target::Add);
        assert_eq!(alarm(540, 400), Target::Blank);
        assert_eq!(alarm(300, 896), Target::Blank);
        assert_eq!(alarm(300, 897), Target::Entry(5));
        assert_eq!(alarm(1079, 1000), Target::Switch(5));
        assert_eq!(alarm(300, 2092), Target::Entry(11));
        let timer = |x, y| target(PANEL, Screen::Timer, Stage::List, x, y, 1012);
        assert_eq!(timer(437, 480), Target::Wheel(Field::Hours, Row::Picked));
        assert_eq!(timer(775, 718), Target::Start);
        assert_eq!(timer(305, 718), Target::Add);
        assert_eq!(timer(300, 896), Target::Blank);
        assert_eq!(timer(300, 897), Target::Entry(5));
        assert_eq!(timer(1079, 2092), Target::Play(11));
    }

    #[test]
    fn a_list_scrolls_as_far_as_its_rows_run_past_the_bar_and_a_saved_row_is_brought_into_view() {
        assert_eq!(reach(PANEL, Screen::Alarm, 12), 1012);
        assert_eq!(reach(PANEL, Screen::Timer, 12), 1012);
        assert_eq!(reach(PANEL, Screen::Alarm, 7), 92);
        assert_eq!(reach(PANEL, Screen::Timer, 6), 0);
        assert_eq!(reach(PANEL, Screen::Alarm, 0), 0);
        assert_eq!(reach(SMALL, Screen::Alarm, 12), 0);
        assert_eq!(into_view(PANEL, 11, 0), 1012);
        assert_eq!(into_view(PANEL, 6, 0), 92);
        assert_eq!(into_view(PANEL, 6, 500), 500);
        assert_eq!(into_view(PANEL, 0, 500), 0);
        assert_eq!(into_view(PANEL, 2, 500), 368);
    }

    #[test]
    fn a_scrolled_alarm_list_moves_its_rows_up_under_a_fixed_ring_and_cuts_them_as_recorded()
    -> Outcome {
        let scrolled = Face {
            alarms: unarmed()?,
            scroll: 40,
            ..face(true, false, charged(80)?)?
        };
        assert_eq!(
            picture(&render(SHEET, &scrolled)),
            include_str!("../testdata/face-sheet-scrolled.txt")
        );
        Ok(())
    }

    #[test]
    fn a_scrolled_timer_list_moves_its_rows_up_under_a_fixed_ring_and_cuts_them_as_recorded()
    -> Outcome {
        let scrolled = Face {
            timers: lengths()?,
            scroll: 40,
            ..timer(None, false)?
        };
        assert_eq!(
            picture(&render(SHEET, &scrolled)),
            include_str!("../testdata/face-timer-scrolled.txt")
        );
        Ok(())
    }

    #[test]
    fn a_tap_on_the_editor_is_a_wheel_the_check_the_cross_or_a_day() {
        let editor = |x, y| target(PANEL, Screen::Alarm, Stage::Editing, x, y, 0);
        assert_eq!(editor(200, 220), Target::Wheel(Field::Hours, Row::Above));
        assert_eq!(editor(308, 400), Target::Wheel(Field::Hours, Row::Picked));
        assert_eq!(editor(309, 400), Target::Wheel(Field::Minutes, Row::Picked));
        assert_eq!(editor(450, 689), Target::Wheel(Field::Minutes, Row::Below));
        assert_eq!(editor(586, 311), Target::Done);
        assert_eq!(editor(815, 494), Target::Done);
        assert_eq!(editor(816, 400), Target::Delete);
        assert_eq!(editor(1079, 400), Target::Delete);
        assert_eq!(editor(700, 310), Target::Blank);
        assert_eq!(editor(700, 495), Target::Blank);
        assert_eq!(editor(540, 700), Target::Blank);
        assert_eq!(editor(0, 725), Target::Day(Weekday::Monday));
        assert_eq!(editor(540, 820), Target::Day(Weekday::Thursday));
        assert_eq!(editor(944, 908), Target::Day(Weekday::Sunday));
        assert_eq!(editor(1079, 800), Target::Day(Weekday::Sunday));
        for (index, day) in Weekday::ALL.into_iter().enumerate() {
            assert_eq!(editor(135 + 135 * index, 820), Target::Day(day));
        }
        assert_eq!(editor(540, 909), Target::Blank);
        assert_eq!(editor(540, 1500), Target::Blank);
        assert_eq!(editor(540, 2200), Target::Tab(Screen::Timer));
        assert_eq!(
            target(PANEL, Screen::Timer, Stage::Editing, 540, 820, 0),
            target(PANEL, Screen::Timer, Stage::List, 540, 820, 0)
        );
    }

    #[test]
    fn a_tap_on_the_bar_picks_the_tab_in_its_third_on_every_screen() {
        for screen in Screen::ALL {
            let tap = |x, y| target(PANEL, screen, Stage::List, x, y, 0);
            assert_eq!(tap(0, 2279), Target::Tab(Screen::Alarm));
            assert_eq!(tap(359, 2100), Target::Tab(Screen::Alarm));
            assert_eq!(tap(360, 2200), Target::Tab(Screen::Timer));
            assert_eq!(tap(719, 2279), Target::Tab(Screen::Timer));
            assert_eq!(tap(720, 2100), Target::Tab(Screen::Settings));
            assert_eq!(tap(1079, 2279), Target::Tab(Screen::Settings));
        }
    }

    #[test]
    fn a_tap_on_the_list_stage_is_a_wheel_a_badge_on_the_ring_a_row_or_its_play() {
        let timer = |x, y| target(PANEL, Screen::Timer, Stage::List, x, y, 0);
        assert_eq!(timer(340, 271), Target::Wheel(Field::Hours, Row::Above));
        assert_eq!(timer(437, 480), Target::Wheel(Field::Hours, Row::Picked));
        assert_eq!(timer(438, 480), Target::Wheel(Field::Minutes, Row::Picked));
        assert_eq!(timer(641, 340), Target::Wheel(Field::Minutes, Row::Above));
        assert_eq!(timer(642, 340), Target::Wheel(Field::Seconds, Row::Above));
        assert_eq!(timer(740, 600), Target::Wheel(Field::Seconds, Row::Below));
        assert_eq!(timer(340, 270), Target::Blank);
        assert_eq!(timer(775, 718), Target::Start);
        assert_eq!(timer(695, 638), Target::Start);
        assert_eq!(timer(855, 798), Target::Start);
        assert_eq!(timer(305, 718), Target::Add);
        assert_eq!(timer(225, 798), Target::Add);
        assert_eq!(timer(385, 638), Target::Add);
        assert_eq!(timer(540, 718), Target::Blank);
        assert_eq!(timer(856, 718), Target::Blank);
        assert_eq!(timer(300, 909), Target::Entry(0));
        assert_eq!(timer(815, 1092), Target::Entry(0));
        assert_eq!(timer(816, 1092), Target::Play(0));
        assert_eq!(timer(1079, 1093), Target::Play(1));
        assert_eq!(timer(0, 1644), Target::Entry(3));
        assert_eq!(timer(540, 1645), Target::Entry(4));
        assert_eq!(timer(540, 100), Target::Blank);
    }

    #[test]
    fn a_tap_on_the_running_stage_is_the_x_the_pause_or_blank() {
        let running = |x, y| target(PANEL, Screen::Timer, Stage::Running, x, y, 0);
        assert_eq!(running(0, 1506), Target::Close);
        assert_eq!(running(218, 1667), Target::Close);
        assert_eq!(running(378, 1827), Target::Close);
        assert_eq!(running(379, 1506), Target::Pause);
        assert_eq!(running(540, 1667), Target::Pause);
        assert_eq!(running(1079, 1827), Target::Pause);
        assert_eq!(running(540, 1505), Target::Blank);
        assert_eq!(running(540, 1828), Target::Blank);
        assert_eq!(running(540, 900), Target::Blank);
        assert_eq!(running(200, 820), Target::Blank);
        assert_eq!(running(540, 2200), Target::Tab(Screen::Timer));
        assert_eq!(
            target(PANEL, Screen::Alarm, Stage::Running, 300, 1000, 0),
            Target::Entry(0)
        );
    }

    #[test]
    fn a_timer_s_three_pairs_and_their_wheels_fit_inside_the_ring() -> Outcome {
        for size in [SHEET, PANEL] {
            let layout = layout(size).ok_or("no layout")?;
            let board = board(size, &layout).ok_or("no board")?;
            let picker = board.picker();
            let bottom = picker.wheel_row(Row::Below) + DIGIT_HEIGHT_UNITS * picker.unit;
            assert!(COUNTDOWN_UNITS * picker.unit <= board.size - 2 * board.unit);
            assert!(picker.wheel_top >= board.top);
            assert!(bottom <= board.top + board.size);
            assert!(COUNTDOWN_UNITS * layout.alarm_unit <= size.width);
        }
        Ok(())
    }

    #[test]
    fn a_wheel_steps_one_layout_unit_at_a_time() {
        assert_eq!(pitch(PANEL), 46);
        assert_eq!(pitch(SHEET), 4);
        assert_eq!(
            pitch(Size {
                width: 1,
                height: 1
            }),
            1
        );
    }

    const LONG: [Target; 14] = [
        Target::PowerOff,
        Target::LitTime,
        Target::Sound,
        Target::SoundList,
        Target::Vibration,
        Target::Ramp,
        Target::Recovery,
        Target::PowerOff,
        Target::LitTime,
        Target::Sound,
        Target::SoundList,
        Target::Vibration,
        Target::Ramp,
        Target::Recovery,
    ];

    #[test]
    fn the_rows_scroll_as_far_as_they_run_past_the_bar_and_not_at_all_when_they_fit() {
        assert_eq!(reach(PANEL, Screen::Settings, 0), 0);
        assert_eq!(reach_of(PANEL, &SETTINGS[..1]), 0);
        assert_eq!(reach_of(PANEL, &LONG), 736 + (14 * 8 - 1) * 23 - 2093);
        assert_eq!(reach_of(SHEET, &LONG), 64 + (14 * 8 - 1) * 2 - 211);
        assert_eq!(reach_of(SMALL, &LONG), 0);
    }

    #[test]
    fn a_tap_after_a_scroll_hits_the_row_drawn_under_it() -> Outcome {
        let layout = layout(PANEL).ok_or("no layout")?;
        let sheet = sheet(PANEL, &layout, Arrangement::Rows).ok_or("no sheet")?;
        let long = |y, scroll| settings_target(sheet, &LONG, y, scroll);
        assert_eq!(long(800, 0), Target::PowerOff);
        assert_eq!(long(800, 184), Target::LitTime);
        assert_eq!(long(2050, 0), Target::PowerOff);
        assert_eq!(long(2050, 184), Target::LitTime);
        assert_eq!(long(725, 1196), Target::Recovery);
        assert_eq!(long(1800, 1196), Target::Ramp);
        assert_eq!(long(1960, 1196), Target::Recovery);
        assert_eq!(long(2092, 1196), Target::Recovery);
        assert_eq!(long(724, 100), Target::Blank);
        assert_eq!(
            target(PANEL, Screen::Settings, Stage::List, 540, 800, 184),
            Target::LitTime
        );
        assert_eq!(
            target(PANEL, Screen::Settings, Stage::List, 900, 2200, 184),
            Target::Tab(Screen::Settings)
        );
        Ok(())
    }

    #[test]
    fn a_scrolled_list_shows_its_rows_moved_up_and_cut_at_the_band_s_edges_as_recorded() -> Outcome
    {
        let scrolled = Face {
            screen: Screen::Settings,
            scroll: 40,
            ..face(true, false, charged(80)?)?
        };
        assert_eq!(
            picture(&drawn(SHEET, &scrolled, &LONG)),
            include_str!("../testdata/face-settings-scrolled.txt")
        );
        assert_eq!(
            drawn(
                SHEET,
                &Face {
                    scroll: 0,
                    ..scrolled.clone()
                },
                &SETTINGS
            ),
            render(
                SHEET,
                &Face {
                    scroll: 0,
                    ..scrolled
                }
            )
        );
        Ok(())
    }
}
