use std::io::{self, Write};
use std::process::ExitCode;

use alarm_core::face::sample::{self, SampleError};
use alarm_core::face::{Colour, DIM, Frame, Size, render};

const PANEL: Size = Size {
    width: 1080,
    height: 2280,
};
const CORNER: usize = 108;
const BORDER: usize = 2;
const SUBSAMPLES: usize = 4;
const BACKDROP: Colour = Colour(0x1c_1c1e);

struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[u8; 4]>,
}

struct Sheet {
    scale: usize,
    width: usize,
    height: usize,
    backdrop: Option<Colour>,
}

const SCREENS: Sheet = Sheet {
    scale: 3,
    width: 1224,
    height: 808,
    backdrop: None,
};
const PREVIEW: Sheet = Sheet {
    scale: 4,
    width: 1280,
    height: 640,
    backdrop: Some(BACKDROP),
};

fn channels(colour: Colour) -> [usize; 3] {
    [16, 8, 0].map(|shift| usize::try_from((colour.0 >> shift) & 0xff).unwrap_or(0))
}

fn byte(value: usize) -> u8 {
    u8::try_from(value.min(255)).unwrap_or(u8::MAX)
}

fn shrunk(frame: &Frame, scale: usize) -> Vec<[usize; 3]> {
    let size = frame.size();
    let width = size.width / scale;
    let height = size.height / scale;
    let area = scale * scale;
    let mut pixels = vec![[0; 3]; width * height];
    for y in 0..height * scale {
        for (x, &colour) in frame.row(y).iter().take(width * scale).enumerate() {
            let pixel = &mut pixels[(y / scale) * width + x / scale];
            for (sum, part) in pixel.iter_mut().zip(channels(colour)) {
                *sum += part;
            }
        }
    }
    for pixel in &mut pixels {
        for sum in pixel.iter_mut() {
            *sum /= area;
        }
    }
    pixels
}

fn inside(x: usize, y: usize, edge: [usize; 4], radius: usize) -> bool {
    let [left, top, right, bottom] = edge;
    if x < left || y < top || x >= right || y >= bottom {
        return false;
    }
    let near_x = x.clamp(left + radius, right - radius);
    let near_y = y.clamp(top + radius, bottom - radius);
    let across = x.abs_diff(near_x);
    let down = y.abs_diff(near_y);
    across * across + down * down <= radius * radius
}

fn rounded(pixels: &[[usize; 3]], width: usize, height: usize, scale: usize) -> Vec<[u8; 4]> {
    let step = 2 * SUBSAMPLES;
    let outer = [0, 0, width * step, height * step];
    let inset = BORDER * step;
    let inner = [inset, inset, outer[2] - inset, outer[3] - inset];
    let radius = CORNER / scale * step;
    let border = channels(DIM);
    let samples = SUBSAMPLES * SUBSAMPLES;
    let mut drawn = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let mut face = 0;
            let mut edge = 0;
            for sub_y in 0..SUBSAMPLES {
                for sub_x in 0..SUBSAMPLES {
                    let point_x = x * step + 2 * sub_x + 1;
                    let point_y = y * step + 2 * sub_y + 1;
                    if inside(point_x, point_y, inner, radius - inset) {
                        face += 1;
                    } else if inside(point_x, point_y, outer, radius) {
                        edge += 1;
                    }
                }
            }
            let covered = face + edge;
            let content = pixels[y * width + x];
            let mut pixel = [0; 4];
            for (out, (part, line)) in pixel.iter_mut().zip(content.iter().zip(border)) {
                *out = byte(
                    (part * face + line * edge)
                        .checked_div(covered)
                        .unwrap_or(0),
                );
            }
            pixel[3] = byte(255 * covered / samples);
            drawn.push(pixel);
        }
    }
    drawn
}

fn over(below: [u8; 4], above: [u8; 4]) -> [u8; 4] {
    let alpha = usize::from(above[3]);
    let under = usize::from(below[3]) * (255 - alpha) / 255;
    let total = alpha + under;
    if total == 0 {
        return [0; 4];
    }
    let mut mixed = [0; 4];
    for (out, (&low, &high)) in mixed.iter_mut().zip(below.iter().zip(above.iter())).take(3) {
        *out = byte((usize::from(high) * alpha + usize::from(low) * under) / total);
    }
    mixed[3] = byte(total);
    mixed
}

fn composed(frames: &[Frame], sheet: &Sheet) -> Canvas {
    let fill = sheet.backdrop.map_or([0; 4], |colour| {
        let [red, green, blue] = channels(colour).map(byte);
        [red, green, blue, u8::MAX]
    });
    let mut canvas = Canvas {
        width: sheet.width,
        height: sheet.height,
        pixels: vec![fill; sheet.width * sheet.height],
    };
    let width = PANEL.width / sheet.scale;
    let height = PANEL.height / sheet.scale;
    let count = frames.len();
    let gap = sheet.width.saturating_sub(count * width) / (count + 1);
    let left = sheet
        .width
        .saturating_sub(count * width + count.saturating_sub(1) * gap)
        / 2;
    let top = sheet.height.saturating_sub(height) / 2;
    for (index, frame) in frames.iter().enumerate() {
        let tile = rounded(&shrunk(frame, sheet.scale), width, height, sheet.scale);
        let origin = left + index * (width + gap);
        for y in 0..height.min(canvas.height - top) {
            for x in 0..width.min(canvas.width.saturating_sub(origin)) {
                let at = (top + y) * canvas.width + origin + x;
                canvas.pixels[at] = over(canvas.pixels[at], tile[y * width + x]);
            }
        }
    }
    canvas
}

fn written(out: &mut impl Write, canvas: &Canvas) -> io::Result<()> {
    for side in [canvas.width, canvas.height] {
        let side = u32::try_from(side).map_err(io::Error::other)?;
        out.write_all(&side.to_le_bytes())?;
    }
    for pixel in &canvas.pixels {
        out.write_all(pixel)?;
    }
    Ok(())
}

fn frames() -> Result<Vec<Frame>, SampleError> {
    Ok([sample::sheet()?, sample::timers()?, sample::settings()?]
        .iter()
        .map(|face| render(PANEL, face))
        .collect())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let frames = frames()?;
    let mut out = io::BufWriter::new(io::stdout().lock());
    for sheet in [&SCREENS, &PREVIEW] {
        written(&mut out, &composed(&frames, sheet))?;
    }
    out.flush()?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "screens: {error}");
            ExitCode::FAILURE
        }
    }
}
