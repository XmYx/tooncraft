//! Selections and their transforms: a polygon (lasso or rectangle) becomes a [`Mask`]; the
//! pixels under it can be cleared, copied, or moved / scaled / rotated / flipped with bilinear
//! resampling.

use crate::fill::Mask;
use crate::{RasterError, RgbaImage, Surface, TILE, over};

/// Largest selection area handled at once (pixels).
pub const MAX_SELECTION_PIXELS: u64 = 64_000_000;

/// `x' = a·x + b·y + tx`, `y' = c·x + d·y + ty`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Affine {
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    /// Scale and rotate (radians) about `pivot`, then move by `(dx, dy)`. Negative scales flip.
    pub fn about(pivot: (f32, f32), dx: f32, dy: f32, sx: f32, sy: f32, angle: f32) -> Affine {
        let (s, c) = angle.sin_cos();
        let (a, b, cc, d) = (c * sx, -s * sy, s * sx, c * sy);
        let (px, py) = pivot;
        Affine { a, b, c: cc, d, tx: px + dx - (a * px + b * py), ty: py + dy - (cc * px + d * py) }
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.b * y + self.tx, self.c * x + self.d * y + self.ty)
    }

    pub fn inverse(&self) -> Option<Affine> {
        let det = self.a * self.d - self.b * self.c;
        if !det.is_finite() || det.abs() < 1e-8 {
            return None;
        }
        let (a, b, c, d) = (self.d / det, -self.b / det, -self.c / det, self.a / det);
        Some(Affine { a, b, c, d, tx: -(a * self.tx + b * self.ty), ty: -(c * self.tx + d * self.ty) })
    }

    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.tx, self.ty].iter().all(|v| v.is_finite())
    }
}

