//! Feathered rectangle and ellipse masks.

use tooncraft_raster::{RgbaImage, scale_alpha};

use crate::{MaskShape, par};

/// Signed distance (pixels, negative inside) from a point to the shape centred at the origin with
/// half extents `hx`, `hy`.
fn signed_distance(shape: MaskShape, px: f32, py: f32, hx: f32, hy: f32) -> f32 {
    match shape {
        MaskShape::Rectangle => {
            let (dx, dy) = (px.abs() - hx, py.abs() - hy);
            let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
            outside + dx.max(dy).min(0.0)
        }
        MaskShape::Ellipse => {
            if hx < 0.5 || hy < 0.5 {
                return f32::INFINITY;
            }
            let k = ((px / hx).powi(2) + (py / hy).powi(2)).sqrt();
            (k - 1.0) * hx.min(hy)
        }
    }
}

/// How much of a pixel at signed distance `d` the mask keeps (0…1) with a `feather`-wide edge.
pub(crate) fn coverage(d: f32, feather: f32) -> f32 {
    let f = feather.max(1.0);
    (0.5 - d / f).clamp(0.0, 1.0)
}

/// Keep the inside (or, inverted, the outside) of the shape `rect` = (centre x, centre y, width,
/// height), with a soft edge.
pub(crate) fn mask(img: &mut RgbaImage, shape: MaskShape, rect: (f32, f32, f32, f32), feather: f32, invert: bool) {
    let w = img.width as usize;
    let (cx, cy, hx, hy) = (rect.0, rect.1, rect.2 / 2.0, rect.3 / 2.0);
    par::rows(&mut img.data, w * 4, |y, row| {
        let py = y as f32 + 0.5 - cy;
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let mut m = coverage(signed_distance(shape, x as f32 + 0.5 - cx, py, hx, hy), feather);
            if invert {
                m = 1.0 - m;
            }
            if m >= 1.0 {
                continue;
            }
            let out = scale_alpha([p[0], p[1], p[2], p[3]], (m * 255.0).round() as u8);
            p.copy_from_slice(&out);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(signed_distance(MaskShape::Rectangle, 0.0, 0.0, 10.0, 5.0), -5.0);
        assert_eq!(signed_distance(MaskShape::Rectangle, 13.0, 0.0, 10.0, 5.0), 3.0);
        assert!((signed_distance(MaskShape::Rectangle, 13.0, 9.0, 10.0, 5.0) - 5.0).abs() < 1e-5);
        assert_eq!(signed_distance(MaskShape::Ellipse, 20.0, 0.0, 10.0, 10.0), 10.0);
        assert_eq!(coverage(-100.0, 10.0), 1.0);
        assert_eq!(coverage(100.0, 10.0), 0.0);
        assert_eq!(coverage(0.0, 10.0), 0.5);
    }
}
