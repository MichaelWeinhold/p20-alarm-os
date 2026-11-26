use crate::battery::{Gauge, Level};
use crate::glyph::{self, Align, Icon, Place, Square};
use crate::sound::Volume;
use crate::time::{Countdown, TimeOfDay};

pub const BACKGROUND: Colour = Colour(0x00_0000);
pub const FOREGROUND: Colour = Colour(0xff_ffff);
pub const WARNING: Colour = Colour(0xff_3b30);
pub const RINGING: Colour = Colour(0xff_9f0a);
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
const ARROW_UNITS: usize = 2;
const ARROW_GAP_UNITS: usize = 2;
const ICON_UNITS: usize = 7;
const RING_UNITS: usize = 21;
const CONTROL_UNITS: usize = 5;
const CONTROL_GAP_UNITS: usize = 3;
const BATTERY_UNITS: usize = 14;
const ICON_EM: usize = 960;
const BARS: [usize; 3] = [150, 359, 568];
const BAR_WIDTH: usize = 164;
const BAR_TOP: usize = 350;
const BAR_HEIGHT: usize = 260;
const VOLUME_HALVES: usize = 4;
const VOLUME_ICON_HALVES: usize = 3;
const VOLUME_TRACK_HALVES: usize = 24;

pub const SAVED_ALARMS: usize = 4;
pub const SAVED_TIMERS: usize = 4;

const POWER_OFF_ROW: usize = 0;
const LIT_ROW: usize = 1;
const RECOVERY_ROW: usize = 2;

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Face {
    pub time: TimeOfDay,
    pub alarms: Vec<TimeOfDay>,
    pub active: Option<usize>,
    pub marked: Option<usize>,
    pub picked: TimeOfDay,
    pub ringing: bool,
    pub battery: Gauge,
    pub screen: Screen,
    pub power_off: bool,
    pub lit_seconds: i64,
    pub recovery_marked: bool,
    pub timers: Vec<Countdown>,
    pub countdown: Countdown,
    pub running: Option<Running>,
    pub rung: Option<Countdown>,
    pub volume: Option<Volume>,
}

impl Face {
    fn active_alarm(&self) -> Option<TimeOfDay> {
        self.active
            .and_then(|index| self.alarms.get(index).copied())
    }

