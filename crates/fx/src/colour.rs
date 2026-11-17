//! Per-pixel colour effects: levels and curves (lookup tables), hue / saturation / lightness,
//! tint, grain and alpha threshold. They work on straight colour and keep the alpha.

use tooncraft_raster::{RgbaImage, premultiply, unpremultiply};

use crate::par;

/// Run `f` on the straight colour of every pixel that is not transparent (with its x, y).
fn each<F: Fn(usize, usize, [u8; 4]) -> [u8; 4] + Sync>(img: &mut RgbaImage, f: F) {
    let w = img.width as usize;
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let s = if p[3] == 255 { [p[0], p[1], p[2], 255] } else { unpremultiply([p[0], p[1], p[2], p[3]]) };
            let o = f(x, y, s);
            let o = if o[3] == 255 { o } else { premultiply(o) };
            p.copy_from_slice(&o);
        }
    });
}

/// Apply one table to red, green and blue.
pub(crate) fn lut(img: &mut RgbaImage, table: &[u8; 256]) {
    each(img, |_, _, s| [table[s[0] as usize], table[s[1] as usize], table[s[2] as usize], s[3]]);
}

pub(crate) fn levels_table(in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32) -> [u8; 256] {
    let span = (in_white - in_black).max(1.0 / 255.0);
    let inv = 1.0 / gamma.max(0.1);
    std::array::from_fn(|i| {
        let v = ((i as f32 / 255.0 - in_black) / span).clamp(0.0, 1.0).powf(inv);
        ((out_black + v * (out_white - out_black)) * 255.0).round().clamp(0.0, 255.0) as u8
    })
}

/// A table for the smooth monotone curve (Fritsch–Carlson cubic Hermite) through sorted points.
pub(crate) fn curve_table(points: &[[f32; 2]]) -> [u8; 256] {
    let n = points.len();
    if n < 2 {
        return std::array::from_fn(|i| i as u8);
    }
    let xs: Vec<f32> = points.iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = points.iter().map(|p| p[1]).collect();
    let d: Vec<f32> = (0..n - 1).map(|k| (ys[k + 1] - ys[k]) / (xs[k + 1] - xs[k]).max(1e-6)).collect();
    let mut m = vec![0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for k in 1..n - 1 {
        m[k] = if d[k - 1] * d[k] > 0.0 { (d[k - 1] + d[k]) / 2.0 } else { 0.0 };
    }
    for k in 0..n - 1 {
        if d[k] == 0.0 {
            m[k] = 0.0;
            m[k + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[k] / d[k], m[k + 1] / d[k]);
        let r = a * a + b * b;
        if r > 9.0 {
            let t = 3.0 / r.sqrt();
            m[k] = t * a * d[k];
            m[k + 1] = t * b * d[k];
        }
    }
    std::array::from_fn(|i| {
        let x = i as f32 / 255.0;
        let y = if x <= xs[0] {
            ys[0]
        } else if x >= xs[n - 1] {
            ys[n - 1]
        } else {
            let k = xs.partition_point(|v| *v <= x).saturating_sub(1).min(n - 2);
            let hgt = (xs[k + 1] - xs[k]).max(1e-6);
            let t = (x - xs[k]) / hgt;
            let (t2, t3) = (t * t, t * t * t);
            (2.0 * t3 - 3.0 * t2 + 1.0) * ys[k] + (t3 - 2.0 * t2 + t) * hgt * m[k] + (-2.0 * t3 + 3.0 * t2) * ys[k + 1] + (t3 - t2) * hgt * m[k + 1]
        };
        (y * 255.0).round().clamp(0.0, 255.0) as u8
    })
}

fn to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx - mn < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |mut t: f32| {
        t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let h = h / 360.0;
    (hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0))
}

pub(crate) fn hue_saturation(img: &mut RgbaImage, hue: f32, saturation: f32, lightness: f32) {
    if hue == 0.0 && saturation == 0.0 && lightness == 0.0 {
        return;
    }
    each(img, |_, _, s| {
        let (h, sat, l) = to_hsl(s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0);
        let sat = (sat * (1.0 + saturation)).clamp(0.0, 1.0);
        let l = if lightness >= 0.0 { l + (1.0 - l) * lightness } else { l * (1.0 + lightness) };
        let (r, g, b) = from_hsl(h + hue, sat, l.clamp(0.0, 1.0));
        let q = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
        [q(r), q(g), q(b), s[3]]
    });
}

pub(crate) fn tint(img: &mut RgbaImage, colour: [u8; 3], amount: f32) {
    if amount <= 0.0 {
        return;
    }
    each(img, |_, _, s| {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * amount).round().clamp(0.0, 255.0) as u8;
        [m(s[0], colour[0]), m(s[1], colour[1]), m(s[2], colour[2]), s[3]]
    });
}

