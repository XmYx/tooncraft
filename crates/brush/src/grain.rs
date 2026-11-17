//! Procedural paper grain: value noise from an integer hash, deterministic and tileable-free.

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ 0x2545_f491;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let a = hash(ix, iy);
    let b = hash(ix + 1, iy);
    let c = hash(ix, iy + 1);
    let d = hash(ix + 1, iy + 1);
    let top = a + (b - a) * tx;
    let bottom = c + (d - c) * tx;
    top + (bottom - top) * ty
}

/// Paper grain height at a canvas position, 0 (valley) … 1 (peak). Two octaves: fibres and tooth.
pub fn paper_grain(x: f32, y: f32, scale: f32) -> f32 {
    let s = scale.max(0.5);
    let n = 0.65 * value_noise(x / s, y / s) + 0.35 * value_noise(x / (s * 0.43) + 17.3, y / (s * 0.43) - 5.1);
    n.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grain_is_deterministic_and_in_range() {
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        for i in 0..2000 {
            let v = paper_grain(i as f32 * 0.37, i as f32 * 1.13, 2.0);
            assert_eq!(v, paper_grain(i as f32 * 0.37, i as f32 * 1.13, 2.0));
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(lo >= 0.0 && hi <= 1.0 && hi - lo > 0.4, "{lo}..{hi}");
    }
}
