//! Blurs and the effects built on them (glow, drop shadow, light wrap).
//!
//! The Gaussian is three box blurs per axis, with box widths chosen so the three passes have the
//! Gaussian's variance (W. Kovesi, "Fast almost-Gaussian filtering", DICTA 2010). Each box pass is
//! a sliding-window sum over a row, so the cost does not grow with the radius; columns are blurred
//! as rows of the transposed image. Pixels outside the canvas count as transparent.

use tooncraft_raster::{RgbaImage, over, premultiply, unpremultiply};

use crate::par;

/// Box radii of three passes approximating a Gaussian of standard deviation `sigma`.
fn boxes(sigma: f32) -> [usize; 3] {
    let s = sigma as f64;
    let n = 3.0;
    let ideal = (12.0 * s * s / n + 1.0).sqrt();
    let mut wl = ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wl = wl.max(1);
    let wu = wl + 2;
    let (wlf, nf) = (wl as f64, n);
    let m = ((12.0 * s * s - nf * wlf * wlf - 4.0 * nf * wlf - 3.0 * nf) / (-4.0 * wlf - 4.0)).round() as i64;
    [0i64, 1, 2].map(|i| (((if i < m { wl } else { wu }) - 1) / 2).max(0) as usize)
}

/// One box pass over a row of `w` pixels of `C` channels: `dst[x]` is the mean of
/// `src[x−r ..= x+r]`.
fn box_row<const C: usize>(src: &[u8], dst: &mut [u8], w: usize, r: usize) {
    let d = (2 * r + 1) as u32;
    let half = d / 2;
    let mut acc = [0u32; C];
    let px = |i: usize| -> [u8; C] {
        let mut p = [0u8; C];
        if let Some(s) = src.get(i * C..i * C + C) {
            p.copy_from_slice(s);
        }
        p
    };
    for i in 0..=r.min(w.saturating_sub(1)) {
        let p = px(i);
        for c in 0..C {
            acc[c] += p[c] as u32;
        }
    }
    for (x, out) in dst.as_chunks_mut::<C>().0.iter_mut().enumerate().take(w) {
        for c in 0..C {
            out[c] = ((acc[c] + half) / d).min(255) as u8;
        }
        if x + r + 1 < w {
            let p = px(x + r + 1);
            for c in 0..C {
                acc[c] += p[c] as u32;
            }
        }
        if x >= r {
            let p = px(x - r);
            for c in 0..C {
                acc[c] = acc[c].saturating_sub(p[c] as u32);
            }
        }
    }
}

/// The three box passes along every row of a buffer `w` pixels wide, each row on its own
/// (two row-sized scratch buffers, no copy of the image).
fn blur_rows<const C: usize>(data: &mut [u8], w: usize, radii: &[usize; 3]) {
    par::rows(data, w * C, |_, row| {
        let mut a = row.to_vec();
        let mut b = vec![0u8; row.len()];
        for &r in radii.iter().filter(|r| **r > 0) {
            box_row::<C>(&a, &mut b, w, r);
            std::mem::swap(&mut a, &mut b);
        }
        row.copy_from_slice(&a);
    });
}

/// Transpose a `w`×`h` buffer of `C`-channel pixels into `h`×`w`.
fn transpose_c<const C: usize>(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    // output row `x` holds column `x` of the source
    par::rows(&mut out, h * C, |x, row| {
        for (y, p) in row.as_chunks_mut::<C>().0.iter_mut().enumerate() {
            let i = (y * w + x) * C;
            if let Some(s) = src.get(i..i + C) {
                p.copy_from_slice(s);
            }
        }
    });
    out
}

/// Transpose a `w`×`h` RGBA buffer into `h`×`w`.
pub(crate) fn transpose(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    transpose_c::<4>(src, w, h)
}

fn gaussian_c<const C: usize>(data: &mut Vec<u8>, w: usize, h: usize, sigma: f32) {
    if sigma.is_nan() || sigma < 0.3 || data.len() != w * h * C {
        return;
    }
    let radii = boxes(sigma.min(crate::MAX_RADIUS));
    blur_rows::<C>(data, w, &radii);
    let mut t = transpose_c::<C>(data, w, h);
    blur_rows::<C>(&mut t, h, &radii);
    *data = transpose_c::<C>(&t, h, w);
}

