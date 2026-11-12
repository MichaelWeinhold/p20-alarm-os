use crate::hardware::{Clock, Display, HardwareError};
use crate::time::{TimeOfDay, UnixSeconds, UtcOffset};

pub const BACKGROUND: Colour = Colour(0x00_0000);
pub const FOREGROUND: Colour = Colour(0xff_ffff);
pub const WARNING: Colour = Colour(0xff_3b30);

const UNITS_ACROSS: usize = 21;
const DIGIT_UNITS: usize = 4;
const DIGIT_HEIGHT_UNITS: usize = 7;
const MARK_GAP_UNITS: usize = 2;

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

fn digit(frame: &mut Frame, x: usize, y: usize, unit: usize, value: u8) {
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
            frame.fill(left, top, across, down, FOREGROUND);
        }
    }
}

fn battery_mark(frame: &mut Frame, x: usize, y: usize, unit: usize) {
    let line = (unit / 4).max(1);
    let width = DIGIT_UNITS * unit;
    let height = 2 * unit;
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
pub fn render(size: Size, time: TimeOfDay, low_battery: bool) -> Frame {
    let mut frame = Frame::filled(size, BACKGROUND);
    let unit = size.width * 9 / 10 / UNITS_ACROSS;
    let height = DIGIT_HEIGHT_UNITS * unit;
    if unit == 0 || height > size.height {
        return frame;
    }
    let mut x = (size.width - UNITS_ACROSS * unit) / 2;
    let y = (size.height - height) / 2;
    let advance = (DIGIT_UNITS + 1) * unit;
    for value in [time.hour() / 10, time.hour() % 10] {
        digit(&mut frame, x, y, unit, value);
        x += advance;
    }
    frame.fill(x, y + 2 * unit, unit, unit, FOREGROUND);
    frame.fill(x, y + 4 * unit, unit, unit, FOREGROUND);
    x += 2 * unit;
    for value in [time.minute() / 10, time.minute() % 10] {
        digit(&mut frame, x, y, unit, value);
        x += advance;
    }
    if low_battery {
        let mark_y = y + height + MARK_GAP_UNITS * unit;
        let mark_x = (size.width - DIGIT_UNITS * unit) / 2;
        battery_mark(&mut frame, mark_x, mark_y, unit);
    }
    frame
}

pub fn show_time(
    clock: &impl Clock,
    offset: UtcOffset,
    low_battery: bool,
    display: &mut impl Display,
) -> Result<UnixSeconds, HardwareError> {
    let now = clock.now()?;
    let frame = render(display.size(), TimeOfDay::at(now, offset), low_battery);
    display.show(&frame)?;
    Ok(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimeError;

    const SMALL: Size = Size {
        width: 100,
        height: 64,
    };
    const PANEL: Size = Size {
        width: 1080,
        height: 2280,
    };

    struct FixedClock(Result<UnixSeconds, HardwareError>);

    impl Clock for FixedClock {
        fn now(&self) -> Result<UnixSeconds, HardwareError> {
            self.0.clone()
        }
    }

    struct FakeDisplay {
        size: Size,
        shown: Vec<Frame>,
    }

    impl Display for FakeDisplay {
        fn size(&self) -> Size {
            self.size
        }

        fn show(&mut self, frame: &Frame) -> Result<(), HardwareError> {
            self.shown.push(frame.clone());
            Ok(())
        }
    }

    fn picture(frame: &Frame) -> String {
        (0..frame.size().height)
            .map(|y| {
                frame
                    .row(y)
                    .iter()
                    .map(|&colour| match colour {
                        FOREGROUND => '#',
                        WARNING => '!',
                        _ => '.',
                    })
                    .chain(['\n'])
                    .collect::<String>()
            })
            .collect()
    }

    fn evening() -> UnixSeconds {
        UnixSeconds(20_768 * 86_400 + 19 * 3_600 + 37 * 60 + 58)
    }

    #[test]
    fn the_time_is_drawn_as_recorded() -> Result<(), TimeError> {
        let frame = render(SMALL, TimeOfDay::new(19, 37)?, false);
        assert_eq!(picture(&frame), include_str!("../testdata/face-1937.txt"));
        Ok(())
    }

    #[test]
    fn a_low_battery_adds_the_mark_as_recorded() -> Result<(), TimeError> {
        let frame = render(SMALL, TimeOfDay::new(19, 37)?, true);
        assert_eq!(
            picture(&frame),
            include_str!("../testdata/face-1937-low.txt")
        );
        Ok(())
    }

    #[test]
    fn every_digit_has_its_own_shape() -> Result<(), TimeError> {
        let mut seen = Vec::new();
        for value in 0..10 {
            let shape = picture(&render(SMALL, TimeOfDay::new(0, value)?, false));
            assert!(!seen.contains(&shape));
            seen.push(shape);
        }
        Ok(())
    }

    #[test]
    fn the_face_fits_the_panel_with_room_around_it() -> Result<(), TimeError> {
        let frame = render(PANEL, TimeOfDay::new(8, 8)?, true);
        assert_eq!(frame.size(), PANEL);
        let lit: Vec<usize> = (0..PANEL.height)
            .filter(|&y| frame.row(y).iter().any(|&colour| colour != BACKGROUND))
            .collect();
        assert!(lit.first().is_some_and(|&y| y > PANEL.height / 3));
        assert!(lit.last().is_some_and(|&y| y < PANEL.height * 2 / 3));
        assert!(frame.row(PANEL.height / 2).contains(&FOREGROUND));
        assert!(lit.iter().any(|&y| frame.row(y).contains(&WARNING)));
        assert!((0..PANEL.height).all(|y| frame.row(y).len() == PANEL.width));
        Ok(())
    }

    #[test]
    fn a_frame_too_small_for_the_face_stays_blank() -> Result<(), TimeError> {
        let tiny = Size {
            width: 20,
            height: 4,
        };
        let frame = render(tiny, TimeOfDay::new(12, 0)?, true);
        assert_eq!(frame, Frame::filled(tiny, BACKGROUND));
        Ok(())
    }

    #[test]
    fn the_clock_is_shown_in_local_time() -> Result<(), Box<dyn std::error::Error>> {
        let clock = FixedClock(Ok(evening()));
        let mut display = FakeDisplay {
            size: SMALL,
            shown: Vec::new(),
        };
        let berlin_winter = UtcOffset::from_seconds(3_600)?;
        assert_eq!(
            show_time(&clock, berlin_winter, false, &mut display)?,
            evening()
        );
        assert_eq!(
            display.shown,
            [render(SMALL, TimeOfDay::new(20, 37)?, false)]
        );
        Ok(())
    }

    #[test]
    fn a_failing_clock_draws_nothing() {
        let clock = FixedClock(Err(HardwareError::Absent("clock")));
        let mut display = FakeDisplay {
            size: SMALL,
            shown: Vec::new(),
        };
        assert_eq!(
            show_time(&clock, UtcOffset::UTC, false, &mut display),
            Err(HardwareError::Absent("clock"))
        );
        assert_eq!(display.shown, Vec::<Frame>::new());
    }
}
