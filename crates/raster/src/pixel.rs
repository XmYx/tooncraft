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

/// How a layer combines with what is below it. The formulas are the public W3C compositing
/// ones (separable modes per channel, then hue, saturation, colour and luminosity on whole colours).
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
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    LinearBurn,
    LinearLight,
    VividLight,
    PinLight,
    HardMix,
    Subtract,
    Divide,
    Hue,
    Saturation,
    Colour,
    Luminosity,
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_colour(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        for v in &mut out {
            *v = l + (*v - l) * l / (l - n).max(1e-6);
        }
    }
    if x > 1.0 {
        for v in &mut out {
            *v = l + (*v - l) * (1.0 - l) / (x - l).max(1e-6);
        }
    }
    out
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_colour([c[0] + d, c[1] + d, c[2] + d])
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
    let [lo, mid, hi] = idx;
    let mut out = [0.0f32; 3];
    if c[hi] > c[lo] {
        out[mid] = (c[mid] - c[lo]) * s / (c[hi] - c[lo]);
        out[hi] = s;
    }
    out
}

impl BlendMode {
    pub const ALL: [BlendMode; 24] = [
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
        BlendMode::Darken,
        BlendMode::Lighten,
        BlendMode::Add,
        BlendMode::ColorDodge,
        BlendMode::ColorBurn,
        BlendMode::HardLight,
        BlendMode::SoftLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::LinearBurn,
        BlendMode::LinearLight,
        BlendMode::VividLight,
        BlendMode::PinLight,
        BlendMode::HardMix,
        BlendMode::Subtract,
        BlendMode::Divide,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Colour,
        BlendMode::Luminosity,
    ];