/// Gaussian blur of a one-byte plane `w`×`h` (standard deviation `sigma`), in place.
pub(crate) fn gaussian_plane(data: &mut Vec<u8>, w: usize, h: usize, sigma: f32) {
    gaussian_c::<1>(data, w, h, sigma);
}

/// How far (pixels) the blur of standard deviation `sigma` reaches.
pub(crate) fn support(sigma: f32) -> usize {
    if sigma.is_nan() || sigma < 0.3 {
        return 0;
    }
    boxes(sigma.min(crate::MAX_RADIUS)).iter().sum::<usize>() + 1
}

/// Gaussian blur with standard deviation `sigma` pixels, in place.
pub fn gaussian(img: &mut RgbaImage, sigma: f32) {
    let (w, h) = (img.width as usize, img.height as usize);
    gaussian_c::<4>(&mut img.data, w, h, sigma);
}

/// The alpha channel of an image as a plane.
fn alpha_plane(img: &RgbaImage) -> Vec<u8> {
    img.data.as_chunks::<4>().0.iter().map(|p| p[3]).collect()
}

fn at(src: &[u8], w: usize, h: usize, x: i64, y: i64) -> [u8; 4] {
    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
        return [0; 4];
    }
    let i = (y as usize * w + x as usize) * 4;
    match src.get(i..i + 4) {
        Some(p) => [p[0], p[1], p[2], p[3]],
        None => [0; 4],
    }
}

/// Mean of the pixels on a line `length` pixels long at `angle` degrees through each pixel.
pub(crate) fn directional(img: &mut RgbaImage, length: f32, angle: f32) {
    if length.is_nan() || length < 1.0 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let n = (length.ceil() as usize + 1).clamp(2, 501);
    let (s, c) = angle.to_radians().sin_cos();
    let offsets: Vec<(i64, i64)> = (0..n)
        .map(|i| {
            let t = (i as f32 / (n - 1) as f32 - 0.5) * length;
            ((t * c).round() as i64, (t * s).round() as i64)
        })
        .collect();
    let src = img.data.clone();
    let div = n as u32;
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let mut acc = [0u32; 4];
            for (dx, dy) in &offsets {
                let p = at(&src, w, h, x as i64 + dx, y as i64 + dy);
                for k in 0..4 {
                    acc[k] += p[k] as u32;
                }
            }
            for k in 0..4 {
                out[k] = ((acc[k] + div / 2) / div).min(255) as u8;
            }
        }
    });
}

/// Zoom blur: each pixel is the mean of the pixels between it and a point `amount` of the way to
/// the centre (`cx`, `cy` as fractions of the image).
pub(crate) fn radial(img: &mut RgbaImage, amount: f32, cx: f32, cy: f32) {
    if amount.is_nan() || amount <= 0.001 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let (cx, cy) = (cx * w as f32, cy * h as f32);
    let far = [(0.0, 0.0), (w as f32, 0.0), (0.0, h as f32), (w as f32, h as f32)]
        .iter()
        .map(|(x, y)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt())
        .fold(0.0f32, f32::max);
    let n = ((amount * far / 2.0).ceil() as usize).clamp(2, 64);
    let src = img.data.clone();
    let div = n as u32;
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let mut acc = [0u32; 4];
            for i in 0..n {
                let k = 1.0 - amount * i as f32 / (n - 1) as f32;
                let p = at(&src, w, h, (cx + px * k - 0.5).round() as i64, (cy + py * k - 0.5).round() as i64);
                for c in 0..4 {
                    acc[c] += p[c] as u32;
                }
            }
            for c in 0..4 {
                out[c] = ((acc[c] + div / 2) / div).min(255) as u8;
            }
        }
    });
}

/// Add a blurred copy tinted by `colour`, scaled by `intensity`.
pub(crate) fn glow(img: &mut RgbaImage, radius: f32, intensity: f32, colour: [u8; 3]) {
    if intensity.is_nan() || intensity <= 0.0 {
        return;
    }
    let mut g = img.clone();
    gaussian(&mut g, radius);
    let k = intensity;
    let tint = [colour[0] as f32 / 255.0, colour[1] as f32 / 255.0, colour[2] as f32 / 255.0];
    for (d, s) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(g.data.as_chunks::<4>().0.iter()) {
        if s[3] == 0 {
            continue;
        }
        let a = (d[3] as f32 + s[3] as f32 * k).min(255.0);
        for c in 0..3 {
            d[c] = (d[c] as f32 + s[c] as f32 * tint[c] * k).min(a).round() as u8;
        }
        d[3] = a.round() as u8;
    }
}

