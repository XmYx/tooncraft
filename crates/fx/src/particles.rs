//! Particles: rain, snow, sparks and dust from a rectangle, worked out for each frame from the
//! particle's birth and a hash of its index (no state from frame to frame), so any frame renders
//! on its own, the same every time.

use tooncraft_raster::{RgbaImage, over, premultiply};

use crate::ParticleStyle;

/// Most particles alive at once.
pub const MAX_LIVE: u32 = 20_000;

pub(crate) struct Emitter {
    pub style: ParticleStyle,
    pub rect: (f32, f32, f32, f32),
    pub rate: f32,
    pub life: u32,
    pub speed: f32,
    pub angle: f32,
    pub spread: f32,
    pub gravity: f32,
    pub size: f32,
    pub colour: [u8; 3],
    pub opacity: f32,
    pub seed: u32,
}

fn hash(i: u64, k: u32, seed: u32) -> f32 {
    let mut h =
        (i as u32).wrapping_mul(0x9E37_79B9) ^ ((i >> 32) as u32).wrapping_mul(0x85EB_CA6B) ^ k.wrapping_mul(0xC2B2_AE35) ^ seed.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFF_FFFF) as f32 / 16_777_215.0
}

/// One particle at a frame: where it is, its tail (rain and sparks streak), radius and opacity.
struct Particle {
    pos: (f32, f32),
    tail: (f32, f32),
    radius: f32,
    alpha: f32,
}

