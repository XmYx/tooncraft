//! Looks and distortions: gradient map, posterise, luminance threshold, vignette, chromatic
//! aberration, lens flare, ripple, wave, turbulence, displacement by another layer and line
//! smoothing. Every one works on the premultiplied
//! image in place and is deterministic (the turbulence noise is a hash of the position and seed).

use tooncraft_raster::{RgbaImage, premultiply, select, unpremultiply};

use crate::par;

/// Straight colour of every pixel that is not transparent, with its canvas x, y.
fn each<F: Fn(f32, f32, [u8; 4]) -> [u8; 4] + Sync>(img: &mut RgbaImage, origin: (u32, u32), f: F) {
    let w = img.width as usize;
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let s = unpremultiply([p[0], p[1], p[2], p[3]]);
            let o = f((origin.0 as usize + x) as f32 + 0.5, (origin.1 as usize + y) as f32 + 0.5, s);
            p.copy_from_slice(&premultiply(o));
        }
    });
}

fn luma(s: [u8; 4]) -> f32 {
    (0.2126 * s[0] as f32 + 0.7152 * s[1] as f32 + 0.0722 * s[2] as f32) / 255.0
}

fn mix(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

/// The brightness of each pixel picks a colour between `dark` and `light`; `amount` of the way.
pub(crate) fn gradient_map(img: &mut RgbaImage, dark: [u8; 3], light: [u8; 3], amount: f32) {
    each(img, (0, 0), |_, _, s| {
        let l = luma(s);
        let g = [mix(dark[0], light[0], l), mix(dark[1], light[1], l), mix(dark[2], light[2], l)];
        [mix(s[0], g[0], amount), mix(s[1], g[1], amount), mix(s[2], g[2], amount), s[3]]
    });
}

/// `levels` steps per channel (2 … 64).
pub(crate) fn posterize(img: &mut RgbaImage, levels: u32) {
    let n = levels.clamp(2, 64) as f32 - 1.0;
    let q = |v: u8| ((v as f32 / 255.0 * n).round() / n * 255.0).round() as u8;
    each(img, (0, 0), |_, _, s| [q(s[0]), q(s[1]), q(s[2]), s[3]]);
}

/// Black or white by brightness against `level` (0 … 1).
pub(crate) fn threshold(img: &mut RgbaImage, level: f32) {
    each(img, (0, 0), |_, _, s| {
        let v = if luma(s) >= level { 255 } else { 0 };
        [v, v, v, s[3]]
    });
}

/// Darken towards the edges: full strength `amount` beyond `radius + softness` from (`cx`,
/// `cy`), none inside `radius`.
pub(crate) fn vignette(img: &mut RgbaImage, origin: (u32, u32), (cx, cy): (f32, f32), radius: f32, softness: f32, amount: f32) {
    each(img, origin, |x, y, s| {
        let d = (x - cx).hypot(y - cy);
        let t = ((d - radius) / softness.max(1.0)).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t) * amount;
        [mix(s[0], 0, t), mix(s[1], 0, t), mix(s[2], 0, t), s[3]]
    });
}

/// Red and blue pulled `amount` pixels apart horizontally (a lens's colour fringe).
pub(crate) fn chromatic_aberration(img: &mut RgbaImage, amount: f32) {
    if amount <= 0.0 {
        return;
    }
    let src = img.clone();
    let (w, h) = (img.width, img.height);
    let s = |x: f32, y: f32| select::sample(&src, x, y);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let (r, g, b) = (s(fx + amount, fy), s(fx, fy), s(fx - amount, fy));
            let a = r[3].max(g[3]).max(b[3]);
            img.set_pixel(x, y, [r[0].min(a), g[1].min(a), b[2].min(a), a]);
        }
    }
}

/// Move every pixel by `offset(canvas x, canvas y)` (sampling the original bilinearly).
fn displace(img: &mut RgbaImage, origin: (u32, u32), offset: impl Fn(f32, f32) -> (f32, f32)) {
    let src = img.clone();
    for y in 0..img.height {
        for x in 0..img.width {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let (dx, dy) = offset(fx + origin.0 as f32, fy + origin.1 as f32);
            img.set_pixel(x, y, select::sample(&src, fx - dx, fy - dy));
        }
    }
}