/// An offset, blurred silhouette in `colour` under the image.
pub(crate) fn drop_shadow(img: &mut RgbaImage, offset: (f32, f32), blur: f32, colour: [u8; 3], opacity: f32) {
    if opacity.is_nan() || opacity <= 0.0 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let (dx, dy) = (offset.0.round() as i64, offset.1.round() as i64);
    let o = opacity.clamp(0.0, 1.0);
    let mut sh = vec![0u8; w * h];
    {
        let src = &img.data;
        par::rows(&mut sh, w, |y, row| {
            for (x, out) in row.iter_mut().enumerate() {
                let a = at(src, w, h, x as i64 - dx, y as i64 - dy)[3];
                *out = (a as f32 * o).round() as u8;
            }
        });
    }
    gaussian_c::<1>(&mut sh, w, h, blur);
    for (d, a) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(sh.iter()) {
        if *a == 0 {
            continue;
        }
        let s = premultiply([colour[0], colour[1], colour[2], *a]);
        *d = over(s, *d);
    }
}

/// Bleed the blurred light of `below` into the inner edges of the image (screen blend).
pub(crate) fn light_wrap(img: &mut RgbaImage, below: &RgbaImage, radius: f32, intensity: f32) {
    if below.width != img.width || below.height != img.height || intensity.is_nan() || intensity <= 0.0 || radius.is_nan() || radius < 0.3 {
        return;
    }
    let mut bg = below.clone();
    gaussian(&mut bg, radius);
    let mut fa = alpha_plane(img);
    gaussian_c::<1>(&mut fa, img.width as usize, img.height as usize, radius);
    for ((d, b), f) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(bg.data.as_chunks::<4>().0.iter()).zip(fa.iter()) {
        if d[3] == 0 {
            continue;
        }
        let edge = (d[3] as f32 / 255.0) * (1.0 - *f as f32 / 255.0);
        let k = (edge * 2.0 * intensity).clamp(0.0, 1.0);
        if k <= 0.0 {
            continue;
        }
        let fg = unpremultiply([d[0], d[1], d[2], d[3]]);
        let light = unpremultiply([b[0], b[1], b[2], b[3]]);
        let mut out = [0u8; 4];
        for c in 0..3 {
            let (s, l) = (fg[c] as f32 / 255.0, light[c] as f32 / 255.0 * k * (b[3] as f32 / 255.0));
            out[c] = ((1.0 - (1.0 - s) * (1.0 - l)) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        out[3] = d[3];
        d.copy_from_slice(&premultiply(out));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_widths_match_the_gaussian_variance() {
        for sigma in [0.5f32, 1.0, 2.5, 4.0, 10.0, 50.0] {
            let r = boxes(sigma);
            // variance of a box of width w is (w² − 1) / 12
            let var: f64 = r.iter().map(|&r| ((2 * r + 1) as f64).powi(2) - 1.0).sum::<f64>() / 12.0;
            assert!((var.sqrt() - sigma as f64).abs() < 0.6 + sigma as f64 * 0.05, "sigma {sigma}: {r:?} gives {}", var.sqrt());
        }
    }

    #[test]
    fn box_row_is_a_mean_with_transparent_edges() {
        let src: Vec<u8> = [0u8, 0, 0, 0, 90, 90, 90, 90, 0, 0, 0, 0].to_vec();
        let mut dst = vec![0u8; 12];
        box_row::<4>(&src, &mut dst, 3, 1);
        assert_eq!(&dst[0..4], &[30, 30, 30, 30]);
        assert_eq!(&dst[4..8], &[30, 30, 30, 30]);
        assert_eq!(&dst[8..12], &[30, 30, 30, 30]);
    }

    #[test]
    fn transpose_round_trips() {
        let src: Vec<u8> = (0..(5 * 3 * 4)).map(|i| i as u8).collect();
        let t = transpose(&src, 5, 3);
        assert_eq!(&t[4..8], &src[20..24], "row 0 of the transpose is column 0");
        assert_eq!(transpose(&t, 3, 5), src);
    }
}
