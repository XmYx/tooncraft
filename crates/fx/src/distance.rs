//! Outline and line thickening / thinning from an exact Euclidean distance transform.
//!
//! The transform is the linear-time lower-envelope algorithm of P. Felzenszwalb and
//! D. Huttenlocher ("Distance Transforms of Sampled Functions", Theory of Computing 8, 2012), run
//! over columns and then rows, and it also reports the nearest feature pixel so thickened lines
//! keep their own colours.

use tooncraft_raster::{RgbaImage, over, premultiply, scale_alpha, unpremultiply};

use crate::{blur::transpose, par};

/// "No feature in this line" for the squared distances.
const FAR: f64 = 1e20;
/// No nearest pixel.
const NONE: u32 = u32::MAX;

/// Squared distance and nearest index along one line: `d[q] = min_p (q − p)² + f[p]`.
fn line(f: &[f64], d: &mut [f64], arg: &mut [usize]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0f64; n + 1];
    let mut k = 0usize;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    for q in 1..n {
        loop {
            let p = v[k];
            let s = ((f[q] + (q * q) as f64) - (f[p] + (p * p) as f64)) / (2.0 * q as f64 - 2.0 * p as f64);
            if s <= z[k] && k > 0 {
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    let mut k = 0usize;
    for q in 0..n {
        while k < n && z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f64 - p as f64;
        d[q] = dq * dq + f[p];
        arg[q] = p;
    }
}

/// Distance (pixels) from every pixel to the nearest pixel where `feature` is set, and that
/// pixel's index (`NONE` when there is no feature pixel). `feature` is `w`×`h`, row-major.
fn transform(feature: &[bool], w: usize, h: usize) -> (Vec<f32>, Vec<u32>) {
    // columns, as rows of the transposed mask: (squared vertical distance, nearest y)
    let mask: Vec<u8> = feature.iter().map(|&b| if b { 255 } else { 0 }).flat_map(|v| [v, 0, 0, 0]).collect();
    let mt = transpose(&mask, w, h);
    let mut cols: Vec<(f32, u32)> = vec![(0.0, NONE); w * h];
    par::rows(&mut cols, h, |x, out| {
        let f: Vec<f64> = (0..h).map(|y| if mt.get((x * h + y) * 4).is_some_and(|v| *v != 0) { 0.0 } else { FAR }).collect();
        let mut d = vec![0f64; h];
        let mut arg = vec![0usize; h];
        line(&f, &mut d, &mut arg);
        for (y, o) in out.iter_mut().enumerate() {
            *o = if d[y] >= FAR / 2.0 { (f32::INFINITY, NONE) } else { (d[y] as f32, arg[y] as u32) };
        }
    });
    // back to row-major: cols[x*h + y] → colres[y*w + x]
    let mut colres: Vec<(f32, u32)> = vec![(f32::INFINITY, NONE); w * h];
    par::rows(&mut colres, w, |y, out| {
        for (x, o) in out.iter_mut().enumerate() {
            if let Some(v) = cols.get(x * h + y) {
                *o = *v;
            }
        }
    });
    let mut res: Vec<(f32, u32)> = vec![(f32::INFINITY, NONE); w * h];
    par::rows(&mut res, w, |y, out| {
        let row = colres.get(y * w..(y + 1) * w).unwrap_or(&[]);
        let f: Vec<f64> = row.iter().map(|(d, _)| if d.is_finite() { *d as f64 } else { FAR }).collect();
        let mut d = vec![0f64; f.len()];
        let mut arg = vec![0usize; f.len()];
        line(&f, &mut d, &mut arg);
        for (x, o) in out.iter_mut().enumerate().take(f.len()) {
            let ax = arg[x];
            *o = match row.get(ax) {
                Some((_, ny)) if d[x] < FAR / 2.0 && *ny != NONE => ((d[x] as f32).sqrt(), ny * w as u32 + ax as u32),
                _ => (f32::INFINITY, NONE),
            };
        }
    });
    res.into_iter().unzip()
}

fn alpha_mask(img: &RgbaImage, inside: bool) -> Vec<bool> {
    img.data.as_chunks::<4>().0.iter().map(|p| (p[3] >= 128) == inside).collect()
}

/// A solid outline `width` pixels wide around the drawing, under it.
pub(crate) fn outline(img: &mut RgbaImage, width: f32, colour: [u8; 3]) {
    if width.is_nan() || width < 0.05 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let (dist, _) = transform(&alpha_mask(img, true), w, h);
    for (p, d) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(dist) {
        let cov = (width + 1.0 - d).clamp(0.0, 1.0);
        if cov <= 0.0 {
            continue;
        }
        let line = premultiply([colour[0], colour[1], colour[2], (cov * 255.0).round() as u8]);
        let out = over(line, [p[0], p[1], p[2], p[3]]);
        p.copy_from_slice(&out);
    }
}

/// Grow lines by `amount` pixels (their nearest colour) or thin them (negative `amount`).
pub(crate) fn thicken(img: &mut RgbaImage, amount: f32) {
    if amount.is_nan() || amount.abs() < 0.05 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    if amount > 0.0 {
        let (dist, near) = transform(&alpha_mask(img, true), w, h);
        let src = img.data.clone();
        for ((p, d), n) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(dist).zip(near) {
            if p[3] == 255 || n == NONE {
                continue;
            }
            let cov = (amount + 1.0 - d).clamp(0.0, 1.0);
            if cov <= 0.0 {
                continue;
            }
            let i = n as usize * 4;
            let Some(s) = src.get(i..i + 4) else { continue };
            let c = unpremultiply([s[0], s[1], s[2], s[3]]);
            let grown = premultiply([c[0], c[1], c[2], (cov * 255.0).round() as u8]);
            let out = over(grown, [p[0], p[1], p[2], p[3]]);
            p.copy_from_slice(&out);
        }
    } else {
        let t = -amount;
        let (dist, _) = transform(&alpha_mask(img, false), w, h);
        for (p, d) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(dist) {
            if p[3] == 0 || d.is_infinite() {
                continue;
            }
            let keep = (d - t).clamp(0.0, 1.0);
            let out = scale_alpha([p[0], p[1], p[2], p[3]], (keep * 255.0).round() as u8);
            p.copy_from_slice(&out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_are_euclidean_and_nearest_is_reported() {
        let (w, h) = (9, 7);
        let mut f = vec![false; w * h];
        f[3 * w + 4] = true;
        let (d, n) = transform(&f, w, h);
        assert_eq!(d[3 * w + 4], 0.0);
        assert!((d[0] - ((4.0f32).powi(2) + 9.0).sqrt()).abs() < 1e-5);
        assert!((d[6 * w + 8] - (16.0f32 + 9.0).sqrt()).abs() < 1e-5);
        assert!(n.iter().all(|&i| i == (3 * w + 4) as u32));
        let (d, n) = transform(&vec![false; w * h], w, h);
        assert!(d.iter().all(|v| v.is_infinite()) && n.iter().all(|&i| i == NONE));
    }

    #[test]
    fn brute_force_agrees() {
        let (w, h) = (23, 17);
        let f: Vec<bool> = (0..w * h).map(|i| (i * 7919 + 13) % 31 == 0).collect();
        let (d, _) = transform(&f, w, h);
        for y in 0..h {
            for x in 0..w {
                let best = (0..w * h)
                    .filter(|&i| f[i])
                    .map(|i| (((i % w) as f32 - x as f32).powi(2) + ((i / w) as f32 - y as f32).powi(2)).sqrt())
                    .fold(f32::INFINITY, f32::min);
                assert!((d[y * w + x] - best).abs() < 1e-4, "({x}, {y}): {} vs {best}", d[y * w + x]);
            }
        }
    }
}
