//! Feathered masks: rectangles, ellipses and drawn shapes (a closed polygon in canvas pixels).

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
        // drawn shapes go through `drawn`
        MaskShape::Drawn => f32::INFINITY,
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

/// Largest standard deviation of a drawn shape's soft edge (its feather / 4).
const MAX_DRAWN_SIGMA: f32 = 200.0;

/// Keep the inside (or the outside) of the closed polygon `points` (canvas pixels); `origin` is
/// where `img` sits on the canvas. The edge is the polygon's coverage blurred by a quarter of
/// `feather`, worked out over the image plus the blur's reach, so a part of the canvas gets the
/// same pixels as the whole.
pub(crate) fn drawn(img: &mut RgbaImage, origin: (u32, u32), points: &[[f32; 2]], feather: f32, invert: bool) {
    let (w, h) = (img.width as usize, img.height as usize);
    let sigma = (feather / 4.0).clamp(0.0, MAX_DRAWN_SIGMA);
    let m = crate::blur::support(sigma);
    let (mw, mh) = (w + 2 * m, h + 2 * m);
    // the polygon in the padded plane's pixels
    let (ox, oy) = (origin.0 as f32 - m as f32, origin.1 as f32 - m as f32);
    let poly: Vec<(f32, f32)> = points.iter().map(|p| (p[0] - ox, p[1] - oy)).collect();
    let mut plane = vec![0u8; mw * mh];
    if let (Ok(pw), Ok(ph)) = (u32::try_from(mw), u32::try_from(mh))
        && let Some(mask) = tooncraft_raster::select::polygon_mask(pw, ph, &poly)
    {
        for y in 0..mask.height {
            for x in 0..mask.width {
                if mask.get(mask.x + x, mask.y + y)
                    && let Some(v) = plane.get_mut((mask.y + y) as usize * mw + (mask.x + x) as usize)
                {
                    *v = 255;
                }
            }
        }
    }
    crate::blur::gaussian_plane(&mut plane, mw, mh, sigma);
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let k = plane.get((y + m) * mw + x + m).copied().unwrap_or(0);
            let k = if invert { 255 - k } else { k };
            if k < 255 {
                let out = scale_alpha([p[0], p[1], p[2], p[3]], k);
                p.copy_from_slice(&out);
            }
        }
    });
}

/// `points` (a closed polygon) resampled to `n` points evenly spaced along its outline, so two
/// shapes drawn differently can be keyed and blended point by point.
pub fn resample_closed(points: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    let pts: Vec<[f32; 2]> = points.iter().copied().filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
    if pts.len() < 2 || n == 0 {
        return pts;
    }
    let seg = |i: usize| -> ([f32; 2], [f32; 2]) { (pts[i % pts.len()], pts[(i + 1) % pts.len()]) };
    let len = |a: [f32; 2], b: [f32; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
    let total: f32 = (0..pts.len())
        .map(|i| {
            let (a, b) = seg(i);
            len(a, b)
        })
        .sum();
    if total.is_nan() || total <= 0.0 {
        return vec![pts[0]; n];
    }
    let step = total / n as f32;
    let mut out = Vec::with_capacity(n);
    let (mut i, mut walked) = (0usize, 0.0f32);
    for k in 0..n {
        let want = k as f32 * step;
        // at most every segment once over the whole walk
        while i < pts.len() {
            let (a, b) = seg(i);
            let l = len(a, b);
            if walked + l >= want || i + 1 == pts.len() {
                let t = if l > 0.0 { ((want - walked) / l).clamp(0.0, 1.0) } else { 0.0 };
                out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                break;
            }
            walked += l;
            i += 1;
        }
    }
    out
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

    #[test]
    fn drawn_shapes_keep_their_inside() {
        let tri = [[4.0, 4.0], [60.0, 4.0], [4.0, 60.0]];
        let mut img = RgbaImage::filled(64, 64, [0, 0, 0, 255]);
        drawn(&mut img, (0, 0), &tri, 0.0, false);
        assert_eq!(img.pixel(10, 10)[3], 255, "inside the triangle");
        assert_eq!(img.pixel(50, 50)[3], 0, "outside it");
        let mut soft = RgbaImage::filled(64, 64, [0, 0, 0, 255]);
        drawn(&mut soft, (0, 0), &tri, 16.0, true);
        assert_eq!(soft.pixel(55, 55)[3], 255, "inverted: the outside stays");
        let edge = soft.pixel(31, 31)[3];
        assert!(edge > 40 && edge < 215, "a soft edge across the hypotenuse: {edge}");
    }

    #[test]
    fn resampling_spaces_points_evenly() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let r = resample_closed(&sq, 8);
        assert_eq!(r.len(), 8);
        assert_eq!(r[0], [0.0, 0.0]);
        assert_eq!(r[1], [5.0, 0.0]);
        assert_eq!(r[4], [10.0, 10.0]);
        assert_eq!(resample_closed(&[[f32::NAN, 1.0]], 8).len(), 0, "nothing finite to walk");
        assert_eq!(resample_closed(&[[1.0, 1.0], [1.0, 1.0]], 3), vec![[1.0, 1.0]; 3], "no length");
    }
}
