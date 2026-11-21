use crate::battery::{Gauge, Level};
use crate::time::TimeOfDay;

pub const BACKGROUND: Colour = Colour(0x00_0000);
pub const FOREGROUND: Colour = Colour(0xff_ffff);
pub const WARNING: Colour = Colour(0xff_3b30);
pub const RINGING: Colour = Colour(0xff_9f0a);
pub const DIM: Colour = Colour(0x63_6366);

const UNITS_ACROSS: usize = 21;
const DIGIT_UNITS: usize = 4;
const DIGIT_HEIGHT_UNITS: usize = 7;
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
const DASH: u8 = 0b100_0000;
const STROKE_PARTS: usize = 3;
const ARROW_UNITS: usize = 2;
const ARROW_GAP_UNITS: usize = 2;
const SWITCH_UNITS: usize = 13;
const KNOB_UNITS: usize = 3;

pub const SAVED_ALARMS: usize = 4;

const BELL: [&str; TAB_UNITS] = [
    "....#....",
    "...###...",
    "..#####..",
    "..#####..",
    "..#####..",
    ".#######.",
    "#########",
    ".........",
    "...###...",
];
const HOURGLASS: [&str; TAB_UNITS] = [
    "#########",
    ".#######.",
    "..#####..",
    "...###...",
    "....#....",
    "...###...",
    "..#####..",
    ".#######.",
    "#########",
];
const GEAR: [&str; TAB_UNITS] = [
    "...###...",
    ".#.###.#.",
    ".#######.",
    "####.####",
    "###...###",
    "####.####",
    ".#######.",
    ".#.###.#.",
    "...###...",
];

const POWER: [&str; DIGIT_HEIGHT_UNITS] = [
    "...#...", ".#.#.#.", "#..#..#", "#..#..#", "#.....#", "#.....#", ".#####.",
];