/// Rings spreading from (`cx`, `cy`): the picture moves in and out by `amplitude` pixels every
/// `wavelength` pixels, shifted by `phase` waves.
pub(crate) fn ripple(img: &mut RgbaImage, origin: (u32, u32), (cx, cy): (f32, f32), amplitude: f32, wavelength: f32, phase: f32) {
    if amplitude == 0.0 {
        return;
    }
    let k = std::f32::consts::TAU / wavelength.max(2.0);
    displace(img, origin, |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let d = dx.hypot(dy).max(1e-3);
        let m = amplitude * (d * k - phase * std::f32::consts::TAU).sin();
        (dx / d * m, dy / d * m)
    });
}

/// A wave across the picture at `angle` degrees: `amplitude` pixels, every `wavelength` pixels.
pub(crate) fn wave(img: &mut RgbaImage, origin: (u32, u32), amplitude: f32, wavelength: f32, angle: f32, phase: f32) {
    if amplitude == 0.0 {
        return;
    }
    let (s, c) = angle.to_radians().sin_cos();
    let k = std::f32::consts::TAU / wavelength.max(2.0);
    displace(img, origin, |x, y| {
        // along the wave's direction, the picture moves across it
        let along = x * c + y * s;
        let m = amplitude * (along * k - phase * std::f32::consts::TAU).sin();
        (-s * m, c * m)
    });
}

fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8DA6_B343) ^ (y as u32).wrapping_mul(0xD816_3841) ^ seed.wrapping_mul(0xCB1A_B31F);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5BD1_E995);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65535.0 * 2.0 - 1.0
}

fn noise(x: f32, y: f32, seed: u32) -> f32 {
    let (x0, y0) = (x.floor(), y.floor());
    let (tx, ty) = (x - x0, y - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let (xi, yi) = (x0 as i32, y0 as i32);
    let a = hash(xi, yi, seed) + (hash(xi + 1, yi, seed) - hash(xi, yi, seed)) * sx;
    let b = hash(xi, yi + 1, seed) + (hash(xi + 1, yi + 1, seed) - hash(xi, yi + 1, seed)) * sx;
    a + (b - a) * sy
}

/// A wobbly displacement: `amount` pixels of smooth noise with features `scale` pixels wide.
pub(crate) fn turbulence(img: &mut RgbaImage, origin: (u32, u32), amount: f32, scale: f32, seed: u32) {
    if amount == 0.0 {
        return;
    }
    let s = 1.0 / scale.max(2.0);
    displace(img, origin, |x, y| {
        let n1 = noise(x * s, y * s, seed) + 0.5 * noise(x * s * 2.0, y * s * 2.0, seed ^ 0x55);
        let n2 = noise(x * s + 17.3, y * s - 9.1, seed ^ 0xA5) + 0.5 * noise(x * s * 2.0 + 3.0, y * s * 2.0, seed ^ 0x5A);
        (n1 * amount / 1.5, n2 * amount / 1.5)
    });
}

/// Smooth jagged lines: blur, then rebuild a crisp but smooth edge from the blurred coverage.
pub(crate) fn smooth(img: &mut RgbaImage, radius: f32) {
    if radius <= 0.0 {
        return;
    }
    crate::blur::gaussian(img, radius);
    for p in img.data.as_chunks_mut::<4>().0 {
        if p[3] == 0 {
            continue;
        }
        let s = unpremultiply([p[0], p[1], p[2], p[3]]);
        let a = p[3] as f32 / 255.0;
        let t = ((a - 0.3) / 0.4).clamp(0.0, 1.0);
        let na = (t * t * (3.0 - 2.0 * t) * 255.0).round() as u8;
        p.copy_from_slice(&premultiply([s[0], s[1], s[2], na]));
    }
}

/// A lens flare's look: where the light is, the point its ghosts line up through, and its size,
/// strength, colour, number of rays and of ghosts.
pub(crate) struct Flare {
    pub light: (f32, f32),
    pub through: (f32, f32),
    pub size: f32,
    pub intensity: f32,
    pub colour: [u8; 3],
    pub rays: u32,
    pub ghosts: u32,
}

/// Light added at canvas `(x, y)` by the flare, per channel 0…1 (before the intensity cap).
fn flare_light(f: &Flare, x: f32, y: f32) -> [f32; 3] {
    let (lx, ly) = f.light;
    let s = f.size.max(1.0);
    let (dx, dy) = (x - lx, y - ly);
    let d = dx.hypot(dy);
    let c = f.colour.map(|v| v as f32 / 255.0);
    // the bright core and a faint ring around it
    let core = (-(d / s).powi(2) * 4.0).exp();
    let ring = 0.3 * (-((d - 1.6 * s) / (0.12 * s)).powi(2)).exp();
    // star rays: narrow peaks around the light, fading with distance
    let rays = if f.rays >= 2 {
        let a = dy.atan2(dx) * f.rays as f32 / 2.0;
        0.6 * a.cos().abs().powi(60) * (-d / (4.0 * s)).exp()
    } else {
        0.0
    };
    let mut out = [0.0f32; 3];
    for ch in 0..3 {
        out[ch] = (core + ring + rays) * c[ch];
    }
    // ghosts: soft discs on the line from the light through `through`, each a different hue
    let (vx, vy) = (f.through.0 - lx, f.through.1 - ly);
    for i in 0..f.ghosts.min(MAX_GHOSTS) {
        let t = 0.5 + i as f32 * 0.55;
        let (gx, gy) = (lx + vx * 2.0 * t, ly + vy * 2.0 * t);
        let r = s * (0.25 + 0.18 * ((i * 7) % 4) as f32);
        let g = (gx - x).hypot(gy - y) / r;
        if g < 1.4 {
            let k = 0.18 * (1.0 - (g / 1.4).powi(2)).max(0.0);
            let tint = [[1.0, 0.6, 0.3], [0.4, 1.0, 0.6], [0.5, 0.6, 1.0], [1.0, 0.4, 0.9]][(i % 4) as usize];
            for ch in 0..3 {
                out[ch] += k * tint[ch] * (0.5 + 0.5 * c[ch]);
            }
        }
    }
    out
}

/// Most ghosts on one flare.
pub const MAX_GHOSTS: u32 = 16;

/// Add a lens flare's light over the picture (screen, so it lights transparent areas too).
pub(crate) fn lens_flare(img: &mut RgbaImage, origin: (u32, u32), f: &Flare) {
    if f.intensity <= 0.0 {
        return;
    }
    let w = img.width as usize;
    par::rows(&mut img.data, w * 4, |y, row| {
        let cy = (origin.1 as usize + y) as f32 + 0.5;
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let cx = (origin.0 as usize + x) as f32 + 0.5;
            let l = flare_light(f, cx, cy).map(|v| (v * f.intensity).clamp(0.0, 1.0));
            let la = l[0].max(l[1]).max(l[2]);
            if la <= 0.5 / 255.0 {
                continue;
            }
            // premultiplied screen of the light (alpha = its strongest channel) over the pixel
            let scr = |dst: u8, src: f32| (dst as f32 + src * (255.0 - dst as f32)).round().clamp(0.0, 255.0) as u8;
            let a = scr(p[3], la);
            for ch in 0..3 {
                p[ch] = scr(p[ch], l[ch]).min(a);
            }
            p[3] = a;
        }
    });
}