/// An integer hash of a grain cell (our own mix of multiplies and shifts).
fn hash(x: u32, y: u32, seed: u32, frame: u32) -> u32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77) ^ seed.wrapping_mul(0xC2B2_AE3D) ^ frame.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Luminance grain: every `size`×`size` cell gets one random brightness change of up to ±`amount`.
pub(crate) fn grain(img: &mut RgbaImage, amount: f32, size: f32, seed: u32, frame: u32) {
    if amount <= 0.0 {
        return;
    }
    let cell = size.round().clamp(1.0, 16.0) as usize;
    each(img, |x, y, s| {
        let n = hash((x / cell) as u32, (y / cell) as u32, seed, frame) as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let v = n * amount * 128.0;
        let a = |c: u8| (c as f32 + v).round().clamp(0.0, 255.0) as u8;
        [a(s[0]), a(s[1]), a(s[2]), s[3]]
    });
}

/// Colour key: transparent near `key`, fading out over `softness` past `tolerance`.
pub(crate) fn key(img: &mut RgbaImage, key: [u8; 3], tolerance: f32, softness: f32) {
    let k = [key[0] as f32 / 255.0, key[1] as f32 / 255.0, key[2] as f32 / 255.0];
    let soft = softness.max(1e-4);
    each(img, |_, _, s| {
        let d = ((0..3).map(|i| (s[i] as f32 / 255.0 - k[i]).powi(2)).sum::<f32>() / 3.0).sqrt();
        let keep = ((d - tolerance) / soft).clamp(0.0, 1.0);
        [s[0], s[1], s[2], (s[3] as f32 * keep).round() as u8]
    });
}

pub(crate) fn alpha_threshold(img: &mut RgbaImage, threshold: f32) {
    let t = (threshold * 255.0).round().clamp(1.0, 255.0) as u8;
    for p in img.data.as_chunks_mut::<4>().0.iter_mut() {
        if p[3] == 0 || p[3] == 255 {
            continue;
        }
        if p[3] >= t {
            let s = unpremultiply([p[0], p[1], p[2], p[3]]);
            p.copy_from_slice(&[s[0], s[1], s[2], 255]);
        } else {
            p.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables() {
        let id = levels_table(0.0, 1.0, 1.0, 0.0, 1.0);
        assert!(id.iter().enumerate().all(|(i, v)| *v as usize == i));
        let inv = levels_table(0.0, 1.0, 1.0, 1.0, 0.0);
        assert_eq!((inv[0], inv[255]), (255, 0));
        let crush = levels_table(0.25, 0.75, 1.0, 0.0, 1.0);
        assert_eq!((crush[50], crush[200]), (0, 255));
        assert!((127..=129).contains(&crush[128]));
        let line = curve_table(&[[0.0, 0.0], [1.0, 1.0]]);
        assert!(line.iter().enumerate().all(|(i, v)| (*v as i32 - i as i32).abs() <= 1));
        let s = curve_table(&[[0.0, 0.0], [0.25, 0.1], [0.75, 0.9], [1.0, 1.0]]);
        assert!(s.windows(2).all(|w| w[1] >= w[0]), "monotone");
        assert!(s[64] < 64 && s[192] > 192, "an S curve adds contrast");
    }

    #[test]
    fn hsl_round_trips() {
        for c in [(1.0, 0.0, 0.0), (0.2, 0.5, 0.9), (0.5, 0.5, 0.5), (0.9, 0.8, 0.1)] {
            let (h, s, l) = to_hsl(c.0, c.1, c.2);
            let (r, g, b) = from_hsl(h, s, l);
            assert!((r - c.0).abs() < 1e-4 && (g - c.1).abs() < 1e-4 && (b - c.2).abs() < 1e-4, "{c:?}");
        }
        let (r, g, b) = from_hsl(to_hsl(1.0, 0.0, 0.0).0 + 120.0, 1.0, 0.5);
        assert!(g > 0.99 && r < 0.01 && b < 0.01, "red + 120° is green");
    }
}