const SEGMENTS: [u8; 10] = [
    0b011_1111, 0b000_0110, 0b101_1011, 0b100_1111, 0b110_0110, 0b110_1101, 0b111_1101, 0b000_0111,
    0b111_1111, 0b110_1111,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Colour(pub u32);

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

    fn symbol(self) -> [&'static str; TAB_UNITS] {
        match self {
            Self::Alarm => BELL,
            Self::Timer => HOURGLASS,
            Self::Settings => GEAR,
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
}

impl Face {
    fn active_alarm(&self) -> Option<TimeOfDay> {
        self.active
            .and_then(|index| self.alarms.get(index).copied())
    }

    fn addable(&self) -> bool {
        self.alarms.len() < SAVED_ALARMS && !self.alarms.contains(&self.picked)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Hour,
    Minute,
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
            Self::Above => -1,
            Self::Picked => 0,
            Self::Below => 1,
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
    PowerOff,
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
    let status_unit = (unit / 4).max(1);
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

    fn setting_row(self) -> usize {
        self.wheel_top
    }

    fn band(self, top: usize, y: usize) -> bool {
        y + self.unit / 2 >= top && y + self.unit / 2 < top + ROW_UNITS * self.unit
    }
}

fn sheet(size: Size, layout: &Layout) -> Option<Sheet> {
    let unit = layout.alarm_unit;
    let clock_top = 3 * layout.unit;
    let active_top = clock_top + DIGIT_HEIGHT_UNITS * layout.unit + 2 * unit;
    let wheel_top = active_top + (DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit;
    let list_top = wheel_top + (2 * ROW_UNITS + DIGIT_HEIGHT_UNITS + SECTION_GAP_UNITS) * unit;
    let list_bottom = list_top + (SAVED_ALARMS * ROW_UNITS - 1) * unit;
    let across = (UNITS_ACROSS + SIDE_GAP_UNITS + SIDE_UNITS) * unit;
    if list_bottom > layout.bar_top || across > size.width {
        return None;
    }
    let left = (size.width - across) / 2;
    Some(Sheet {
        unit,
        clock: (layout.clock.0, clock_top),
        active: (layout.alarm.0, active_top),
        left,
        side: left + (UNITS_ACROSS + SIDE_GAP_UNITS) * unit,
        wheel_top,
        list_top,
    })
}

fn stroke(unit: usize) -> usize {
    (unit / STROKE_PARTS).max(1)
}

fn slanted(
    frame: &mut Frame,
    (x, y): (usize, usize),
    (across, down): (usize, usize),
    unit: usize,
    rising: bool,
    colour: Colour,
) {
    let thick = stroke(unit);
    let reach = across.saturating_sub(thick);
    let last = down.saturating_sub(1).max(1);
    for row in 0..down {
        let step = if rising { last - row.min(last) } else { row };
        frame.fill(x + step * reach / last, y + row, thick, 1, colour);
    }
}

fn digit(frame: &mut Frame, x: usize, y: usize, unit: usize, value: u8, colour: Colour) {
    segmented(frame, x, y, unit, SEGMENTS[usize::from(value % 10)], colour);
}

fn segmented(frame: &mut Frame, x: usize, y: usize, unit: usize, segments: u8, colour: Colour) {
    let thick = stroke(unit);
    let width = DIGIT_UNITS * unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    let right = x + width - thick;
    let middle = y + (height - thick) / 2;
    let upper = middle + thick - y;
    let lower = y + height - middle;
    let bars = [
        (x, y, width, thick),
        (right, y, thick, upper),
        (right, middle, thick, lower),
        (x, y + height - thick, width, thick),
        (x, middle, thick, lower),
        (x, y, thick, upper),
        (x, middle, width, thick),
    ];
    for (index, (left, top, across, down)) in bars.into_iter().enumerate() {
        if segments & (1 << index) != 0 {
            frame.fill(left, top, across, down, colour);
        }
    }
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

fn colon(frame: &mut Frame, (x, y): (usize, usize), unit: usize, colour: Colour) {
    let thick = stroke(unit);
    let inset = (unit - thick) / 2;
    frame.fill(x + inset, y + 2 * unit + inset, thick, thick, colour);
    frame.fill(x + inset, y + 4 * unit + inset, thick, thick, colour);
}

fn time(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    value: Option<TimeOfDay>,
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
            segmented(frame, x + figure * unit, y, unit, DASH, colour);
        }
        return;
    };
    pair(frame, (x, y), unit, value.hour(), colour);
    pair(
        frame,
        (x + MINUTE_UNITS * unit, y),
        unit,
        value.minute(),
        colour,
    );
}

fn percent_sign(frame: &mut Frame, x: usize, y: usize, unit: usize, colour: Colour) {
    frame.fill(x, y, unit, unit, colour);
    frame.fill(x + 3 * unit, y + 6 * unit, unit, unit, colour);
    slanted(
        frame,
        (x, y),
        (PERCENT_UNITS * unit, DIGIT_HEIGHT_UNITS * unit),
        unit,
        true,
        colour,
    );
}

fn battery_symbol(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    gauge: Gauge,
    colour: Colour,
) {
    let thick = stroke(unit);
    let width = (SYMBOL_UNITS - 1) * unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    frame.fill(x, y, width, thick, colour);
    frame.fill(x, y + height - thick, width, thick, colour);
    frame.fill(x, y, thick, height, colour);
    frame.fill(x + width - thick, y, thick, height, colour);
    frame.fill(x + width, y + 2 * unit, unit, 3 * unit, colour);
    let bars = match gauge.level {
        Level::Low => 1,
        Level::Middle => 2,
        Level::Full => 3,
    };
    for bar in 0..bars {
        frame.fill(
            x + (2 + 3 * bar) * unit,
            y + 2 * unit,
            2 * unit,
            3 * unit,
            colour,
        );
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

fn symbol(frame: &mut Frame, (x, y): (usize, usize), unit: usize, rows: &[&str], colour: Colour) {
    for (row, line) in rows.iter().enumerate() {
        for (column, cell) in line.bytes().enumerate() {
            if cell == b'#' {
                frame.fill(x + column * unit, y + row * unit, unit, unit, colour);
            }
        }
    }
}

fn plus(frame: &mut Frame, (x, y): (usize, usize), unit: usize, colour: Colour) {
    let thick = stroke(unit);
    let across = SIDE_UNITS * unit;
    let middle = (across - thick) / 2;
    frame.fill(x, y + middle, across, thick, colour);
    frame.fill(x + middle, y, thick, across, colour);
}

fn cross(frame: &mut Frame, (x, y): (usize, usize), unit: usize, colour: Colour) {
    let across = SIDE_UNITS * unit;
    for rising in [false, true] {
        slanted(frame, (x, y), (across, across), unit, rising, colour);
    }
}

fn arrow(frame: &mut Frame, (centre, y): (usize, usize), unit: usize, up: bool, colour: Colour) {
    let half = ARROW_UNITS * unit;
    let left = centre.saturating_sub(half);
    let down = ARROW_UNITS * unit;
    slanted(frame, (left, y), (half, down), unit, up, colour);
    slanted(frame, (centre, y), (half, down), unit, !up, colour);
}

fn switch(frame: &mut Frame, (x, y): (usize, usize), unit: usize, on: bool) {
    let colour = if on { FOREGROUND } else { DIM };
    let thick = stroke(unit);
    let width = SWITCH_UNITS * unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    frame.fill(x, y, width, thick, colour);
    frame.fill(x, y + height - thick, width, thick, colour);
    frame.fill(x, y, thick, height, colour);
    frame.fill(x + width - thick, y, thick, height, colour);
    let knob = if on { SWITCH_UNITS - 2 - KNOB_UNITS } else { 2 };
    frame.fill(
        x + knob * unit,
        y + 2 * unit,
        KNOB_UNITS * unit,
        KNOB_UNITS * unit,
        colour,
    );
}

fn settings_screen(frame: &mut Frame, sheet: Sheet, face: &Face) {
    let unit = sheet.unit;
    let top = sheet.setting_row();
    symbol(frame, (sheet.left, top), unit, &POWER, FOREGROUND);
    let right = sheet.side + SIDE_UNITS * unit;
    switch(
        frame,
        (right - SWITCH_UNITS * unit, top),
        unit,
        face.power_off,
    );
}

fn wheels(frame: &mut Frame, sheet: Sheet, picked: TimeOfDay) {
    let unit = sheet.unit;
    let y = sheet.wheel_row(Row::Picked);
    time(frame, (sheet.left, y), unit, Some(picked), FOREGROUND);
    let above = y - (ARROW_UNITS + ARROW_GAP_UNITS) * unit;
    let below = y + (DIGIT_HEIGHT_UNITS + ARROW_GAP_UNITS) * unit;
    let middle = (2 * DIGIT_UNITS + 1) * unit / 2;
    for start in [0, MINUTE_UNITS * unit] {
        let centre = sheet.left + start + middle;
        arrow(frame, (centre, above), unit, true, DIM);
        arrow(frame, (centre, below), unit, false, DIM);
    }
}

fn alarm_screen(frame: &mut Frame, layout: &Layout, sheet: Sheet, face: &Face) {
    let unit = sheet.unit;
    time(frame, sheet.clock, layout.unit, Some(face.time), FOREGROUND);
    let active = face.active_alarm();
    let colour = if active.is_some() { FOREGROUND } else { DIM };
    time(frame, sheet.active, unit, active, colour);
    wheels(frame, sheet, face.picked);
    let add = if face.addable() { FOREGROUND } else { DIM };
    plus(frame, (sheet.side, sheet.wheel_row(Row::Picked)), unit, add);
    for (index, &alarm) in face.alarms.iter().take(SAVED_ALARMS).enumerate() {
        let top = sheet.list_row(index);
        let colour = if face.active == Some(index) {
            FOREGROUND
        } else {
            DIM
        };
        time(frame, (sheet.left, top), unit, Some(alarm), colour);
        let mark = if face.marked == Some(index) {
            WARNING
        } else {
            DIM
        };
        cross(frame, (sheet.side, top), unit, mark);
    }
}

fn bar(frame: &mut Frame, layout: &Layout, current: Screen) {
    let width = frame.size().width;
    let across = TAB_UNITS * layout.tab_unit;
    let top = layout.bar_top + TAB_MARGIN_UNITS * layout.tab_unit;
    for (index, screen) in Screen::ALL.into_iter().enumerate() {
        let centre = width * (2 * index + 1) / (2 * Screen::ALL.len());
        let colour = if screen == current { FOREGROUND } else { DIM };
        symbol(
            frame,
            (centre.saturating_sub(across / 2), top),
            layout.tab_unit,
            &screen.symbol(),
            colour,
        );
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
    let sheet = sheet(size, &layout);
    if face.ringing || (sheet.is_some() && face.screen == Screen::Alarm) {
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
        time(
            &mut frame,
            layout.alarm,
            layout.alarm_unit,
            face.active_alarm(),
            digits,
        );
        return frame;
    }
    match (face.screen, sheet) {
        (Screen::Alarm, Some(sheet)) => alarm_screen(&mut frame, &layout, sheet, face),
        (Screen::Settings, Some(sheet)) => settings_screen(&mut frame, sheet, face),
        _ => {}
    }
    bar(&mut frame, &layout, face.screen);
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
            Field::Hour
        } else {
            Field::Minute
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
        Some(index) if side => Target::Remove(index),
        Some(index) => Target::Entry(index),
        None => Target::Blank,
    }
}

#[must_use]
pub fn target(size: Size, screen: Screen, x: usize, y: usize) -> Target {
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
    match (screen, sheet(size, &layout)) {
        (Screen::Alarm, Some(sheet)) => sheet_target(sheet, x, y),
        (Screen::Settings, Some(sheet)) if sheet.band(sheet.setting_row(), y) => Target::PowerOff,
        _ => Target::Blank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{ChargeLimit, Percent};

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

    fn picture(frame: &Frame) -> String {
        (0..frame.size().height)
            .map(|y| {
                frame
                    .row(y)
                    .iter()
                    .map(|&colour| match colour {
                        FOREGROUND => '#',
                        WARNING => '!',
                        RINGING => '~',
                        DIM => ':',
                        _ => '.',
                    })
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
        let sheet = sheet(SHEET, &layout).ok_or("no sheet")?;
        let across = SIDE_UNITS * sheet.unit;
        let top = sheet.list_row(1);
        for y in 0..SHEET.height {
            for (x, (&before, &after)) in plain.row(y).iter().zip(marked.row(y)).enumerate() {
                let inside = (sheet.side..sheet.side + across).contains(&x)
                    && (top..top + across).contains(&y);
                if inside && before == DIM {
                    assert_eq!(after, WARNING);
                } else {
                    assert_eq!(after, before);
                }
            }
        }
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

    #[test]
    fn the_timer_screen_shows_only_the_bar_with_its_tab_marked() -> Outcome {
        let timer = Face {
            screen: Screen::Timer,
            ..face(Some(0), false, charged(80)?)?
        };
        let frame = render(SMALL, &timer);
        assert_eq!(picture(&frame), include_str!("../testdata/face-timer.txt"));
        Ok(())
    }

    #[test]
    fn the_settings_screen_shows_the_power_off_toggle_as_recorded() -> Outcome {
        for (power_off, recorded) in [
            (true, include_str!("../testdata/face-settings.txt")),
            (false, include_str!("../testdata/face-settings-stays.txt")),
        ] {
            let settings = Face {
                screen: Screen::Settings,
                power_off,
                ..face(Some(0), false, charged(80)?)?
            };
            assert_eq!(picture(&render(SHEET, &settings)), recorded);
        }
        Ok(())
    }

    #[test]
    fn a_tap_across_the_toggle_row_is_the_power_off_and_elsewhere_blank() {
        let settings = |x, y| target(PANEL, Screen::Settings, x, y);
        assert_eq!(settings(0, 725), Target::PowerOff);
        assert_eq!(settings(540, 800), Target::PowerOff);
        assert_eq!(settings(1079, 908), Target::PowerOff);
        assert_eq!(settings(540, 724), Target::Blank);
        assert_eq!(settings(540, 909), Target::Blank);
        assert_eq!(settings(540, 1400), Target::Blank);
        assert_eq!(target(SMALL, Screen::Settings, 50, 20), Target::Blank);
    }

    #[test]
    fn each_screen_marks_its_own_tab_and_only_the_alarm_screen_shows_the_slider() -> Outcome {
        let mut seen = Vec::new();
        for screen in Screen::ALL {
            let face = Face {
                screen,
                ..face(Some(0), false, charged(80)?)?
            };
            let frame = render(PANEL, &face);
            let marked: Vec<_> = lit(&frame)
                .into_iter()
                .filter(|&(_, y, colour)| y > PANEL.height * 19 / 20 && colour == FOREGROUND)
                .map(|(x, _, _)| x * Screen::ALL.len() / PANEL.width)
                .collect();
            assert_ne!(marked, []);
            assert!(marked.iter().all(|&third| marked[0] == third));
            assert!(!seen.contains(&marked[0]));
            seen.push(marked[0]);
            assert_eq!(
                frame.row(1000).contains(&FOREGROUND),
                screen == Screen::Alarm
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
                    .all(|&(_, y, lit)| y < PANEL.height - 40 && (lit == FOREGROUND || lit == DIM))
            );
            assert!(bar.iter().any(|&(_, _, lit)| lit == DIM));
            assert_ne!(status, []);
            assert!(status.iter().all(|&(_, _, lit)| lit == colour));
            assert!(
                status
                    .iter()
                    .all(|&(x, y, _)| x < PANEL.width - 40 && y > 40)
            );
            assert!(rest.iter().all(|&(_, y, _)| y >= 138));
            assert!(rest.iter().all(|&(x, _, _)| x > 40 && x < PANEL.width - 40));
            assert!(
                rest.iter()
                    .all(|&(_, _, lit)| lit == FOREGROUND || lit == DIM)
            );
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
        assert_eq!(target(short, Screen::Alarm, 10, 10), Target::Blank);
        assert_eq!(target(short, Screen::Timer, 10, 10), Target::Blank);
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
        assert_eq!(target(SMALL, Screen::Alarm, 50, 20), Target::Blank);
        Ok(())
    }

    #[test]
    fn a_tap_on_a_wheel_picks_its_field_and_row() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, x, y);
        assert_eq!(alarm(300, 720), Target::Wheel(Field::Hour, Row::Above));
        assert_eq!(alarm(423, 800), Target::Wheel(Field::Hour, Row::Above));
        assert_eq!(alarm(424, 800), Target::Wheel(Field::Minute, Row::Above));
        assert_eq!(alarm(300, 1000), Target::Wheel(Field::Hour, Row::Picked));
        assert_eq!(alarm(600, 1000), Target::Wheel(Field::Minute, Row::Picked));
        assert_eq!(alarm(0, 1200), Target::Wheel(Field::Hour, Row::Below));
        assert_eq!(alarm(700, 1287), Target::Wheel(Field::Minute, Row::Below));
    }

    #[test]
    fn a_tap_beside_the_picked_row_adds_and_beside_another_is_blank() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, x, y);
        assert_eq!(alarm(701, 1000), Target::Add);
        assert_eq!(alarm(1079, 909), Target::Add);
        assert_eq!(alarm(800, 800), Target::Blank);
        assert_eq!(alarm(800, 1200), Target::Blank);
    }

    #[test]
    fn a_tap_on_a_list_row_picks_the_entry_or_its_cross() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, x, y);
        assert_eq!(alarm(300, 1323), Target::Entry(0));
        assert_eq!(alarm(300, 1506), Target::Entry(0));
        assert_eq!(alarm(300, 1507), Target::Entry(1));
        assert_eq!(alarm(700, 1800), Target::Entry(2));
        assert_eq!(alarm(701, 1800), Target::Remove(2));
        assert_eq!(alarm(1000, 2058), Target::Remove(3));
    }

    #[test]
    fn a_tap_between_the_parts_of_the_sheet_is_blank() {
        let alarm = |x, y| target(PANEL, Screen::Alarm, x, y);
        assert_eq!(alarm(540, 300), Target::Blank);
        assert_eq!(alarm(540, 600), Target::Blank);
        assert_eq!(alarm(540, 1300), Target::Blank);
        assert_eq!(alarm(540, 2059), Target::Blank);
        assert_eq!(alarm(540, 2092), Target::Blank);
    }

    #[test]
    fn a_tap_on_the_bar_picks_the_tab_in_its_third_on_every_screen() {
        for screen in Screen::ALL {
            let tap = |x, y| target(PANEL, screen, x, y);
            assert_eq!(tap(0, 2279), Target::Tab(Screen::Alarm));
            assert_eq!(tap(359, 2100), Target::Tab(Screen::Alarm));
            assert_eq!(tap(360, 2200), Target::Tab(Screen::Timer));
            assert_eq!(tap(719, 2279), Target::Tab(Screen::Timer));
            assert_eq!(tap(720, 2100), Target::Tab(Screen::Settings));
            assert_eq!(tap(1079, 2279), Target::Tab(Screen::Settings));
        }
    }

    #[test]
    fn a_tap_above_the_bar_on_an_empty_screen_is_blank() {
        for (x, y) in [(400, 800), (400, 1000), (300, 1400)] {
            assert_eq!(target(PANEL, Screen::Timer, x, y), Target::Blank);
        }
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