    /// Stable id for command files and the CLI.
    pub fn id(self) -> &'static str {
        match self {
            BlendMode::Normal => "normal",
            BlendMode::Multiply => "multiply",
            BlendMode::Screen => "screen",
            BlendMode::Overlay => "overlay",
            BlendMode::Darken => "darken",
            BlendMode::Lighten => "lighten",
            BlendMode::Add => "add",
            BlendMode::ColorDodge => "color_dodge",
            BlendMode::ColorBurn => "color_burn",
            BlendMode::HardLight => "hard_light",
            BlendMode::SoftLight => "soft_light",
            BlendMode::Difference => "difference",
            BlendMode::Exclusion => "exclusion",
            BlendMode::LinearBurn => "linear_burn",
            BlendMode::LinearLight => "linear_light",
            BlendMode::VividLight => "vivid_light",
            BlendMode::PinLight => "pin_light",
            BlendMode::HardMix => "hard_mix",
            BlendMode::Subtract => "subtract",
            BlendMode::Divide => "divide",
            BlendMode::Hue => "hue",
            BlendMode::Saturation => "saturation",
            BlendMode::Colour => "colour",
            BlendMode::Luminosity => "luminosity",
        }
    }

    fn channel(self, b: f32, s: f32) -> f32 {
        let dodge = |b: f32, s: f32| {
            if b <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        };
        let burn = |b: f32, s: f32| {
            if b >= 1.0 {
                1.0
            } else if s <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        };
        match self {
            BlendMode::Normal | BlendMode::Hue | BlendMode::Saturation | BlendMode::Colour | BlendMode::Luminosity => s,
            BlendMode::Multiply => b * s,
            BlendMode::Screen => b + s - b * s,
            BlendMode::Overlay => BlendMode::HardLight.channel(s, b),
            BlendMode::Darken => b.min(s),
            BlendMode::Lighten => b.max(s),
            BlendMode::Add => (b + s).min(1.0),
            BlendMode::ColorDodge => dodge(b, s),
            BlendMode::ColorBurn => burn(b, s),
            BlendMode::HardLight => {
                if s <= 0.5 {
                    b * 2.0 * s
                } else {
                    let s2 = 2.0 * s - 1.0;
                    b + s2 - b * s2
                }
            }
            BlendMode::SoftLight => {
                if s <= 0.5 {
                    b - (1.0 - 2.0 * s) * b * (1.0 - b)
                } else {
                    let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
                    b + (2.0 * s - 1.0) * (d - b)
                }
            }
            BlendMode::Difference => (b - s).abs(),
            BlendMode::Exclusion => b + s - 2.0 * b * s,
            BlendMode::LinearBurn => (b + s - 1.0).max(0.0),
            BlendMode::LinearLight => (b + 2.0 * s - 1.0).clamp(0.0, 1.0),
            BlendMode::VividLight => {
                if s <= 0.5 {
                    burn(b, 2.0 * s)
                } else {
                    dodge(b, 2.0 * s - 1.0)
                }
            }
            BlendMode::PinLight => {
                if s < 0.5 {
                    b.min(2.0 * s)
                } else {
                    b.max(2.0 * s - 1.0)
                }
            }
            BlendMode::HardMix => {
                if b + s >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            BlendMode::Subtract => (b - s).max(0.0),
            BlendMode::Divide => {
                if s <= 0.0 {
                    1.0
                } else {
                    (b / s).min(1.0)
                }
            }
        }
    }

    /// The mixed colour for straight backdrop `b` and source `s`.
    fn mix(self, b: [f32; 3], s: [f32; 3]) -> [f32; 3] {
        let out = match self {
            BlendMode::Hue => set_lum(set_sat(s, sat(b)), lum(b)),
            BlendMode::Saturation => set_lum(set_sat(b, sat(s)), lum(b)),
            BlendMode::Colour => set_lum(s, lum(b)),
            BlendMode::Luminosity => set_lum(b, lum(s)),
            _ => [self.channel(b[0], s[0]), self.channel(b[1], s[1]), self.channel(b[2], s[2])],
        };
        out.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 })
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
    let straight = |p: [u8; 4], a: f32| -> [f32; 3] { if a > 0.0 { [0, 1, 2].map(|k| (p[k] as f32 / 255.0 / a).min(1.0)) } else { [0.0; 3] } };
    let (du, su) = (straight(dst, da), straight(src, sa));
    let mixed = mode.mix(du, su);
    let mut out = [0u8; 4];
    for k in 0..3 {
        let sc = src[k] as f32 / 255.0;
        let dc = dst[k] as f32 / 255.0;
        let co = sc * (1.0 - da) + dc * (1.0 - sa) + sa * da * mixed[k];
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

    #[test]
    fn every_mode_stays_a_valid_premultiplied_colour() {
        let samples = [[0u8, 0, 0, 0], [0, 0, 0, 255], [255, 255, 255, 255], [200, 30, 90, 255], [60, 20, 10, 100], [255, 128, 0, 255], [5, 5, 5, 8]];
        for mode in BlendMode::ALL {
            for d in samples {
                for s in samples {
                    let o = blend(d, s, mode);
                    assert!(o[0] <= o[3] && o[1] <= o[3] && o[2] <= o[3], "{mode:?} {d:?} {s:?} gave {o:?}");
                    if s[3] == 0 {
                        assert_eq!(o, d, "{mode:?}: a clear source leaves the backdrop");
                    }
                }
            }
        }
    }

    #[test]
    fn modes_follow_their_definitions() {
        let (gray, red, white, black) = ([128u8, 128, 128, 255], [255u8, 0, 0, 255], [255u8, 255, 255, 255], [0u8, 0, 0, 255]);
        assert_eq!(blend(red, red, BlendMode::Difference), black);
        assert_eq!(blend(white, red, BlendMode::Difference), [0, 255, 255, 255]);
        assert_eq!(blend(white, white, BlendMode::Exclusion), black);
        assert_eq!(blend(gray, black, BlendMode::ColorDodge), gray, "dodging by black changes nothing");
        assert_eq!(blend(gray, white, BlendMode::ColorDodge), white);
        assert_eq!(blend(white, white, BlendMode::ColorBurn), white);
        assert_eq!(blend(gray, black, BlendMode::ColorBurn), black);
        assert_eq!(blend(gray, gray, BlendMode::Subtract)[0], 0);
        assert_eq!(blend(gray, white, BlendMode::Divide), gray);
        assert_eq!(blend(gray, black, BlendMode::Divide), white);
        assert_eq!(blend([100, 100, 100, 255], [200, 200, 200, 255], BlendMode::HardMix), white);
        assert_eq!(blend([100, 100, 100, 255], [100, 100, 100, 255], BlendMode::HardMix), black);
        // hard light with a middle-grey source leaves the backdrop alone (within rounding)
        let hl = blend([40, 120, 220, 255], gray, BlendMode::HardLight);
        assert!(hl[0].abs_diff(40) <= 2 && hl[1].abs_diff(120) <= 2 && hl[2].abs_diff(220) <= 2, "{hl:?}");
        // soft light is gentler than hard light and keeps black and white
        assert_eq!(blend(black, gray, BlendMode::SoftLight), black);
        assert_eq!(blend(white, gray, BlendMode::SoftLight)[0], 255);
        // luminosity: the backdrop's colour at the source's brightness
        let lumin = blend(red, gray, BlendMode::Luminosity);
        assert!(lumin[0] > lumin[1] && lumin[1] == lumin[2], "keeps the red hue: {lumin:?}");
        // colour mode keeps the backdrop's brightness and takes the source's hue
        let colour = blend(gray, red, BlendMode::Colour);
        assert!(colour[0] > colour[1], "takes the red: {colour:?}");
        // hue / saturation of a grey backdrop have nothing to rotate or scale
        assert_eq!(blend(gray, red, BlendMode::Hue), gray);
        assert_eq!(blend(red, gray, BlendMode::Saturation), [77, 77, 77, 255], "no saturation left: the backdrop's brightness as grey");
        for mode in BlendMode::ALL {
            assert_eq!(BlendMode::ALL.iter().filter(|m| m.id() == mode.id()).count(), 1, "ids are unique");
        }
    }
}
