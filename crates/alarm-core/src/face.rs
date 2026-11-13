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
    pub low_battery: bool,
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
    mark: (usize, usize),
    alarm_unit: usize,
    alarm: (usize, usize),
}

fn layout(size: Size) -> Option<Layout> {
    let unit = size.width * 9 / 10 / UNITS_ACROSS;
    let alarm_unit = unit / 2;
    let height = DIGIT_HEIGHT_UNITS * unit;
    if alarm_unit == 0 || height > size.height {
        return None;
    }
    let top = (size.height - height) / 2;
    let mark_top = top + height + MARK_GAP_UNITS * unit;
    let alarm_top = mark_top + (MARK_HEIGHT_UNITS + MARK_GAP_UNITS) * unit;
    if alarm_top + DIGIT_HEIGHT_UNITS * alarm_unit > size.height {
        return None;
    }
    Some(Layout {
        unit,
        clock: ((size.width - UNITS_ACROSS * unit) / 2, top),
        mark: ((size.width - DIGIT_UNITS * unit) / 2, mark_top),
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

fn battery_mark(frame: &mut Frame, (x, y): (usize, usize), unit: usize) {
    let line = (unit / 4).max(1);
    let width = DIGIT_UNITS * unit;
    let height = MARK_HEIGHT_UNITS * unit;
    frame.fill(x, y, width, line, WARNING);
    frame.fill(x, y + height - line, width, line, WARNING);
    frame.fill(x, y, line, height, WARNING);
    frame.fill(x + width - line, y, line, height, WARNING);
    frame.fill(x + width, y + unit / 2, unit / 2, unit, WARNING);
    frame.fill(
        x + 2 * line,
        y + 2 * line,
        width / 5,
        height - 4 * line,
        WARNING,
    );
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
    if face.low_battery {
        battery_mark(&mut frame, layout.mark, layout.unit);
    }
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
    use crate::time::TimeError;

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

    fn face(armed: bool, ringing: bool, low_battery: bool) -> Result<Face, TimeError> {
        Ok(Face {
            time: TimeOfDay::new(19, 37)?,
            alarm: TimeOfDay::new(6, 45)?,
            armed,
            ringing,
            low_battery,
        })
    }

    #[test]
    fn the_time_and_the_armed_alarm_are_drawn_as_recorded() -> Result<(), TimeError> {
        let frame = render(SMALL, &face(true, false, false)?);
        assert_eq!(picture(&frame), include_str!("../testdata/face-1937.txt"));
        Ok(())
    }

    #[test]
    fn a_low_battery_adds_the_mark_and_a_disarmed_alarm_is_dim() -> Result<(), TimeError> {
        let frame = render(SMALL, &face(false, false, true)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-low.txt")
        );
        Ok(())
    }

    #[test]
    fn a_ringing_alarm_turns_the_face_amber() -> Result<(), TimeError> {
        let frame = render(SMALL, &face(true, true, false)?);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-ringing.txt")
        );
        Ok(())
    }

    #[test]
    fn every_digit_has_its_own_shape() -> Result<(), TimeError> {
        let mut seen = Vec::new();
        for value in 0..10 {
            let face = Face {
                time: TimeOfDay::new(0, value)?,
                ..face(true, false, false)?
            };
            let shape = picture(&render(SMALL, &face));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn the_face_fits_the_panel_with_room_around_it() -> Result<(), TimeError> {
        let frame = render(PANEL, &face(true, false, true)?);
        assert_eq!(frame.size(), PANEL);
        let lit: Vec<usize> = (0..PANEL.height)
            .filter(|&y| frame.row(y).iter().any(|&colour| colour != BACKGROUND))
            .collect();
        assert!(lit.first().is_some_and(|&y| y > PANEL.height / 3));
        assert!(lit.last().is_some_and(|&y| y < PANEL.height * 4 / 5));
        assert!(frame.row(PANEL.height / 2).contains(&FOREGROUND));
        assert!(lit.iter().any(|&y| frame.row(y).contains(&WARNING)));
        assert!((0..PANEL.height).all(|y| frame.row(y).len() == PANEL.width));
        Ok(())
    }

    #[test]
    fn a_frame_too_small_for_the_face_stays_blank() -> Result<(), TimeError> {
        let short = Size {
            width: 100,
            height: 64,
        };
        let frame = render(short, &face(true, false, true)?);
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
