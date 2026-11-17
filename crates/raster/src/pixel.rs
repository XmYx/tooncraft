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
    }
}
