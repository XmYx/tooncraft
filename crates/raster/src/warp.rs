//! Compositing a surface through an affine map (pegs, the multiplane camera): every output pixel
//! is mapped back into the surface and sampled bilinearly, tile by tile, so empty space costs
//! nothing.

use crate::select::Affine;
use crate::{RgbaImage, Surface, TILE, mul, over, scale_alpha};

/// Largest output area composited through one map (pixels); bigger requests are clipped.
const MAX_WARP_PIXELS: u64 = 1 << 28;

impl Surface {
    /// Composite this surface over `dst` through `t` (surface pixels → canvas pixels); `dst` is
    /// the canvas rectangle with its top-left at `origin`. A map that cannot be inverted (zero
    /// scale) or is not finite draws nothing.
    pub fn composite_affine(&self, dst: &mut RgbaImage, origin: (u32, u32), t: &Affine, opacity: f32, tint: Option<[u8; 3]>) {
        let o = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
        if o == 0 || dst.width == 0 || dst.height == 0 || !t.is_finite() {
            return;
        }
        if t.is_identity(1e-6) {
            self.composite_region(dst, origin.0, origin.1, opacity, tint);
            return;
        }
        let Some(inv) = t.inverse() else { return };
        if (dst.width as u64) * (dst.height as u64) > MAX_WARP_PIXELS {
            return;
        }
        let (ox, oy) = (origin.0 as f32, origin.1 as f32);
        let (sw, sh) = (self.width(), self.height());
        for (index, _) in self.tiles() {
            let (tx, ty) = self.tile_coords(index);
            let (x0, y0) = ((tx * TILE) as f32, (ty * TILE) as f32);
            let (x1, y1) = (((tx + 1) * TILE).min(sw) as f32, ((ty + 1) * TILE).min(sh) as f32);
            // output bounding box of the tile (one pixel of margin for the bilinear footprint)
            let pts = [t.apply(x0 - 1.0, y0 - 1.0), t.apply(x1 + 1.0, y0 - 1.0), t.apply(x1 + 1.0, y1 + 1.0), t.apply(x0 - 1.0, y1 + 1.0)];
            let (mut bx0, mut by0, mut bx1, mut by1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for (x, y) in pts {
                bx0 = bx0.min(x);
                by0 = by0.min(y);
                bx1 = bx1.max(x);
                by1 = by1.max(y);
            }
            let cx0 = (bx0 - ox).floor().max(0.0) as u32;
            let cy0 = (by0 - oy).floor().max(0.0) as u32;
            let cx1 = ((bx1 - ox).ceil().max(0.0) as u32).min(dst.width);
            let cy1 = ((by1 - oy).ceil().max(0.0) as u32).min(dst.height);
            for y in cy0..cy1 {
                for x in cx0..cx1 {
                    let (sx, sy) = inv.apply(x as f32 + 0.5 + ox, y as f32 + 0.5 + oy);
                    // each output pixel belongs to the one tile its source position falls in
                    if !(sx >= x0 && sx < x1 && sy >= y0 && sy < y1) {
                        continue;
                    }
                    let s = self.sample(sx, sy);
                    if s[3] == 0 {
                        continue;
                    }
                    let mut px = s;
                    if let Some(c) = tint {
                        px = [mul(c[0], s[3]), mul(c[1], s[3]), mul(c[2], s[3]), s[3]];
                    }
                    if o < 255 {
                        px = scale_alpha(px, o as u8);
                    }
                    let d = dst.pixel(x, y);
                    dst.set_pixel(x, y, over(d, px));
                }
            }
        }
    }

    /// Bilinear sample at a position in pixels (centres at +0.5); transparent outside.
    pub fn sample(&self, x: f32, y: f32) -> [u8; 4] {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (wx, wy) = (fx - x0, fy - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let get = |x: i64, y: i64| {
            let p = self.pixel(x, y);
            [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
        };
        let (p00, p10, p01, p11) = (get(ix, iy), get(ix + 1, iy), get(ix, iy + 1), get(ix + 1, iy + 1));
        let mut out = [0u8; 4];
        for k in 0..4 {
            let top = p00[k] + (p10[k] - p00[k]) * wx;
            let bottom = p01[k] + (p11[k] - p01[k]) * wx;
            out[k] = (top + (bottom - top) * wy).round().clamp(0.0, 255.0) as u8;
        }
        let a = out[3];
        [out[0].min(a), out[1].min(a), out[2].min(a), a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot() -> Surface {
        let mut s = Surface::new(200, 100).unwrap();
        for y in 10..20 {
            for x in 10..20 {
                let i = s.tile_index(x / TILE, y / TILE).unwrap();
                let t = s.tile_mut(i).unwrap();
                let o = (((y % TILE) * TILE + x % TILE) * 4) as usize;
                t.bytes_mut()[o..o + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
        s
    }

    #[test]
    fn identity_matches_plain_composite() {
        let s = dot();
        let mut a = RgbaImage::transparent(200, 100);
        let mut b = RgbaImage::transparent(200, 100);
        s.composite_region(&mut a, 0, 0, 0.7, None);
        s.composite_affine(&mut b, (0, 0), &Affine::IDENTITY, 0.7, None);
        assert_eq!(a, b);
    }

    #[test]
    fn translation_and_scale_move_the_pixels() {
        let s = dot();
        let mut img = RgbaImage::transparent(200, 100);
        s.composite_affine(&mut img, (0, 0), &Affine::translate(100.0, 50.0), 1.0, None);
        assert_eq!(img.pixel(115, 65), [0, 0, 255, 255]);
        assert_eq!(img.pixel(15, 15), [0, 0, 0, 0]);
        let mut img = RgbaImage::transparent(200, 100);
        let t = Affine::about((0.0, 0.0), 0.0, 0.0, 2.0, 2.0, 0.0);
        s.composite_affine(&mut img, (0, 0), &t, 1.0, None);
        assert_eq!(img.pixel(30, 30), [0, 0, 255, 255]);
        assert_eq!(img.pixel(15, 15), [0, 0, 0, 0]);
        // offset destination rectangle
        let mut part = RgbaImage::transparent(20, 20);
        s.composite_affine(&mut part, (100, 50), &Affine::translate(100.0, 50.0), 1.0, None);
        assert_eq!(part.pixel(15, 15), [0, 0, 255, 255]);
    }

    #[test]
    fn degenerate_maps_draw_nothing() {
        let s = dot();
        let mut img = RgbaImage::transparent(200, 100);
        s.composite_affine(&mut img, (0, 0), &Affine { a: 0.0, d: 0.0, ..Affine::IDENTITY }, 1.0, None);
        s.composite_affine(&mut img, (0, 0), &Affine { tx: f32::NAN, ..Affine::IDENTITY }, 1.0, None);
        assert_eq!(img, RgbaImage::transparent(200, 100));
        let t = Affine::about((5.0, 5.0), 3.0, -2.0, 1.5, 0.5, 0.7);
        let back = t.then(&t.inverse().unwrap());
        assert!(back.is_identity(1e-4));
    }
}