/// Pixels whose centres lie inside the polygon (even-odd rule), clipped to `w`×`h`.
pub fn polygon_mask(w: u32, h: u32, poly: &[(f32, f32)]) -> Option<Mask> {
    if poly.len() < 3 || poly.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    let bx0 = x0.floor().max(0.0) as u32;
    let by0 = y0.floor().max(0.0) as u32;
    let bx1 = (x1.ceil().max(0.0) as u32).min(w);
    let by1 = (y1.ceil().max(0.0) as u32).min(h);
    if bx0 >= bx1 || by0 >= by1 || (bx1 - bx0) as u64 * (by1 - by0) as u64 > MAX_SELECTION_PIXELS {
        return None;
    }
    let (mw, mh) = (bx1 - bx0, by1 - by0);
    let mut data = vec![0u8; mw as usize * mh as usize];
    let mut xs: Vec<f32> = Vec::new();
    for row in 0..mh {
        let cy = (by0 + row) as f32 + 0.5;
        xs.clear();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            if (a.1 <= cy) != (b.1 <= cy) {
                xs.push(a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.as_chunks::<2>().0 {
            let start = ((pair[0] - 0.5).ceil().max(bx0 as f32) as u32).max(bx0);
            let end = ((pair[1] - 0.5).floor().min((bx1 - 1) as f32)) as i64;
            let mut x = start as i64;
            while x <= end {
                data[row as usize * mw as usize + (x as u32 - bx0) as usize] = 255;
                x += 1;
            }
        }
    }
    data.iter().any(|v| *v != 0).then_some(Mask { x: bx0, y: by0, width: mw, height: mh, data })
}

/// The pixels under the mask, as an image the size of the mask's box.
pub fn copy(surface: &Surface, mask: &Mask) -> RgbaImage {
    let mut img = RgbaImage::transparent(mask.width, mask.height);
    surface.composite_region(&mut img, mask.x, mask.y, 1.0, None);
    for (i, m) in mask.data.iter().enumerate() {
        if *m == 0 {
            let at = i * 4;
            if let Some(px) = img.data.get_mut(at..at + 4) {
                px.copy_from_slice(&[0; 4]);
            }
        }
    }
    img
}

/// Clear the pixels under the mask.
pub fn clear(surface: &mut Surface, mask: &Mask) {
    if mask.width == 0 || mask.height == 0 {
        return;
    }
    for ty in mask.y / TILE..=(mask.y + mask.height - 1) / TILE {
        for tx in mask.x / TILE..=(mask.x + mask.width - 1) / TILE {
            let touched = (0..TILE).any(|r| (0..TILE).any(|c| mask.get(tx * TILE + c, ty * TILE + r)));
            if !touched || surface.tile(surface.tile_index(tx, ty).unwrap_or(u32::MAX)).is_none() {
                continue;
            }
            let Some(index) = surface.tile_index(tx, ty) else { continue };
            let Some(tile) = surface.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for r in 0..TILE {
                for c in 0..TILE {
                    if mask.get(tx * TILE + c, ty * TILE + r) {
                        let i = ((r * TILE + c) * 4) as usize;
                        px[i..i + 4].copy_from_slice(&[0; 4]);
                    }
                }
            }
        }
    }
    surface.prune();
}

/// Bilinear sample of a premultiplied image at a position in pixels (centres at +0.5);
/// transparent outside.
pub fn sample(img: &RgbaImage, x: f32, y: f32) -> [u8; 4] {
    // bilinear on premultiplied pixels, transparent outside
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let get = |x: f32, y: f32| -> [f32; 4] {
        if x < 0.0 || y < 0.0 || x >= img.width as f32 || y >= img.height as f32 {
            return [0.0; 4];
        }
        let p = img.pixel(x as u32, y as u32);
        [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32]
    };
    let (p00, p10, p01, p11) = (get(x0, y0), get(x0 + 1.0, y0), get(x0, y0 + 1.0), get(x0 + 1.0, y0 + 1.0));
    let mut out = [0u8; 4];
    for k in 0..4 {
        let top = p00[k] + (p10[k] - p00[k]) * tx;
        let bottom = p01[k] + (p11[k] - p01[k]) * tx;
        out[k] = (top + (bottom - top) * ty).round().clamp(0.0, 255.0) as u8;
    }
    let a = out[3];
    [out[0].min(a), out[1].min(a), out[2].min(a), a]
}

/// Draw `img` (placed with its top-left at `origin`, then mapped by `t`) over the surface.
pub fn draw_transformed(surface: &mut Surface, img: &RgbaImage, origin: (f32, f32), t: Affine) -> Result<(), RasterError> {
    let inv = t.inverse().ok_or(RasterError::BadImage { got: 0, want: 1 })?;
    let corners = [(0.0, 0.0), (img.width as f32, 0.0), (0.0, img.height as f32), (img.width as f32, img.height as f32)];
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for (cx, cy) in corners {
        let (x, y) = t.apply(cx + origin.0, cy + origin.1);
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }
    let bx0 = x0.floor().max(0.0) as u32;
    let by0 = y0.floor().max(0.0) as u32;
    let bx1 = (x1.ceil().max(0.0) as u32).min(surface.width());
    let by1 = (y1.ceil().max(0.0) as u32).min(surface.height());
    if bx0 >= bx1 || by0 >= by1 {
        return Ok(());
    }
    if (bx1 - bx0) as u64 * (by1 - by0) as u64 > MAX_SELECTION_PIXELS {
        return Err(RasterError::FillTooLarge(MAX_SELECTION_PIXELS));
    }
    for ty in by0 / TILE..=(by1 - 1) / TILE {
        for tx in bx0 / TILE..=(bx1 - 1) / TILE {
            let Some(index) = surface.tile_index(tx, ty) else { continue };
            let Some(tile) = surface.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for r in 0..TILE {
                let y = ty * TILE + r;
                if y < by0 || y >= by1 {
                    continue;
                }
                for c in 0..TILE {
                    let x = tx * TILE + c;
                    if x < bx0 || x >= bx1 {
                        continue;
                    }
                    let (sx, sy) = inv.apply(x as f32 + 0.5, y as f32 + 0.5);
                    let s = sample(img, sx - origin.0, sy - origin.1);
                    if s[3] == 0 {
                        continue;
                    }
                    let i = ((r * TILE + c) * 4) as usize;
                    let out = over([px[i], px[i + 1], px[i + 2], px[i + 3]], s);
                    px[i..i + 4].copy_from_slice(&out);
                }
            }
        }
    }
    surface.prune();
    Ok(())
}

/// Move / scale / rotate / flip the pixels under `mask` by `t`.
pub fn transform(surface: &mut Surface, mask: &Mask, t: Affine) -> Result<(), RasterError> {
    if !t.is_finite() {
        return Err(RasterError::BadImage { got: 0, want: 1 });
    }
    let img = copy(surface, mask);
    clear(surface, mask);
    draw_transformed(surface, &img, (mask.x as f32, mask.y as f32), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dot_surface() -> Surface {
        let mut s = Surface::new(200, 200).unwrap();
        for y in 20..40 {
            for x in 20..40 {
                let i = s.tile_index(x / TILE, y / TILE).unwrap();
                let t = s.tile_mut(i).unwrap();
                let at = (((y % TILE) * TILE + x % TILE) * 4) as usize;
                t.bytes_mut()[at..at + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        s
    }

    #[test]
    fn masks() {
        let m = polygon_mask(100, 100, &[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]).unwrap();
        assert_eq!((m.x, m.y, m.count()), (10, 10, 100));
        assert!(polygon_mask(100, 100, &[(1.0, 1.0), (2.0, 2.0)]).is_none());
        assert!(polygon_mask(100, 100, &[(-50.0, -50.0), (-40.0, -50.0), (-40.0, -40.0)]).is_none());
        let tri = polygon_mask(100, 100, &[(0.0, 0.0), (100.0, 0.0), (0.0, 100.0)]).unwrap();
        assert!(tri.get(10, 10) && !tri.get(90, 90));
    }

    #[test]
    fn move_flip_rotate() {
        let mut s = dot_surface();
        let m = polygon_mask(200, 200, &[(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)]).unwrap();
        transform(&mut s, &m, Affine::about((30.0, 30.0), 100.0, 50.0, 1.0, 1.0, 0.0)).unwrap();
        assert_eq!(s.pixel(25, 25)[3], 0, "the source area is cleared");
        assert_eq!(s.pixel(125, 75), [0, 0, 0, 255], "moved by (100, 50)");
        let m = polygon_mask(200, 200, &[(110.0, 60.0), (150.0, 60.0), (150.0, 100.0), (110.0, 100.0)]).unwrap();
        transform(&mut s, &m, Affine::about((130.0, 80.0), 0.0, 0.0, 2.0, 2.0, std::f32::consts::FRAC_PI_4)).unwrap();
        assert_eq!(s.pixel(130, 80), [0, 0, 0, 255]);
        assert!(s.pixel(130, 80 - 25)[3] > 200, "scaled ×2 and turned 45°: the corner reaches further");
        assert!(Affine::about((0.0, 0.0), 0.0, 0.0, 0.0, 1.0, 0.0).inverse().is_none());
    }

    #[test]
    fn transform_undo_via_capture() {
        let mut s = dot_surface();
        let before = s.clone();
        s.begin_capture();
        let m = polygon_mask(200, 200, &[(0.0, 0.0), (64.0, 0.0), (64.0, 64.0), (0.0, 64.0)]).unwrap();
        transform(&mut s, &m, Affine::about((32.0, 32.0), 0.0, 0.0, -1.0, 1.0, 0.0)).unwrap();
        let snap = s.take_capture();
        s.swap_snapshot(snap);
        s.prune();
        assert_eq!(s, before);
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(48))]
        #[test]
        fn hostile_transforms_never_panic(pts in proptest::collection::vec((-500f32..700.0, -500f32..700.0), 0..12), a in -10f32..10.0, sx in -5f32..5.0, sy in -5f32..5.0, dx in -1e4f32..1e4) {
            let mut s = dot_surface();
            if let Some(m) = polygon_mask(200, 200, &pts) {
                let _ = transform(&mut s, &m, Affine::about((50.0, 50.0), dx, 0.0, sx, sy, a));
            }
        }
    }
}
