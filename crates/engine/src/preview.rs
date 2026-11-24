//! The brush preview stroke: one stroke that exercises every input (pressure swells and fades,
//! the pen leans, the speed varies, the path curves), rendered with the real brush engine. The
//! brush editor shows it live; `tooncraft-cli brush preview` writes it to a PNG.

use tooncraft_brush::{Brush, BrushKind, Images, Sample};
use tooncraft_raster::{RgbaImage, Surface};

/// The input of the preview stroke on a `w` × `h` sheet: 90 samples left to right.
pub fn samples(w: u32, h: u32) -> Vec<Sample> {
    let (wf, hf) = (w as f32, h as f32);
    let mut time = 0.0f64;
    (0..90)
        .map(|i| {
            let t = i as f32 / 89.0;
            // slow at the ends, quick in the middle
            time += 0.004 + 0.01 * (1.0 - (t * std::f32::consts::TAU).cos()) as f64;
            Sample {
                x: 10.0 + t * (wf - 20.0),
                y: hf * 0.5 + (t * std::f32::consts::TAU).sin() * hf * 0.26,
                pressure: (t * std::f32::consts::PI).sin().max(0.04),
                tilt_x: 0.85 * (t * std::f32::consts::PI * 1.5).sin(),
                tilt_y: 0.4 * (t * std::f32::consts::PI).cos(),
                time,
                // half a turn of the barrel along the stroke, so a mapping from it shows
                rotation: t * 0.5,
            }
        })
        .collect()
}

fn bar(s: &mut Surface, w: u32, y: f32, size: f32, color: [u8; 4]) {
    let pts: Vec<Sample> = (0..30).map(|i| Sample::at(10.0 + i as f32 * (w as f32 - 20.0) / 29.0, y, 1.0, i as f64 * 0.01)).collect();
    tooncraft_brush::paint(s, Brush { color, size, ..Brush::preset(BrushKind::Ink) }, &pts);
}

/// Render the preview stroke of `brush` onto transparent paper. The eraser is shown as a dark pen
/// of its size; a wet brush gets colour under it to mix with; the smudge brush gets two bars of
/// colour to drag. `None` when the sheet is out of range.
pub fn render(brush: Brush, images: Images, w: u32, h: u32) -> Option<RgbaImage> {
    if w < 8 || h < 8 || w > 4096 || h > 4096 {
        return None;
    }
    let mut s = Surface::new(w, h).ok()?;
    let mut b = brush;
    if b.kind == BrushKind::Eraser {
        b.kind = BrushKind::Ink;
        b.color = [28, 26, 40, 255];
    }
    b.size = b.size.min(h as f32 * 0.5);
    let hf = h as f32;
    if b.kind == BrushKind::Smudge {
        bar(&mut s, w, hf * 0.3, hf * 0.18, [220, 70, 60, 255]);
        bar(&mut s, w, hf * 0.7, hf * 0.18, [50, 110, 200, 255]);
    } else if b.wet.pickup > 0.0 {
        bar(&mut s, w, hf * 0.5, hf * 0.3, [60, 120, 210, 255]);
    }
    tooncraft_brush::paint_with(&mut s, b, &samples(w, h), images);
    Some(s.to_image())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_pixels(img: &RgbaImage) -> usize {
        img.data.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()
    }

    #[test]
    fn every_brush_makes_a_preview() {
        for kind in BrushKind::ALL {
            let img = render(Brush::preset(kind), Images::default(), 200, 60).unwrap();
            assert_eq!((img.width, img.height), (200, 60));
            assert!(ink_pixels(&img) > 50, "{kind:?} preview is empty");
        }
    }

    #[test]
    fn the_preview_follows_the_brush() {
        let thin = render(Brush { size: 3.0, ..Brush::preset(BrushKind::Ink) }, Images::default(), 200, 60).unwrap();
        let thick = render(Brush { size: 20.0, ..Brush::preset(BrushKind::Ink) }, Images::default(), 200, 60).unwrap();
        assert!(ink_pixels(&thick) > ink_pixels(&thin) * 2);
        assert!(render(Brush::default(), Images::default(), 2, 2).is_none());
        assert!(render(Brush::default(), Images::default(), 100_000, 10).is_none());
    }
}