fn particle(e: &Emitter, i: u64, age: f32) -> Particle {
    let h = |k| hash(i, k, e.seed);
    let start = (e.rect.0 + h(1) * e.rect.2, e.rect.1 + h(2) * e.rect.3);
    let a = (e.angle + (h(3) - 0.5) * e.spread).to_radians();
    let speed = e.speed * (0.7 + 0.6 * h(4));
    let (vx, vy) = (a.cos() * speed, a.sin() * speed);
    let mut pos = (start.0 + vx * age, start.1 + vy * age + 0.5 * e.gravity * age * age);
    let radius = (e.size * (0.6 + 0.8 * h(5)) / 2.0).max(0.3);
    let life = e.life.max(1) as f32;
    let mut alpha = e.opacity;
    let mut tail = pos;
    match e.style {
        ParticleStyle::Rain => {
            // a streak along the fall, as long as a third of a frame's travel
            let (tvx, tvy) = (vx, vy + e.gravity * age);
            tail = (pos.0 - tvx * 0.35, pos.1 - tvy * 0.35);
        }
        ParticleStyle::Snow => {
            pos.0 += (age * 0.15 + h(6) * 6.3).sin() * radius * 3.0;
        }
        ParticleStyle::Sparks => {
            let (tvx, tvy) = (vx, vy + e.gravity * age);
            tail = (pos.0 - tvx * 0.5, pos.1 - tvy * 0.5);
            alpha *= (1.0 - age / life).clamp(0.0, 1.0);
        }
        ParticleStyle::Dust => {
            alpha *= 0.5 + 0.5 * (age * 0.3 + h(7) * 6.3).sin().abs();
            alpha *= (1.0 - age / life).clamp(0.0, 1.0).sqrt();
        }
    }
    Particle { pos, tail, radius, alpha }
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 1e-6 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

/// Draw the particles alive at `frame` over `img` (which sits at `origin` on the canvas).
pub(crate) fn draw(img: &mut RgbaImage, origin: (u32, u32), e: &Emitter, frame: u32) {
    if e.rate <= 0.0 || e.opacity <= 0.0 {
        return;
    }
    let life = e.life.max(1) as f64;
    let rate = e.rate as f64;
    let f = frame as f64;
    // particle i is born at i / rate; alive while its age is under `life`
    let first = ((f - life) * rate).ceil().max(0.0) as u64;
    let last = (f * rate).floor().max(0.0) as u64;
    let last = last.min(first.saturating_add(MAX_LIVE as u64));
    let (w, h) = (img.width as i64, img.height as i64);
    let (ox, oy) = (origin.0 as f32, origin.1 as f32);
    for i in first..=last {
        let age = (f - i as f64 / rate) as f32;
        if age < 0.0 || age as f64 >= life {
            continue;
        }
        let p = particle(e, i, age);
        if p.alpha <= 0.0 {
            continue;
        }
        let r = p.radius + 1.0;
        let (x0, x1) = ((p.pos.0.min(p.tail.0) - r - ox).floor() as i64, (p.pos.0.max(p.tail.0) + r - ox).ceil() as i64);
        let (y0, y1) = ((p.pos.1.min(p.tail.1) - r - oy).floor() as i64, (p.pos.1.max(p.tail.1) + r - oy).ceil() as i64);
        if x1 < 0 || y1 < 0 || x0 >= w || y0 >= h || (x1 - x0) * (y1 - y0) > 1 << 16 {
            continue;
        }
        for y in y0.max(0)..y1.min(h) {
            for x in x0.max(0)..x1.min(w) {
                let c = (x as f32 + 0.5 + ox, y as f32 + 0.5 + oy);
                let d = segment_distance(c, p.tail, p.pos);
                let k = (p.radius + 0.5 - d).clamp(0.0, 1.0) * p.alpha;
                if k <= 0.0 {
                    continue;
                }
                let a = (k * 255.0).round() as u8;
                let src = premultiply([e.colour[0], e.colour[1], e.colour[2], a]);
                let dst = img.pixel(x as u32, y as u32);
                img.set_pixel(x as u32, y as u32, over(dst, src));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emitter(style: ParticleStyle) -> Emitter {
        Emitter {
            style,
            rect: (0.0, 0.0, 64.0, 1.0),
            rate: 20.0,
            life: 30,
            speed: 3.0,
            angle: 90.0,
            spread: 10.0,
            gravity: 0.0,
            size: 3.0,
            colour: [255, 255, 255],
            opacity: 1.0,
            seed: 3,
        }
    }

    fn painted(img: &RgbaImage) -> usize {
        img.data.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count()
    }

    #[test]
    fn particles_fall_and_repeat_exactly() {
        for style in [ParticleStyle::Rain, ParticleStyle::Snow, ParticleStyle::Sparks, ParticleStyle::Dust] {
            let mut a = RgbaImage::transparent(64, 64);
            draw(&mut a, (0, 0), &emitter(style), 20);
            assert!(painted(&a) > 0, "{style:?} draws something");
            let mut b = RgbaImage::transparent(64, 64);
            draw(&mut b, (0, 0), &emitter(style), 20);
            assert_eq!(a, b, "{style:?}: the same frame twice is the same picture");
            assert!(a.data.as_chunks::<4>().0.iter().all(|p| p[0] <= p[3]));
        }
        let mut start = RgbaImage::transparent(64, 64);
        draw(&mut start, (0, 0), &emitter(ParticleStyle::Snow), 0);
        let mut later = RgbaImage::transparent(64, 64);
        draw(&mut later, (0, 0), &emitter(ParticleStyle::Snow), 15);
        assert!(painted(&later) > painted(&start), "more fall as time goes on");
        // a part of the canvas gets the same pixels as the whole
        let mut whole = RgbaImage::transparent(64, 64);
        draw(&mut whole, (0, 0), &emitter(ParticleStyle::Rain), 12);
        let mut part = RgbaImage::transparent(30, 20);
        draw(&mut part, (10, 30), &emitter(ParticleStyle::Rain), 12);
        assert_eq!(part, whole.crop(10, 30, 30, 20));
    }

    #[test]
    fn huge_rates_stay_bounded() {
        let e = Emitter { rate: 1e9, life: 600, size: 1.0, ..emitter(ParticleStyle::Dust) };
        let mut img = RgbaImage::transparent(16, 16);
        draw(&mut img, (0, 0), &e, 100_000);
        // finishes: at most MAX_LIVE particles are looked at
    }
}
