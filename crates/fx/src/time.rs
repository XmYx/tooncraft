//! Transitions: a layer appearing (or leaving) over `length` frames from `start` by dissolving,
//! wiping, opening an iris or pushing in. The time effects that read other frames (motion blur,
//! trails, frame blending, time remap) are worked out by the compositor, which can draw the
//! layer at any frame; see `tooncraft_doc::render`.

use tooncraft_raster::{RgbaImage, scale_alpha, select};

use crate::{TransitionStyle, par, shape::coverage};

/// How far the layer has come in, 0 (hidden) … 1 (fully there), at `frame`.
pub(crate) fn shown(start: u32, length: u32, out: bool, frame: u32) -> f32 {
    let t = (frame as f32 - start as f32 + 1.0) / length.max(1) as f32;
    let t = t.clamp(0.0, 1.0);
    if out { 1.0 - t } else { t }
}

/// A transition's settings.
pub(crate) struct Transition {
    pub style: TransitionStyle,
    pub shown: f32,
    pub angle: f32,
    pub centre: (f32, f32),
    pub size: f32,
}

pub(crate) fn apply(img: &mut RgbaImage, origin: (u32, u32), t: &Transition) {
    let v = t.shown.clamp(0.0, 1.0);
    if v >= 1.0 {
        return;
    }
    if v <= 0.0 {
        img.data.fill(0);
        return;
    }
    let (s, c) = t.angle.to_radians().sin_cos();
    let size = t.size.max(1.0);
    let w = img.width as usize;
    match t.style {
        TransitionStyle::Dissolve => {
            let k = (v * 255.0).round() as u8;
            for p in img.data.as_chunks_mut::<4>().0 {
                *p = scale_alpha(*p, k);
            }
        }
        TransitionStyle::Wipe | TransitionStyle::Iris => {
            // the edge sweeps across `2 * size` (wipe) or opens to radius `size` (iris)
            let feather = size * 0.04;
            par::rows(&mut img.data, w * 4, |y, row| {
                let py = (origin.1 as usize + y) as f32 + 0.5 - t.centre.1;
                for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    if p[3] == 0 {
                        continue;
                    }
                    let px = (origin.0 as usize + x) as f32 + 0.5 - t.centre.0;
                    let d = match t.style {
                        TransitionStyle::Iris => px.hypot(py) - v * size,
                        _ => (px * c + py * s) - (-size + 2.0 * size * v),
                    };
                    let k = coverage(d, feather);
                    if k < 1.0 {
                        *p = scale_alpha(*p, (k * 255.0).round() as u8);
                    }
                }
            });
        }
        TransitionStyle::Push => {
            // comes in from behind the direction: `(1 - v) * 2 * size` pixels back
            let back = (1.0 - v) * 2.0 * size;
            let (dx, dy) = (-c * back, -s * back);
            let src = img.clone();
            par::rows(&mut img.data, w * 4, |y, row| {
                for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let out = select::sample(&src, x as f32 + 0.5 - dx, y as f32 + 0.5 - dy);
                    p.copy_from_slice(&out);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> RgbaImage {
        RgbaImage::filled(40, 20, [0, 0, 0, 255])
    }

    #[test]
    fn progress_runs_from_start_over_length() {
        assert_eq!(shown(10, 4, false, 5), 0.0, "before it starts, hidden");
        assert_eq!(shown(10, 4, false, 13), 1.0, "fully in at the last frame");
        assert_eq!(shown(10, 4, false, 11), 0.5);
        assert_eq!(shown(10, 4, true, 30), 0.0, "gone after a transition out");
        assert_eq!(shown(10, 0, false, 10), 1.0, "a zero length cuts");
    }

    #[test]
    fn styles() {
        let tr = |style, shown| Transition { style, shown, angle: 0.0, centre: (20.0, 10.0), size: 20.0 };
        let mut d = full();
        apply(&mut d, (0, 0), &tr(TransitionStyle::Dissolve, 0.5));
        assert_eq!(d.pixel(5, 5)[3], 128);
        let mut w = full();
        apply(&mut w, (0, 0), &tr(TransitionStyle::Wipe, 0.5));
        assert_eq!(w.pixel(2, 5)[3], 255, "the left half is in");
        assert_eq!(w.pixel(37, 5)[3], 0, "the right half not yet");
        let mut i = full();
        apply(&mut i, (0, 0), &tr(TransitionStyle::Iris, 0.25));
        assert_eq!(i.pixel(20, 10)[3], 255);
        assert_eq!(i.pixel(2, 2)[3], 0);
        let mut line = RgbaImage::transparent(40, 20);
        for y in 0..20 {
            line.set_pixel(30, y, [0, 0, 0, 255]);
        }
        apply(&mut line, (0, 0), &Transition { size: 5.0, ..tr(TransitionStyle::Push, 0.5) });
        assert!(line.pixel(25, 5)[3] > 200, "pushed in from the left: {:?}", line.pixel(25, 5));
        let mut gone = full();
        apply(&mut gone, (0, 0), &tr(TransitionStyle::Iris, 0.0));
        assert!(gone.data.iter().all(|v| *v == 0));
    }
}