/// Move the picture by another layer's pixels (`map`, the same size and place as `img`): its red
/// moves pixels across by up to `amount_x`, its green down by up to `amount_y` (mid grey: no
/// move); with `brightness`, its brightness moves them both ways. Where the map is transparent
/// nothing moves.
pub(crate) fn displace_by(img: &mut RgbaImage, map: &RgbaImage, amount_x: f32, amount_y: f32, brightness: bool) {
    if (amount_x == 0.0 && amount_y == 0.0) || map.width != img.width || map.height != img.height {
        return;
    }
    let src = img.clone();
    let w = img.width as usize;
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let Some(m) = map.data.get((y * w + x) * 4..(y * w + x) * 4 + 4) else { continue };
            if m[3] == 0 {
                continue;
            }
            let s = unpremultiply([m[0], m[1], m[2], m[3]]);
            let k = m[3] as f32 / 255.0;
            let (u, v) = if brightness {
                let l = luma(s) * 2.0 - 1.0;
                (l, l)
            } else {
                (s[0] as f32 / 127.5 - 1.0, s[1] as f32 / 127.5 - 1.0)
            };
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let out = select::sample(&src, fx - u * amount_x * k, fy - v * amount_y * k);
            p.copy_from_slice(&out);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, c: [u8; 4]) -> RgbaImage {
        let mut i = RgbaImage::transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                i.set_pixel(x, y, premultiply(c));
            }
        }
        i
    }

    #[test]
    fn colour_looks() {
        let mut g = img(4, 4, [255, 255, 255, 255]);
        gradient_map(&mut g, [10, 0, 50], [250, 200, 0], 1.0);
        assert_eq!(g.pixel(0, 0), [250, 200, 0, 255], "white maps to the light colour");
        let mut p = img(2, 2, [100, 150, 200, 255]);
        posterize(&mut p, 2);
        assert_eq!(p.pixel(0, 0), [0, 255, 255, 255]);
        let mut t = img(2, 2, [200, 200, 200, 255]);
        threshold(&mut t, 0.5);
        assert_eq!(t.pixel(1, 1), [255, 255, 255, 255]);
        let mut v = img(100, 10, [200, 200, 200, 255]);
        vignette(&mut v, (0, 0), (0.0, 5.0), 10.0, 20.0, 1.0);
        assert_eq!(v.pixel(2, 5), [200, 200, 200, 255], "inside the radius");
        assert_eq!(v.pixel(90, 5)[0], 0, "outside, fully dark");
    }

    #[test]
    fn distortions_move_the_picture_and_zero_does_nothing() {
        let mut base = RgbaImage::transparent(64, 64);
        for y in 0..64 {
            base.set_pixel(32, y, [255, 255, 255, 255]);
        }
        let mut i = base.clone();
        ripple(&mut i, (0, 0), (10.0, 10.0), 0.0, 20.0, 0.0);
        wave(&mut i, (0, 0), 0.0, 20.0, 0.0, 0.0);
        turbulence(&mut i, (0, 0), 0.0, 20.0, 3);
        assert_eq!(i, base, "zero amounts change nothing");
        let mut w = base.clone();
        wave(&mut w, (0, 0), 4.0, 32.0, 90.0, 0.0);
        assert_ne!(w, base, "a vertical line under a wave across it bends");
        let mut r = base.clone();
        ripple(&mut r, (0, 0), (10.0, 10.0), 3.0, 16.0, 0.25);
        assert_ne!(r, base);
        let mut t = base.clone();
        turbulence(&mut t, (0, 0), 4.0, 16.0, 7);
        assert_ne!(t, base);
        let mut c = base.clone();
        chromatic_aberration(&mut c, 2.0);
        assert!(c.pixel(30, 10)[0] > 0 && c.pixel(34, 10)[2] > 0, "red and blue moved apart: {:?} {:?}", c.pixel(30, 10), c.pixel(34, 10));
        // smoothing keeps a solid block solid
        let mut s = img(40, 40, [0, 0, 0, 255]);
        smooth(&mut s, 2.0);
        assert_eq!(s.pixel(20, 20)[3], 255);
    }

    #[test]
    fn lens_flare_lights_the_picture() {
        let f = Flare { light: (10.0, 10.0), through: (32.0, 32.0), size: 8.0, intensity: 1.0, colour: [255, 220, 180], rays: 6, ghosts: 3 };
        let mut i = RgbaImage::transparent(64, 64);
        lens_flare(&mut i, (0, 0), &f);
        assert!(i.pixel(10, 10)[3] > 200, "the core is bright: {:?}", i.pixel(10, 10));
        // the first ghost sits on the line through the centre, past it
        let g = i.pixel(32, 32);
        assert!(g[3] > 0, "a ghost on the line: {g:?}");
        assert!(i.data.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]));
        let mut off = RgbaImage::transparent(64, 64);
        lens_flare(&mut off, (0, 0), &Flare { intensity: 0.0, ..f });
        assert!(off.data.iter().all(|v| *v == 0), "no strength, no light");
    }

    #[test]
    fn displacement_follows_the_map() {
        // a 1 px vertical line at x = 20; the map's red pushes pixels 4 px to the right
        let mut i = RgbaImage::transparent(40, 20);
        for y in 0..20 {
            i.set_pixel(20, y, [0, 0, 0, 255]);
        }
        let map = img(40, 20, [255, 128, 0, 255]);
        let mut d = i.clone();
        displace_by(&mut d, &map, 4.0, 0.0, false);
        assert_eq!(d.pixel(20, 5)[3], 0, "the line moved away");
        assert!(d.pixel(24, 5)[3] > 200, "to the right: {:?}", d.pixel(24, 5));
        let mut none = i.clone();
        displace_by(&mut none, &RgbaImage::transparent(40, 20), 4.0, 4.0, false);
        assert_eq!(none, i, "a transparent map moves nothing");
        let mut b = i.clone();
        displace_by(&mut b, &img(40, 20, [0, 0, 0, 255]), 3.0, 0.0, true);
        assert!(b.pixel(17, 5)[3] > 200, "dark moves the other way: {:?}", b.pixel(17, 5));
    }
}
