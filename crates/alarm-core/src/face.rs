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
pub struct Face {
    pub time: TimeOfDay,
    pub alarm: TimeOfDay,
    pub armed: bool,
    pub ringing: bool,
    pub battery: Gauge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Hour(Step),
    Minute(Step),
    Elsewhere,
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
}

fn layout(size: Size) -> Option<Layout> {
    let unit = size.width * 9 / 10 / UNITS_ACROSS;
    let alarm_unit = unit / 2;
    let status_unit = (unit / 4).max(1);
    let status_width =
        (FIGURES * (DIGIT_UNITS + 1) + PERCENT_UNITS + SYMBOL_GAP_UNITS + SYMBOL_UNITS)
            * status_unit
            + unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    if alarm_unit == 0 || height > size.height {
        return None;
    }
    let top = (size.height - height) / 2;
    let mark_top = top + height + MARK_GAP_UNITS * unit;
    let alarm_top = mark_top + (MARK_HEIGHT_UNITS + MARK_GAP_UNITS) * unit;
    if alarm_top + DIGIT_HEIGHT_UNITS * alarm_unit > size.height
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
    })
}

fn digit(frame: &mut Frame, x: usize, y: usize, unit: usize, value: u8, colour: Colour) {
    let segments = SEGMENTS[usize::from(value % 10)];
    let width = DIGIT_UNITS * unit;
    let right = x + width - unit;
    let bars = [
        (x, y, width, unit),
        (right, y, unit, 4 * unit),
        (right, y + 3 * unit, unit, 4 * unit),
        (x, y + 6 * unit, width, unit),
        (x, y + 3 * unit, unit, 4 * unit),
        (x, y, unit, 4 * unit),
        (x, y + 3 * unit, width, unit),
    ];
    for (index, (left, top, across, down)) in bars.into_iter().enumerate() {
        if segments & (1 << index) != 0 {
            frame.fill(left, top, across, down, colour);
        }
    }
}

fn time(
    frame: &mut Frame,
    (left, y): (usize, usize),
    unit: usize,
    value: TimeOfDay,
    colour: Colour,
) {
    let mut x = left;
    let advance = (DIGIT_UNITS + 1) * unit;
    for figure in [value.hour() / 10, value.hour() % 10] {
        digit(frame, x, y, unit, figure, colour);
        x += advance;
    }
    frame.fill(x, y + 2 * unit, unit, unit, colour);
    frame.fill(x, y + 4 * unit, unit, unit, colour);
    x += 2 * unit;
    for figure in [value.minute() / 10, value.minute() % 10] {
        digit(frame, x, y, unit, figure, colour);
        x += advance;
    }
}

fn percent_sign(frame: &mut Frame, x: usize, y: usize, unit: usize, colour: Colour) {
    frame.fill(x, y, unit, unit, colour);
    frame.fill(x + 3 * unit, y + 6 * unit, unit, unit, colour);
    for row in 0..DIGIT_HEIGHT_UNITS {
        let column = (DIGIT_HEIGHT_UNITS - 1 - row) * 3 / (DIGIT_HEIGHT_UNITS - 1);
        frame.fill(x + column * unit, y + row * unit, unit, unit, colour);
    }
}

