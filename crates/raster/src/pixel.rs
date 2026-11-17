//! Premultiplied RGBA8 pixel arithmetic.

/// `a·b/255`, rounded.
#[inline]
pub(crate) fn mul8(a: u8, b: u8) -> u8 {
    let t = a as u32 * b as u32 + 128;
    ((t + (t >> 8)) >> 8) as u8
}

/// Source-over for premultiplied pixels.
#[inline]
pub fn over(dst: [u8; 4], src: [u8; 4]) -> [u8; 4] {
    let inv = 255 - src[3];
    if inv == 0 {
        return src;
    }
    [
        src[0].saturating_add(mul8(dst[0], inv)),
        src[1].saturating_add(mul8(dst[1], inv)),
        src[2].saturating_add(mul8(dst[2], inv)),
        src[3].saturating_add(mul8(dst[3], inv)),
    ]
}

/// Multiply a premultiplied pixel by an opacity.
#[inline]
pub fn scale_alpha(p: [u8; 4], o: u8) -> [u8; 4] {
    [mul8(p[0], o), mul8(p[1], o), mul8(p[2], o), mul8(p[3], o)]
}

/// Straight → premultiplied.
#[inline]
pub fn premultiply(p: [u8; 4]) -> [u8; 4] {
    [mul8(p[0], p[3]), mul8(p[1], p[3]), mul8(p[2], p[3]), p[3]]
}

/// Premultiplied → straight.
#[inline]
pub fn unpremultiply(p: [u8; 4]) -> [u8; 4] {
    let a = p[3] as u32;
    if a == 0 {
        return [0; 4];
    }
    let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
    [un(p[0]), un(p[1]), un(p[2]), p[3]]
}

/// How a layer combines with what is below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Add,
}

impl BlendMode {
    pub const ALL: [BlendMode; 7] =
        [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay, BlendMode::Darken, BlendMode::Lighten, BlendMode::Add];

    fn channel(self, b: f32, s: f32) -> f32 {
        match self {
            BlendMode::Normal => s,
            BlendMode::Multiply => b * s,
            BlendMode::Screen => b + s - b * s,
            BlendMode::Overlay => {
                if b <= 0.5 {
                    2.0 * b * s
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
                }
            }
            BlendMode::Darken => b.min(s),
            BlendMode::Lighten => b.max(s),
            BlendMode::Add => (b + s).min(1.0),
        }
    }
}

/// Composite premultiplied `src` over `dst` with a blend mode (W3C compositing: separable modes,
/// source-over).
pub fn blend(dst: [u8; 4], src: [u8; 4], mode: BlendMode) -> [u8; 4] {
    if mode == BlendMode::Normal || src[3] == 0 {
        return over(dst, src);
    }
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let mut out = [0u8; 4];
    for k in 0..3 {
        let sc = src[k] as f32 / 255.0;
        let dc = dst[k] as f32 / 255.0;
        let (su, du) = (if sa > 0.0 { sc / sa } else { 0.0 }, if da > 0.0 { dc / da } else { 0.0 });
        let co = sc * (1.0 - da) + dc * (1.0 - sa) + sa * da * mode.channel(du.min(1.0), su.min(1.0));
        out[k] = (co * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    let ao = sa + da - sa * da;
    out[3] = (ao * 255.0).round().clamp(0.0, 255.0) as u8;
    for k in 0..3 {
        out[k] = out[k].min(out[3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic() {
        assert_eq!(mul8(255, 255), 255);
        assert_eq!(mul8(255, 0), 0);
        assert_eq!(mul8(128, 255), 128);
        assert_eq!(over([0, 0, 0, 0], [10, 20, 30, 40]), [10, 20, 30, 40]);
        assert_eq!(over([255, 255, 255, 255], [0, 0, 0, 255]), [0, 0, 0, 255]);
        assert_eq!(unpremultiply(premultiply([200, 100, 50, 255])), [200, 100, 50, 255]);
        let p = unpremultiply(premultiply([200, 100, 50, 128]));
        assert!(p[0].abs_diff(200) <= 2 && p[1].abs_diff(100) <= 2);
        let white = [255, 255, 255, 255];
        let red = [255, 0, 0, 255];
        assert_eq!(blend(white, red, BlendMode::Multiply), red);
        assert_eq!(blend([0, 0, 0, 255], red, BlendMode::Screen), red);
        assert_eq!(blend([100, 100, 100, 255], [50, 50, 50, 255], BlendMode::Darken), [50, 50, 50, 255]);
        assert_eq!(blend(white, [0, 0, 0, 0], BlendMode::Multiply), white);
        assert_eq!(blend([200, 0, 0, 255], [100, 100, 0, 255], BlendMode::Add), [255, 100, 0, 255]);
    }
}