    fn addable(&self) -> bool {
        self.alarms.len() < SAVED_ALARMS && !self.alarms.contains(&self.picked)
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Left,
    Right,
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
    Remove(usize),
    Play(usize),
    Pause,
    Close,
    PowerOff,
    LitTime,
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
    clock: (usize, usize),
    status_unit: usize,
    symbol: (usize, usize),
    alarm_unit: usize,
    alarm: (usize, usize),
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
        clock: ((size.width - UNITS_ACROSS * unit) / 2, top),
        status_unit,
        symbol: (size.width - unit - SYMBOL_UNITS * status_unit, unit),
        alarm_unit,
        alarm: ((size.width - UNITS_ACROSS * alarm_unit) / 2, alarm_top),
        tab_unit,
        bar_top,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Sheet {
    unit: usize,
    clock: (usize, usize),
    active: (usize, usize),
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
        y + self.unit / 2 >= top && y + self.unit / 2 < top + ROW_UNITS * self.unit
    }
}

fn sheet(size: Size, layout: &Layout, screen: Screen) -> Option<Sheet> {
    let unit = layout.alarm_unit;
    let clock_top = 3 * layout.unit;
    let active_top = clock_top + DIGIT_HEIGHT_UNITS * layout.unit + 2 * unit;
    let (wheel_top, columns) = if screen == Screen::Timer {
        (clock_top, 2)
    } else {
        (
            active_top + (DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit,
            1,
        )
    };
    let list_top = wheel_top + (2 * ROW_UNITS + DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit;
    let list_bottom = list_top + (SAVED_ALARMS * ROW_UNITS - 1) * unit;
    let across = (UNITS_ACROSS + columns * (SIDE_GAP_UNITS + SIDE_UNITS)) * unit;
    if list_bottom > layout.bar_top || across > size.width {
        return None;
    }
    let left = (size.width - across) / 2;
    let beside = left + (UNITS_ACROSS + SIDE_GAP_UNITS) * unit;
    Some(Sheet {
        unit,
        clock: (layout.clock.0, clock_top),
        active: (layout.alarm.0, active_top),
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

fn of_day(time: TimeOfDay) -> (u8, u8) {
    (time.hour(), time.minute())
}

fn of_countdown(countdown: Countdown) -> (u8, u8) {
    (countdown.minutes(), countdown.seconds())
}

fn time(
    frame: &mut Frame,
    at: (usize, usize),
    unit: usize,
    value: Option<TimeOfDay>,
    colour: Colour,
) {
    figures(frame, at, unit, value.map(of_day), colour);
}

fn figures(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    value: Option<(u8, u8)>,
    colour: Colour,
) {
    colon(frame, (x + COLON_UNITS * unit, y), unit, colour);
    let Some(value) = value else {
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
        return;
    };
    pair(frame, (x, y), unit, value.0, colour);
    pair(frame, (x + MINUTE_UNITS * unit, y), unit, value.1, colour);
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
    let left = centre2.saturating_sub(size) / 2;
    let top = middle2.saturating_sub(size) / 2;
    glyph::icon(icon, square, |x, y, alpha| {
        let colour = match ink {
            Ink::Flat(colour) => colour,
            Ink::Accent => ACCENT.at(x.saturating_sub(left) + y.saturating_sub(top), 2 * size),
        };
        frame.blend(x, y, colour, alpha);
    });
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

fn settings_screen(frame: &mut Frame, sheet: Sheet, face: &Face) {
    let unit = sheet.unit;
    let right = sheet.side + SIDE_UNITS * unit;
    let top = sheet.setting_row(POWER_OFF_ROW);
    symbol(
        frame,
        Icon::Power,
        (sheet.left, top),
        unit,
        Ink::Flat(FOREGROUND),
    );
    switch(frame, (right, top), unit, face.power_off);
    let top = sheet.setting_row(LIT_ROW);
    symbol(
        frame,
        Icon::Light,
        (sheet.left, top),
        unit,
        Ink::Flat(FOREGROUND),
    );
    figures(
        frame,
        (right - UNITS_ACROSS * unit, top),
        unit,
        Some(of_countdown(Countdown::of(face.lit_seconds))),
        FOREGROUND,
    );
    let top = sheet.setting_row(RECOVERY_ROW);
    let ink = Ink::Flat(if face.recovery_marked {
        WARNING
    } else {
        FOREGROUND
    });
    symbol(frame, Icon::Recovery, (sheet.left, top), unit, ink);
    symbol(
        frame,
        Icon::Restart,
        (right - ICON_UNITS * unit, top),
        unit,
        ink,
    );
}

fn wheels(frame: &mut Frame, sheet: Sheet, picked: (u8, u8)) {
    let unit = sheet.unit;
    let y = sheet.wheel_row(Row::Picked);
    figures(frame, (sheet.left, y), unit, Some(picked), FOREGROUND);
    let above = 2 * (y - (ARROW_UNITS + ARROW_GAP_UNITS) * unit) + ARROW_UNITS * unit;
    let below = 2 * (y + (DIGIT_HEIGHT_UNITS + ARROW_GAP_UNITS) * unit) + ARROW_UNITS * unit;
    let size = ICON_UNITS * unit;
    for start in [0, MINUTE_UNITS * unit] {
        let centre2 = 2 * (sheet.left + start) + (2 * DIGIT_UNITS + 1) * unit;
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

fn slider(
    frame: &mut Frame,
    sheet: Sheet,
    picked: (u8, u8),
    addable: bool,
    entries: &[((u8, u8), bool)],
    marked: Option<usize>,
) {
    let unit = sheet.unit;
    wheels(frame, sheet, picked);
    symbol(
        frame,
        Icon::Add,
        (sheet.side, sheet.wheel_row(Row::Picked)),
        unit,
        accent(addable),
    );
    for (index, &(entry, on)) in entries.iter().take(SAVED_ALARMS).enumerate() {
        let top = sheet.list_row(index);
        figures(frame, (sheet.left, top), unit, Some(entry), shade(on));
        let mark = if marked == Some(index) { WARNING } else { DIM };
        symbol(frame, Icon::Close, (sheet.far, top), unit, Ink::Flat(mark));
    }
}

fn alarm_screen(frame: &mut Frame, layout: &Layout, sheet: Sheet, face: &Face) {
    time(frame, sheet.clock, layout.unit, Some(face.time), FOREGROUND);
    let active = face.active_alarm();
    time(
        frame,
        sheet.active,
        sheet.unit,
        active,
        shade(active.is_some()),
    );
    let entries: Vec<_> = face
        .alarms
        .iter()
        .enumerate()
        .map(|(index, &alarm)| (of_day(alarm), face.active == Some(index)))
        .collect();
    slider(
        frame,
        sheet,
        of_day(face.picked),
        face.addable(),
        &entries,
        face.marked,
    );
}

fn list_stage(frame: &mut Frame, sheet: Sheet, face: &Face) {
    let entries: Vec<_> = face
        .timers
        .iter()
        .map(|&timer| (of_countdown(timer), true))
        .collect();
    slider(
        frame,
        sheet,
        of_countdown(face.countdown),
        face.timer_addable(),
        &entries,
        face.marked,
    );
    for index in 0..entries.len().min(SAVED_TIMERS) {
        symbol(
            frame,
            Icon::Play,
            (sheet.side, sheet.list_row(index)),
            sheet.unit,
            Ink::Accent,
        );
    }
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
    glyph::ring(square, unit / 2, (1, 1), |x, y, alpha| {
        frame.blend(x, y, DIM, alpha);
    });
    let share = (running.left.total(), running.length.total());
    glyph::ring(square, unit / 2, share, |x, y, alpha| {
        let colour = ACCENT.at(
            x.saturating_sub(left) + y.saturating_sub(dial.top),
            2 * size,
        );
        frame.blend(x, y, colour, alpha);
    });
    let figure = unit * 3 / 4;
    figures(
        frame,
        (
            dial.centre.saturating_sub(UNITS_ACROSS * figure / 2),
            (dial.top + size / 2).saturating_sub(DIGIT_HEIGHT_UNITS * figure / 2),
        ),
        figure,
        Some(of_countdown(running.left)),
        FOREGROUND,
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

fn timer_screen(frame: &mut Frame, layout: &Layout, sheet: Sheet, face: &Face) {
    match (face.running, dial(frame.size(), layout)) {
        (Some(running), Some(dial)) => running_stage(frame, dial, running),
        (Some(_), None) => {}
        (None, _) => list_stage(frame, sheet, face),
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
    let background = if face.ringing { RINGING } else { BACKGROUND };
    let mut frame = Frame::filled(size, background);
    let Some(layout) = layout(size) else {
        return frame;
    };
    let digits = if face.ringing { BACKGROUND } else { FOREGROUND };
    let sheet = sheet(size, &layout, face.screen);
    if face.ringing || (sheet.is_some() && face.screen != Screen::Settings) {
        let status = if face.battery.level == Level::Low {
            WARNING
        } else {
            digits
        };
        battery_symbol(
            &mut frame,
            layout.symbol,
            layout.status_unit,
            face.battery,
            status,
        );
    }
    if face.ringing {
        time(
            &mut frame,
            layout.clock,
            layout.unit,
            Some(face.time),
            digits,
        );
        let rang = face.rung.map_or_else(
            || face.active_alarm().map(of_day),
            |rung| Some(of_countdown(rung)),
        );
        figures(&mut frame, layout.alarm, layout.alarm_unit, rang, digits);
        volume_panel(&mut frame, layout.unit, face.volume);
        return frame;
    }
    match (face.screen, sheet) {
        (Screen::Alarm, Some(sheet)) => alarm_screen(&mut frame, &layout, sheet, face),
        (Screen::Timer, Some(sheet)) => timer_screen(&mut frame, &layout, sheet, face),
        (Screen::Settings, Some(sheet)) => settings_screen(&mut frame, sheet, face),
        _ => {}
    }
    bar(&mut frame, &layout, face.screen);
    volume_panel(&mut frame, layout.unit, face.volume);
    frame
}

#[must_use]
pub fn pitch(size: Size) -> usize {
    layout(size).map_or(1, |layout| layout.unit.max(1))
}

fn sheet_target(sheet: Sheet, x: usize, y: usize) -> Target {
    let unit = sheet.unit;
    let side = x + 3 * unit / 2 >= sheet.side;
    let wheel_bottom = sheet.wheel_row(Row::Below) + (DIGIT_HEIGHT_UNITS + 1) * unit;
    if y + unit >= sheet.wheel_top && y < wheel_bottom {
        if side {
            return if sheet.band(sheet.wheel_row(Row::Picked), y) {
                Target::Add
            } else {
                Target::Blank
            };
        }
        let field = if x < sheet.left + COLON_UNITS * unit + unit / 2 {
            Field::Left
        } else {
            Field::Right
        };
        let row = Row::ALL
            .into_iter()
            .find(|&row| sheet.band(sheet.wheel_row(row), y))
            .unwrap_or(if y < sheet.wheel_top {
                Row::Above
            } else {
                Row::Below
            });
        return Target::Wheel(field, row);
    }
    match (0..SAVED_ALARMS).find(|&index| sheet.band(sheet.list_row(index), y)) {
        Some(index) if x + 3 * unit / 2 >= sheet.far => Target::Remove(index),
        Some(index) if side => Target::Play(index),
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

#[must_use]
pub fn target(size: Size, screen: Screen, stage: Stage, x: usize, y: usize) -> Target {
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
    if (screen, stage) == (Screen::Timer, Stage::Running) {
        return dial(size, &layout).map_or(Target::Blank, |dial| dial_target(dial, x, y));
    }
    match (screen, sheet(size, &layout, screen)) {
        (Screen::Alarm | Screen::Timer, Some(sheet)) => sheet_target(sheet, x, y),
        (Screen::Settings, Some(sheet)) if sheet.band(sheet.setting_row(POWER_OFF_ROW), y) => {
            Target::PowerOff
        }
        (Screen::Settings, Some(sheet)) if sheet.band(sheet.setting_row(LIT_ROW), y) => {
            Target::LitTime
        }
        (Screen::Settings, Some(sheet)) if sheet.band(sheet.setting_row(RECOVERY_ROW), y) => {
            Target::Recovery
        }
        _ => Target::Blank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{ChargeLimit, Percent};
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

    const PALETTE: [(Colour, char); 5] = [
        (BACKGROUND, '.'),
        (FOREGROUND, '#'),
        (WARNING, '!'),
        (RINGING, '~'),
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

    fn face(
        active: Option<usize>,
        ringing: bool,
        battery: Gauge,
    ) -> Result<Face, Box<dyn std::error::Error>> {
        Ok(Face {
            time: TimeOfDay::new(19, 37)?,
            alarms: vec![TimeOfDay::new(6, 45)?, TimeOfDay::new(7, 30)?],
            active,
            marked: None,
            picked: TimeOfDay::new(7, 15)?,
            ringing,
            battery,
            screen: Screen::Alarm,
            power_off: true,
            lit_seconds: 10,
            recovery_marked: false,
            timers: Vec::new(),
            countdown: Countdown::default(),
            running: None,
            rung: None,
            volume: None,
        })
    }

    #[test]
    fn the_alarm_screen_shows_the_active_alarm_the_slider_and_the_list_as_recorded() -> Outcome {
        let frame = render(SHEET, &face(Some(0), false, charged(80)?)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-sheet.txt"));
        Ok(())
    }

    #[test]
    fn with_no_active_alarm_and_a_full_list_the_alarm_and_the_add_are_dim() -> Outcome {
        let full = Face {
            alarms: [(5, 0), (6, 45), (7, 30), (9, 0)]
                .into_iter()
                .map(|(hour, minute)| TimeOfDay::new(hour, minute))
                .collect::<Result<_, _>>()?,
            ..face(None, false, charged(7)?)?
        };
        let frame = render(SHEET, &full);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-sheet-full.txt")
        );
        Ok(())
    }

    #[test]
    fn an_empty_list_shows_dashes_and_an_unread_battery_the_low_symbol() -> Outcome {
        let empty = Face {
            alarms: Vec::new(),
            ..face(None, false, Gauge::UNREAD)?
        };
        let frame = render(SHEET, &empty);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-sheet-empty.txt")
        );
        Ok(())
    }

    #[test]
    fn a_marked_cross_alone_turns_red() -> Outcome {
        let plain = render(SHEET, &face(Some(0), false, charged(80)?)?);
        let marked = render(
            SHEET,
            &Face {
                marked: Some(1),
                ..face(Some(0), false, charged(80)?)?
            },
        );
        let layout = layout(SHEET).ok_or("no layout")?;
        let sheet = sheet(SHEET, &layout, Screen::Alarm).ok_or("no sheet")?;
        let across = SIDE_UNITS * sheet.unit;
        let top = sheet.list_row(1);
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
    fn a_ringing_alarm_turns_the_face_amber() -> Outcome {
        let frame = render(SMALL, &face(Some(0), true, charged(64)?)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-ringing.txt")
        );
        Ok(())
    }

    fn timer(running: Option<Running>, ringing: bool) -> Result<Face, Box<dyn std::error::Error>> {
        Ok(Face {
            screen: Screen::Timer,
            timers: vec![Countdown::new(3, 0)?, Countdown::new(12, 5)?],
            countdown: Countdown::new(3, 0)?,
            running,
            ..face(Some(0), ringing, charged(80)?)?
        })
    }

    fn running(paused: bool) -> Result<Option<Running>, Box<dyn std::error::Error>> {
        Ok(Some(Running {
            left: Countdown::new(1, 7)?,
            length: Countdown::new(3, 0)?,
            paused,
        }))
    }

    #[test]
    fn the_list_stage_shows_the_slider_and_each_saved_timer_with_a_play_and_a_cross_as_recorded()
    -> Outcome {
        let frame = render(SHEET, &timer(None, false)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-timer.txt"));
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
                    length: Countdown::new(3, 0)?,
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
    fn a_zero_countdown_dims_the_add() -> Outcome {
        let idle = render(
            SHEET,
            &Face {
                countdown: Countdown::new(4, 0)?,
                ..timer(None, false)?
            },
        );
        let zero = render(
            SHEET,
            &Face {
                countdown: Countdown::default(),
                ..timer(None, false)?
            },
        );
        let layout = layout(SHEET).ok_or("no layout")?;
        let sheet = sheet(SHEET, &layout, Screen::Timer).ok_or("no sheet")?;
        let across = SIDE_UNITS * sheet.unit;
        let shown = |frame: &Frame, top: usize| {
            (top..top + across)
                .flat_map(|y| frame.row(y)[sheet.side..sheet.side + across].to_vec())
                .filter(|&colour| colour != BACKGROUND)
                .collect::<Vec<_>>()
        };
        let top = sheet.wheel_row(Row::Picked);
        assert_ne!(shown(&idle, top), []);
        assert!(shown(&idle, top).iter().all(|&colour| accented(colour)));
        assert_ne!(shown(&zero, top), []);
        assert!(shown(&zero, top).iter().all(|&colour| blended(colour, DIM)));
        Ok(())
    }

    #[test]
    fn a_ringing_timer_shows_its_length_in_amber_as_recorded() -> Outcome {
        let rung = Face {
            rung: Some(Countdown::new(12, 5)?),
            ..timer(None, true)?
        };
        let frame = render(SMALL, &rung);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-timer-ringing.txt")
        );
        assert_ne!(frame, render(SMALL, &face(Some(0), true, charged(80)?)?));
        Ok(())
    }

    #[test]
    fn the_settings_screen_shows_the_toggle_the_lit_time_and_the_recovery_row_as_recorded()
    -> Outcome {
        for (power_off, lit_seconds, recovery_marked, recorded) in [
            (
                true,
                10,
                false,
                include_str!("../testdata/face-settings.txt"),
            ),
            (
                false,
                300,
                false,
                include_str!("../testdata/face-settings-stays.txt"),
            ),
            (
                true,
                10,
                true,
                include_str!("../testdata/face-settings-recovery.txt"),
            ),
        ] {
            let settings = Face {
                screen: Screen::Settings,
                power_off,
                lit_seconds,
                recovery_marked,
                ..face(Some(0), false, charged(80)?)?
            };
            assert_eq!(picture(&render(SHEET, &settings)), recorded);
        }
        Ok(())
    }

    #[test]
    fn a_tap_across_a_settings_row_is_its_setting_and_elsewhere_blank() {
        let settings = |x, y| target(PANEL, Screen::Settings, Stage::List, x, y);
        assert_eq!(settings(0, 725), Target::PowerOff);
        assert_eq!(settings(540, 800), Target::PowerOff);
        assert_eq!(settings(1079, 908), Target::PowerOff);
        assert_eq!(settings(0, 909), Target::LitTime);
        assert_eq!(settings(540, 1000), Target::LitTime);
        assert_eq!(settings(1079, 1092), Target::LitTime);
        assert_eq!(settings(0, 1093), Target::Recovery);
        assert_eq!(settings(540, 1180), Target::Recovery);
        assert_eq!(settings(1079, 1276), Target::Recovery);
        assert_eq!(settings(540, 724), Target::Blank);
        assert_eq!(settings(540, 1277), Target::Blank);
        assert_eq!(settings(540, 1400), Target::Blank);
        assert_eq!(
            target(SMALL, Screen::Settings, Stage::List, 50, 20),
            Target::Blank
        );
    }

    #[test]
    fn each_screen_marks_its_own_tab_and_the_settings_screen_alone_has_no_slider() -> Outcome {
        let mut seen = Vec::new();
        for screen in Screen::ALL {
            let face = Face {
                screen,
                ..face(Some(0), false, charged(80)?)?
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
    fn a_ringing_alarm_takes_the_whole_panel_with_no_bar() -> Outcome {
        let frame = render(PANEL, &face(Some(0), true, charged(64)?)?);
        assert!(
            (PANEL.height * 9 / 10..PANEL.height)
                .all(|y| frame.row(y).iter().all(|&colour| colour == RINGING))
        );
        Ok(())
    }

    #[test]
    fn the_volume_shows_at_the_left_edge_over_a_ringing_face_as_recorded() -> Outcome {
        let frame = render(
            SHEET,
            &Face {
                volume: Volume::new(10),
                ..face(Some(0), true, charged(80)?)?
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
        let ringing = face(Some(0), true, charged(80)?)?;
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
                .flat_map(|y| frame.row(y).iter().copied())
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
    fn a_ringing_face_with_no_active_alarm_shows_dashes() -> Outcome {
        let none = render(SMALL, &face(None, true, charged(64)?)?);
        let some = render(SMALL, &face(Some(0), true, charged(64)?)?);
        assert_ne!(none, some);
        assert!(
            lit(&none)
                .iter()
                .all(|&(_, _, colour)| colour != FOREGROUND)
        );
        Ok(())
    }

    #[test]
    fn the_clock_is_drawn_with_grey_edges() -> Outcome {
        let frame = render(PANEL, &face(Some(0), false, charged(80)?)?);
        let layout = layout(PANEL).ok_or("no layout")?;
        let (left, top) = layout.clock;
        let across = UNITS_ACROSS * layout.unit;
        let down = DIGIT_HEIGHT_UNITS * layout.unit;
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
            RINGING,
        );
        frame.blend(0, 0, BACKGROUND, 0);
        assert_eq!(frame.row(0), [RINGING]);
        frame.blend(0, 0, BACKGROUND, 0x80);
        assert_eq!(frame.row(0), [Colour(0x7f_4f04)]);
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
            [FOREGROUND, DIM, WARNING, RINGING]
                .into_iter()
                .all(|colour| !accented(colour) && !accented(mixed(colour, BACKGROUND, 0x80)))
        );
    }

    #[test]
    fn the_open_tab_runs_blue_at_its_top_left_to_purple_at_its_bottom_right() -> Outcome {
        let frame = render(PANEL, &face(Some(0), false, charged(80)?)?);
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
                ..face(Some(0), false, charged(50)?)?
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
            let shape = picture(&render(SHEET, &face(Some(0), false, charged(value)?)?));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn the_wheels_wrap_past_midnight_and_the_hour() -> Outcome {
        let wrapped = Face {
            picked: TimeOfDay::new(23, 59)?,
            ..face(Some(0), false, charged(50)?)?
        };
        let zero = Face {
            picked: TimeOfDay::new(0, 0)?,
            ..face(Some(0), false, charged(50)?)?
        };
        assert_ne!(render(SHEET, &wrapped), render(SHEET, &zero));
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
            let frame = render(PANEL, &face(Some(0), false, charged(value)?)?);
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
            assert!(rest.iter().all(|&(_, _, lit)| blended(lit, FOREGROUND)
                || blended(lit, DIM)
                || accented(lit)));
        }
        Ok(())
    }

    #[test]
    fn the_battery_leaves_the_rest_of_the_screen_as_it_was() -> Outcome {
        let full = render(PANEL, &face(Some(0), false, charged(100)?)?);
        let unread = render(PANEL, &face(Some(0), false, Gauge::UNREAD)?);
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
        let frame = render(short, &face(Some(0), false, Gauge::UNREAD)?);
        assert_eq!(frame, Frame::filled(short, BACKGROUND));
        assert_eq!(
            target(short, Screen::Alarm, Stage::List, 10, 10),
            Target::Blank
        );
        assert_eq!(
            target(short, Screen::Timer, Stage::List, 10, 10),
            Target::Blank
        );
        Ok(())
    }

    #[test]
    fn a_frame_too_short_for_the_sheet_shows_only_the_bar() -> Outcome {
        let frame = render(SMALL, &face(Some(0), false, charged(80)?)?);
        let timer = Face {
            screen: Screen::Timer,
            ..face(Some(0), false, charged(80)?)?
        };
        assert_eq!(picture(&frame).len(), picture(&render(SMALL, &timer)).len());
        assert!(lit(&frame).iter().all(|&(_, y, _)| y > SMALL.height / 2));
        assert_eq!(
            target(SMALL, Screen::Alarm, Stage::List, 50, 20),
            Target::Blank
        );
        Ok(())
    }

    #[test]
    fn a_tap_on_a_wheel_picks_its_field_and_row() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y);
        assert_eq!(alarm(300, 720), Target::Wheel(Field::Left, Row::Above));
        assert_eq!(alarm(423, 800), Target::Wheel(Field::Left, Row::Above));
        assert_eq!(alarm(424, 800), Target::Wheel(Field::Right, Row::Above));
        assert_eq!(alarm(300, 1000), Target::Wheel(Field::Left, Row::Picked));
        assert_eq!(alarm(600, 1000), Target::Wheel(Field::Right, Row::Picked));
        assert_eq!(alarm(0, 1200), Target::Wheel(Field::Left, Row::Below));
        assert_eq!(alarm(700, 1287), Target::Wheel(Field::Right, Row::Below));
    }

    #[test]
    fn a_tap_beside_the_picked_row_adds_and_beside_another_is_blank() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y);
        assert_eq!(alarm(701, 1000), Target::Add);
        assert_eq!(alarm(1079, 909), Target::Add);
        assert_eq!(alarm(800, 800), Target::Blank);
        assert_eq!(alarm(800, 1200), Target::Blank);
    }

    #[test]
    fn a_tap_on_a_list_row_picks_the_entry_or_its_cross() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y);
        assert_eq!(alarm(300, 1323), Target::Entry(0));
        assert_eq!(alarm(300, 1506), Target::Entry(0));
        assert_eq!(alarm(300, 1507), Target::Entry(1));
        assert_eq!(alarm(700, 1800), Target::Entry(2));
        assert_eq!(alarm(701, 1800), Target::Remove(2));
        assert_eq!(alarm(1000, 2058), Target::Remove(3));
    }

    #[test]
    fn a_tap_between_the_parts_of_the_sheet_is_blank() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, Stage::List, x, y);
        assert_eq!(alarm(540, 300), Target::Blank);
        assert_eq!(alarm(540, 600), Target::Blank);
        assert_eq!(alarm(540, 1300), Target::Blank);
        assert_eq!(alarm(540, 2059), Target::Blank);
        assert_eq!(alarm(540, 2092), Target::Blank);
    }

    #[test]
    fn a_tap_on_the_bar_picks_the_tab_in_its_third_on_every_screen() {
        for screen in Screen::ALL {
            let tap = |x, y| target(PANEL, screen, Stage::List, x, y);
            assert_eq!(tap(0, 2279), Target::Tab(Screen::Alarm));
            assert_eq!(tap(359, 2100), Target::Tab(Screen::Alarm));
            assert_eq!(tap(360, 2200), Target::Tab(Screen::Timer));
            assert_eq!(tap(719, 2279), Target::Tab(Screen::Timer));
            assert_eq!(tap(720, 2100), Target::Tab(Screen::Settings));
            assert_eq!(tap(1079, 2279), Target::Tab(Screen::Settings));
        }
    }

    #[test]
    fn a_tap_on_the_list_stage_is_the_slider_a_play_or_a_cross() {
        let timer = |x, y| target(PANEL, Screen::Timer, Stage::List, x, y);
        assert_eq!(timer(200, 220), Target::Wheel(Field::Left, Row::Above));
        assert_eq!(timer(450, 220), Target::Wheel(Field::Right, Row::Above));
        assert_eq!(timer(200, 400), Target::Wheel(Field::Left, Row::Picked));
        assert_eq!(timer(450, 678), Target::Wheel(Field::Right, Row::Below));
        assert_eq!(timer(586, 400), Target::Add);
        assert_eq!(timer(1079, 311), Target::Add);
        assert_eq!(timer(700, 220), Target::Blank);
        assert_eq!(timer(300, 725), Target::Entry(0));
        assert_eq!(timer(585, 820), Target::Entry(0));
        assert_eq!(timer(586, 820), Target::Play(0));
        assert_eq!(timer(815, 908), Target::Play(0));
        assert_eq!(timer(815, 909), Target::Play(1));
        assert_eq!(timer(816, 1000), Target::Remove(1));
        assert_eq!(timer(1079, 1460), Target::Remove(3));
        assert_eq!(timer(540, 1461), Target::Blank);
        assert_eq!(timer(540, 100), Target::Blank);
    }

    #[test]
    fn a_tap_on_the_running_stage_is_the_x_the_pause_or_blank() {
        let running = |x, y| target(PANEL, Screen::Timer, Stage::Running, x, y);
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
            target(PANEL, Screen::Alarm, Stage::Running, 300, 1400),
            Target::Entry(0)
        );
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
}