fn battery_symbol(
    frame: &mut Frame,
    (x, y): (usize, usize),
    unit: usize,
    gauge: Gauge,
    colour: Colour,
) {
    let width = (SYMBOL_UNITS - 1) * unit;
    let height = DIGIT_HEIGHT_UNITS * unit;
    frame.fill(x, y, width, unit, colour);
    frame.fill(x, y + height - unit, width, unit, colour);
    frame.fill(x, y, unit, height, colour);
    frame.fill(x + width - unit, y, unit, height, colour);
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

#[must_use]
pub fn render(size: Size, face: &Face) -> Frame {
    let background = if face.ringing { RINGING } else { BACKGROUND };
    let mut frame = Frame::filled(size, background);
    let Some(layout) = layout(size) else {
        return frame;
    };
    let digits = if face.ringing { BACKGROUND } else { FOREGROUND };
    time(&mut frame, layout.clock, layout.unit, face.time, digits);
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
    let alarm = if face.armed { digits } else { DIM };
    time(
        &mut frame,
        layout.alarm,
        layout.alarm_unit,
        face.alarm,
        alarm,
    );
    frame
}

#[must_use]
pub fn target(size: Size, x: usize, y: usize) -> Target {
    let Some(layout) = layout(size) else {
        return Target::Elsewhere;
    };
    let height = DIGIT_HEIGHT_UNITS * layout.alarm_unit;
    let (_, top) = layout.alarm;
    if y + height / 2 < top || y >= top + height + height / 2 {
        return Target::Elsewhere;
    }
    let step = if y < top + height / 2 {
        Step::Up
    } else {
        Step::Down
    };
    if x < size.width / 2 {
        Target::Hour(step)
    } else {
        Target::Minute(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battery::{ChargeLimit, Percent};

    type Outcome = Result<(), Box<dyn std::error::Error>>;

    const SMALL: Size = Size {
        width: 100,
        height: 112,
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
        armed: bool,
        ringing: bool,
        battery: Gauge,
    ) -> Result<Face, Box<dyn std::error::Error>> {
        Ok(Face {
            time: TimeOfDay::new(19, 37)?,
            alarm: TimeOfDay::new(6, 45)?,
            armed,
            ringing,
            battery,
        })
    }

    #[test]
    fn the_time_the_armed_alarm_and_a_full_battery_are_drawn_as_recorded() -> Outcome {
        let frame = render(SMALL, &face(true, false, charged(80)?)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-1937.txt"));
        Ok(())
    }

    #[test]
    fn a_low_battery_is_a_warning_and_a_disarmed_alarm_is_dim() -> Outcome {
        let frame = render(SMALL, &face(false, false, charged(7)?)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-low.txt")
        );
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_turns_the_face_amber() -> Outcome {
        let frame = render(SMALL, &face(true, true, charged(64)?)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-ringing.txt")
        );
        Ok(())
    }

    #[test]
    fn an_unread_battery_draws_the_low_symbol_and_no_figures() -> Outcome {
        let frame = render(SMALL, &face(true, false, Gauge::UNREAD)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-unread.txt")
        );
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
            let shape = picture(&render(SMALL, &face));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn every_level_and_capacity_has_its_own_symbol() -> Outcome {
        let mut seen = Vec::new();
        for value in [0, 9, 20, 21, 55, 79, 80, 100] {
            let shape = picture(&render(SMALL, &face(true, false, charged(value)?)?));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
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
            let corner = |x: usize, y: usize| x > PANEL.width / 2 && y < PANEL.height / 10;
            let (status, rest): (Vec<_>, Vec<_>) =
                lit(&frame).into_iter().partition(|&(x, y, _)| corner(x, y));
            assert_ne!(status, []);
            assert!(status.iter().all(|&(_, _, lit)| lit == colour));
            assert!(
                status
                    .iter()
                    .all(|&(x, y, _)| x < PANEL.width - 40 && y > 40)
            );
            assert!(rest.iter().all(|&(_, y, _)| y > PANEL.height / 3));
            assert!(rest.iter().all(|&(_, y, _)| y < PANEL.height * 4 / 5));
            assert!(rest.iter().all(|&(_, _, lit)| lit == FOREGROUND));
            assert!(frame.row(PANEL.height / 2).contains(&FOREGROUND));
        }
        Ok(())
    }

    #[test]
    fn the_battery_leaves_the_time_and_the_alarm_as_they_were() -> Outcome {
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
        assert_eq!(target(short, 10, 10), Target::Elsewhere);
        Ok(())
    }

    #[test]
    fn a_tap_on_the_alarm_steps_the_half_it_lands_on() {
        assert_eq!(target(PANEL, 400, 1600), Target::Hour(Step::Up));
        assert_eq!(target(PANEL, 400, 1700), Target::Hour(Step::Down));
        assert_eq!(target(PANEL, 700, 1510), Target::Minute(Step::Up));
        assert_eq!(target(PANEL, 1079, 1810), Target::Minute(Step::Down));
    }

    #[test]
    fn a_tap_away_from_the_alarm_is_elsewhere() {
        assert_eq!(target(PANEL, 509, 1100), Target::Elsewhere);
        assert_eq!(target(PANEL, 400, 1490), Target::Elsewhere);
        assert_eq!(target(PANEL, 700, 1820), Target::Elsewhere);
        assert_eq!(target(PANEL, 540, 2279), Target::Elsewhere);
    }
}
