use ab_glyph_rasterizer::{Point, Rasterizer, point};
use ttf_parser::{Face, GlyphId, OutlineBuilder, Rect};

const INTER: &[u8] = include_bytes!("../fonts/inter-light.ttf");
const SYMBOLS: &[u8] = include_bytes!("../fonts/material-symbols-light-icons.ttf");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Icon {
    Alarm,
    Timer,
    Settings,
    Battery,
    Power,
    Light,
    Add,
    Close,
    Up,
    Down,
    Play,
    Pause,
    On,
    Off,
    Recovery,
    Restart,
    Volume,
    Vibration,
    Ramp,
    AddCircle,
    Check,
    Delete,
}

impl Icon {
    #[cfg(test)]
    const ALL: [Self; 22] = [
        Self::Alarm,
        Self::Timer,
        Self::Settings,
        Self::Battery,
        Self::Power,
        Self::Light,
        Self::Add,
        Self::Close,
        Self::Up,
        Self::Down,
        Self::Play,
        Self::Pause,
        Self::On,
        Self::Off,
        Self::Recovery,
        Self::Restart,
        Self::Volume,
        Self::Vibration,
        Self::Ramp,
        Self::AddCircle,
        Self::Check,
        Self::Delete,
    ];

    fn code(self) -> char {
        match self {
            Self::Alarm => '\u{e855}',
            Self::Timer => '\u{e88b}',
            Self::Settings => '\u{e8b8}',
            Self::Battery => '\u{f30d}',
            Self::Power => '\u{f8c7}',
            Self::Light => '\u{e518}',
            Self::Add => '\u{e145}',
            Self::Close => '\u{e5cd}',
            Self::Up => '\u{e316}',
            Self::Down => '\u{e313}',
            Self::Play => '\u{e037}',
            Self::Pause => '\u{e034}',
            Self::On => '\u{e9f6}',
            Self::Off => '\u{e9f5}',
            Self::Recovery => '\u{f8cd}',
            Self::Restart => '\u{f053}',
            Self::Volume => '\u{e050}',
            Self::Vibration => '\u{e62d}',
            Self::Ramp => '\u{e8e5}',
            Self::AddCircle => '\u{e147}',
            Self::Check => '\u{e5ca}',
            Self::Delete => '\u{e872}',
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Baseline,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub centre2: usize,
    pub middle2: usize,
    pub height: usize,
    pub align: Align,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Square {
    pub centre2: usize,
    pub middle2: usize,
    pub size: usize,
}

#[derive(Debug)]
struct Outline {
    raster: Rasterizer,
    left: f32,
    top: f32,
    scale: f32,
    start: Point,
    last: Point,
}

impl Outline {
    fn at(&self, x: f32, y: f32) -> Point {
        point(self.left + x * self.scale, self.top - y * self.scale)
    }
}

impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = self.at(x, y);
        self.last = self.start;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let next = self.at(x, y);
        self.raster.draw_line(self.last, next);
        self.last = next;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let next = self.at(x, y);
        self.raster.draw_quad(self.last, self.at(x1, y1), next);
        self.last = next;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let next = self.at(x, y);
        self.raster
            .draw_cubic(self.last, self.at(x1, y1), self.at(x2, y2), next);
        self.last = next;
    }

    fn close(&mut self) {
        if self.last != self.start {
            self.raster.draw_line(self.last, self.start);
        }
        self.last = self.start;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Span {
    start: i64,
    length: usize,
    offset: f32,
}

fn span(numerator: i64, extent: i64, denominator: i64) -> Option<Span> {
    let start = numerator.div_euclid(denominator);
    let rest = numerator.rem_euclid(denominator);
    let length = usize::try_from((rest + extent + denominator - 1) / denominator).ok()?;
    let offset = f32::from(u16::try_from(rest).ok()?) / f32::from(u16::try_from(denominator).ok()?);
    Some(Span {
        start,
        length,
        offset,
    })
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn level(coverage: f32) -> u8 {
    (coverage.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn ratio(numerator: i64, denominator: i64) -> Option<f32> {
    Some(f32::from(u16::try_from(numerator).ok()?) / f32::from(u16::try_from(denominator).ok()?))
}

pub(crate) fn rasterise(character: char, place: Place, mut plot: impl FnMut(usize, usize, u8)) {
    let _ = drawn(character, place, &mut plot);
}

pub(crate) fn width(line: &str, height: usize) -> usize {
    measured(line, height).unwrap_or(0)
}

pub(crate) fn write(
    line: &str,
    (left, baseline): (usize, usize),
    height: usize,
    mut plot: impl FnMut(usize, usize, u8),
) {
    let _ = written(line, (left, baseline), height, &mut plot);
}

fn measured(line: &str, height: usize) -> Option<usize> {
    let face = Face::parse(INTER, 0).ok()?;
    let cap = i64::from(face.capital_height()?);
    let pen = line
        .chars()
        .filter_map(|character| face.glyph_index(character))
        .filter_map(|id| face.glyph_hor_advance(id))
        .map(i64::from)
        .sum::<i64>();
    usize::try_from((pen * i64::try_from(height).ok()? + cap - 1) / cap).ok()
}

fn written(
    line: &str,
    (left, baseline): (usize, usize),
    height: usize,
    plot: &mut impl FnMut(usize, usize, u8),
) -> Option<()> {
    let face = Face::parse(INTER, 0).ok()?;
    let cap = i64::from(face.capital_height()?);
    let height = i64::try_from(height).ok()?;
    let left = i64::try_from(left).ok()? * cap;
    let baseline = i64::try_from(baseline).ok()? * cap;
    let scale = ratio(height, cap)?;
    let mut pen = 0;
    for id in line
        .chars()
        .filter_map(|character| face.glyph_index(character))
    {
        if let Some(bounds) = face.glyph_bounding_box(id) {
            let width = i64::from(bounds.x_max) - i64::from(bounds.x_min);
            let tall = i64::from(bounds.y_max) - i64::from(bounds.y_min);
            let across = span(
                left + (pen + i64::from(bounds.x_min)) * height,
                width * height,
                cap,
            )?;
            let down = span(
                baseline - i64::from(bounds.y_max) * height,
                tall * height,
                cap,
            )?;
            traced(&face, id, bounds, (across, down), scale, plot)?;
        }
        pen += i64::from(face.glyph_hor_advance(id)?);
    }
    Some(())
}

pub(crate) fn icon(icon: Icon, square: Square, mut plot: impl FnMut(usize, usize, u8)) {
    let _ = drawn_icon(icon, square, &mut plot);
}

fn drawn(character: char, place: Place, plot: &mut impl FnMut(usize, usize, u8)) -> Option<()> {
    let face = Face::parse(INTER, 0).ok()?;
    let id = face.glyph_index(character)?;
    let bounds = face.glyph_bounding_box(id)?;
    let cap = i64::from(face.capital_height()?);
    let height = i64::try_from(place.height).ok()?;
    let centre2 = i64::try_from(place.centre2).ok()?;
    let middle2 = i64::try_from(place.middle2).ok()?;
    let width = i64::from(bounds.x_max) - i64::from(bounds.x_min);
    let tall = i64::from(bounds.y_max) - i64::from(bounds.y_min);
    let top = match place.align {
        Align::Baseline => (middle2 + height) * cap - 2 * i64::from(bounds.y_max) * height,
        Align::Middle => middle2 * cap - tall * height,
    };
    let across = span(centre2 * cap - width * height, 2 * width * height, 2 * cap)?;
    let down = span(top, 2 * tall * height, 2 * cap)?;
    traced(&face, id, bounds, (across, down), ratio(height, cap)?, plot)
}

fn drawn_icon(icon: Icon, square: Square, plot: &mut impl FnMut(usize, usize, u8)) -> Option<()> {
    let face = Face::parse(SYMBOLS, 0).ok()?;
    let id = face.glyph_index(icon.code())?;
    let bounds = face.glyph_bounding_box(id)?;
    let em = i64::from(face.units_per_em());
    let size = i64::try_from(square.size).ok()?;
    let centre2 = i64::try_from(square.centre2).ok()?;
    let middle2 = i64::try_from(square.middle2).ok()?;
    let width = i64::from(bounds.x_max) - i64::from(bounds.x_min);
    let tall = i64::from(bounds.y_max) - i64::from(bounds.y_min);
    let left = (centre2 - size) * em + 2 * i64::from(bounds.x_min) * size;
    let top = (middle2 - size) * em + 2 * (em - i64::from(bounds.y_max)) * size;
    let across = span(left, 2 * width * size, 2 * em)?;
    let down = span(top, 2 * tall * size, 2 * em)?;
    traced(&face, id, bounds, (across, down), ratio(size, em)?, plot)
}

fn traced(
    face: &Face,
    id: GlyphId,
    bounds: Rect,
    (across, down): (Span, Span),
    scale: f32,
    plot: &mut impl FnMut(usize, usize, u8),
) -> Option<()> {
    let mut outline = Outline {
        raster: Rasterizer::new(across.length, down.length),
        left: across.offset - f32::from(bounds.x_min) * scale,
        top: down.offset + f32::from(bounds.y_max) * scale,
        scale,
        start: point(0.0, 0.0),
        last: point(0.0, 0.0),
    };
    face.outline_glyph(id, &mut outline)?;
    covered(&outline.raster, (across, down), plot);
    Some(())
}

fn covered(
    raster: &Rasterizer,
    (across, down): (Span, Span),
    plot: &mut impl FnMut(usize, usize, u8),
) {
    raster.for_each_pixel(|index, coverage| {
        let alpha = level(coverage);
        if alpha == 0 {
            return;
        }
        let x = across.start + i64::try_from(index % across.length).unwrap_or(i64::MAX);
        let y = down.start + i64::try_from(index / across.length).unwrap_or(i64::MAX);
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) {
            plot(x, y, alpha);
        }
    });
}

pub(crate) fn ring(
    square: Square,
    thickness: usize,
    share: (i64, i64),
    mut plot: impl FnMut(usize, usize, u8, u8),
) {
    let _ = drawn_ring(square, thickness, share, &mut plot);
}

fn drawn_ring(
    square: Square,
    thickness: usize,
    (part, whole): (i64, i64),
    plot: &mut impl FnMut(usize, usize, u8, u8),
) -> Option<()> {
    let part = part.clamp(0, whole);
    if whole <= 0 || 2 * thickness > square.size {
        return None;
    }
    let outer = ratio(i64::try_from(square.size).ok()?, 2)?;
    let inner = outer - ratio(i64::try_from(thickness).ok()?, 1)?;
    let centre = ratio(i64::try_from(square.centre2).ok()?, 2)?;
    let middle = ratio(i64::try_from(square.middle2).ok()?, 2)?;
    let turn = (part < whole).then_some((std::f32::consts::TAU * ratio(part, whole)?).sin_cos());
    let solid = ((outer - 0.5).powi(2), (inner + 0.5).max(0.0).powi(2));
    for y in floor(middle - outer - 1.0).max(0)..=floor(middle + outer + 1.0) {
        let dy = ratio(y, 1)? + 0.5 - middle;
        let reach = (outer + 0.5).powi(2) - dy * dy;
        if reach <= 0.0 {
            continue;
        }
        let hole = (inner - 0.5).powi(2) - dy * dy;
        let hole = if inner > 0.5 && hole > 0.0 {
            hole.sqrt()
        } else {
            0.0
        };
        let reach = reach.sqrt();
        let first = floor(centre - hole).max(0);
        let second = (first + 1).max(floor(centre + hole));
        let columns = (floor(centre - reach).max(0)..=first).chain(second..=floor(centre + reach));
        for x in columns {
            let dx = ratio(x, 1)? + 0.5 - centre;
            let squared = dx * dx + dy * dy;
            let band = if squared <= solid.0 && squared >= solid.1 {
                1.0
            } else {
                let distance = squared.sqrt();
                let band = (outer - distance + 0.5).clamp(0.0, 1.0);
                if inner > 0.0 {
                    band.min((distance - inner + 0.5).clamp(0.0, 1.0))
                } else {
                    band
                }
            };
            let arc = match turn {
                _ if part == 0 => 0.0,
                Some(turn) => band.min(swept(dx, dy, turn)),
                None => band,
            };
            let alpha = level(band);
            if alpha > 0 {
                plot(
                    usize::try_from(x).ok()?,
                    usize::try_from(y).ok()?,
                    alpha,
                    level(arc),
                );
            }
        }
    }
    Some(())
}

fn swept(dx: f32, dy: f32, (sin, cos): (f32, f32)) -> f32 {
    let start = dx;
    let end = -dx * cos - dy * sin;
    let inside = if sin >= 0.0 {
        start.min(end)
    } else {
        start.max(end)
    };
    (inside + 0.5).clamp(0.0, 1.0)
}

#[expect(clippy::cast_possible_truncation)]
fn floor(value: f32) -> i64 {
    value.floor() as i64
}

#[cfg(test)]
mod tests {
    use super::{Align, Icon, Place, Square, icon, rasterise, ring, width, write};

    const FIGURES: &str = "0123456789:%-ADFMSTWadehilmnorstuy";

    fn place(height: usize, align: Align) -> Place {
        Place {
            centre2: 200,
            middle2: 200,
            height,
            align,
        }
    }

    fn plotted(character: char, place: Place) -> Vec<(usize, usize, u8)> {
        let mut found = Vec::new();
        rasterise(character, place, |x, y, alpha| {
            if alpha > 0 {
                found.push((x, y, alpha));
            }
        });
        found
    }

    #[test]
    fn every_figure_face_draws_is_in_the_font() {
        for character in FIGURES.chars() {
            assert!(
                !plotted(character, place(42, Align::Baseline)).is_empty(),
                "{character}"
            );
        }
    }

    #[test]
    fn a_character_outside_the_subset_draws_nothing() {
        assert_eq!(plotted('+', place(42, Align::Baseline)), Vec::new());
    }

    #[test]
    fn a_figure_sits_on_the_baseline_at_the_height_asked_for() {
        let pixels = plotted('1', place(60, Align::Baseline));
        let top = pixels.iter().map(|&(_, y, _)| y).min();
        let bottom = pixels.iter().map(|&(_, y, _)| y).max();
        assert_eq!((top, bottom), (Some(70), Some(129)));
    }

    #[test]
    fn a_figure_is_centred_on_the_centre_asked_for() {
        let pixels = plotted('0', place(60, Align::Baseline));
        let left = pixels.iter().map(|&(x, _, _)| x).min().unwrap_or(0);
        let right = pixels.iter().map(|&(x, _, _)| x).max().unwrap_or(0);
        assert!((left + right).abs_diff(200) <= 1, "{left}..{right}");
    }

    #[test]
    fn a_middle_glyph_is_centred_on_the_middle_asked_for() {
        let pixels = plotted('-', place(60, Align::Middle));
        let top = pixels.iter().map(|&(_, y, _)| y).min().unwrap_or(0);
        let bottom = pixels.iter().map(|&(_, y, _)| y).max().unwrap_or(0);
        assert!((top + bottom).abs_diff(200) <= 1, "{top}..{bottom}");
    }

    #[test]
    fn edges_come_out_grey() {
        let pixels = plotted('0', place(60, Align::Baseline));
        assert!(pixels.iter().any(|&(_, _, alpha)| alpha < 255));
        assert!(pixels.iter().any(|&(_, _, alpha)| alpha == 255));
    }

    #[test]
    fn a_glyph_past_the_top_left_is_clipped_not_wrapped() {
        let place = Place {
            centre2: 0,
            middle2: 0,
            height: 60,
            align: Align::Baseline,
        };
        let pixels = plotted('0', place);
        assert_ne!(pixels, Vec::new());
        assert!(pixels.iter().all(|&(x, y, _)| x < 30 && y < 32));
    }

    fn written(line: &str, height: usize) -> Vec<(usize, usize, u8)> {
        let mut found = Vec::new();
        write(line, (40, 100), height, |x, y, alpha| {
            if alpha > 0 {
                found.push((x, y, alpha));
            }
        });
        found
    }

    #[test]
    fn a_line_starts_at_its_left_and_stands_on_its_baseline() {
        let pixels = written("Mon", 30);
        let left = pixels.iter().map(|&(x, _, _)| x).min();
        let top = pixels.iter().map(|&(_, y, _)| y).min();
        let bottom = pixels.iter().map(|&(_, y, _)| y).max();
        assert_eq!((left, top, bottom), (Some(43), Some(70), Some(100)));
    }

    #[test]
    fn a_line_ends_within_its_measured_width() {
        let pixels = written("Alarm in 11h 42m", 30);
        let right = pixels.iter().map(|&(x, _, _)| x).max().unwrap_or(0);
        let measured = width("Alarm in 11h 42m", 30);
        assert!(
            right < 40 + measured && right + 6 > 40 + measured,
            "{right} {measured}"
        );
    }

    #[test]
    fn a_space_moves_the_pen_and_draws_nothing() {
        assert!(width("M T", 30) > width("MT", 30));
        assert_eq!(written(" ", 30), Vec::new());
        assert_eq!(width("", 30), 0);
    }

    #[test]
    fn a_character_outside_the_subset_neither_draws_nor_moves_the_pen() {
        assert_eq!(width("M+", 30), width("M", 30));
        assert_eq!(written("M+", 30), written("M", 30));
    }

    fn drawn_icon(shown: Icon, size: usize) -> Vec<(usize, usize, u8)> {
        let mut found = Vec::new();
        let square = Square {
            centre2: 200,
            middle2: 200,
            size,
        };
        icon(shown, square, |x, y, alpha| {
            if alpha > 0 {
                found.push((x, y, alpha));
            }
        });
        found
    }

    fn extent(pixels: &[(usize, usize, u8)]) -> Option<(usize, usize, usize, usize)> {
        Some((
            pixels.iter().map(|&(x, _, _)| x).min()?,
            pixels.iter().map(|&(_, y, _)| y).min()?,
            pixels.iter().map(|&(x, _, _)| x).max()?,
            pixels.iter().map(|&(_, y, _)| y).max()?,
        ))
    }

    #[test]
    fn every_icon_face_draws_is_in_the_font() {
        for shown in Icon::ALL {
            assert!(!drawn_icon(shown, 48).is_empty(), "{shown:?}");
        }
    }

    #[test]
    fn every_icon_has_its_own_shape() {
        let mut seen = Vec::new();
        for shown in Icon::ALL {
            let shape = drawn_icon(shown, 48);
            assert!(!seen.contains(&shape), "{shown:?}");
            seen.push(shape);
        }
    }

    #[test]
    fn an_icon_fills_the_em_square_asked_for_around_its_centre() {
        let add = extent(&drawn_icon(Icon::Add, 96));
        assert_eq!(add, Some((74, 74, 125, 125)));
    }

    #[test]
    fn an_icon_scales_with_the_square_and_keeps_its_strokes_thin() {
        let stroke = |size: usize| {
            let row = 100 + size / 4;
            let coverage: u32 = drawn_icon(Icon::Add, size)
                .iter()
                .filter(|&&(_, y, _)| y == row)
                .map(|&(_, _, alpha)| u32::from(alpha))
                .sum();
            coverage / 255
        };
        assert_eq!((stroke(96), stroke(192), stroke(384)), (4, 9, 18));
    }

    #[test]
    fn an_icon_has_grey_edges() {
        let pixels = drawn_icon(Icon::Close, 96);
        assert!(pixels.iter().any(|&(_, _, alpha)| alpha < 255));
        assert!(pixels.iter().any(|&(_, _, alpha)| alpha == 255));
    }

    #[test]
    fn an_icon_past_the_top_left_is_clipped_not_wrapped() {
        let mut found = Vec::new();
        let square = Square {
            centre2: 0,
            middle2: 0,
            size: 96,
        };
        icon(Icon::Light, square, |x, y, alpha| {
            if alpha > 0 {
                found.push((x, y));
            }
        });
        assert_ne!(found, Vec::new());
        assert!(found.iter().all(|&(x, y)| x < 48 && y < 48));
    }

    fn drawn_ring(share: (i64, i64)) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        let square = Square {
            centre2: 200,
            middle2: 200,
            size: 96,
        };
        ring(square, 8, share, |x, y, _, arc| {
            if arc > 127 {
                found.push((x, y));
            }
        });
        found
    }

    #[test]
    fn an_empty_share_draws_no_arc_but_the_whole_band() {
        assert_eq!(drawn_ring((0, 180)), Vec::new());
        let square = Square {
            centre2: 200,
            middle2: 200,
            size: 96,
        };
        let mut band = Vec::new();
        ring(square, 8, (0, 180), |x, y, alpha, _| {
            if alpha > 127 {
                band.push((x, y));
            }
        });
        assert_eq!(band, drawn_ring((180, 180)));
    }

    #[test]
    fn a_full_ring_is_a_band_around_the_square() {
        let full = drawn_ring((180, 180));
        assert!(full.contains(&(100, 54)));
        assert!(full.contains(&(54, 100)));
        assert!(full.contains(&(145, 100)));
        assert!(full.contains(&(100, 145)));
        assert!(!full.contains(&(100, 100)));
        assert!(!full.contains(&(100, 70)));
    }

    #[test]
    fn a_half_ring_runs_clockwise_from_the_top_down_the_right() {
        let half = drawn_ring((90, 180));
        assert!(half.contains(&(145, 100)));
        assert!(!half.contains(&(54, 100)));
        assert!(half.iter().all(|&(x, _)| x >= 99));
    }
}
