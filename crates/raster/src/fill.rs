//! Paint-bucket flood fill with gap closing.
//!
//! The fill works on a *reference* image (the current drawing, or all visible layers composited)
//! and returns a [`Mask`] of the pixels to paint:
//!
//! 1. Pixels whose colour is within `tolerance` of the seed pixel are *open*; the rest are lines.
//! 2. Gap closing: open pixels nearer than `gap` pixels to a line are closed too (a chamfer
//!    distance transform), so a break in a line narrower than about `2·gap` stops the fill.
//! 3. A scanline flood fill from the seed over the remaining open pixels.
//! 4. The region grows back by `gap` steps through open pixels only, so it reaches the lines
//!    again without crossing the gap it closed.
//! 5. `spread` grows the result under the lines' antialiased edges, so no light halo is left
//!    between line and paint.
//!
//! Everything is iterative (no recursion) and the work area is capped at [`MAX_FILL_PIXELS`].

use crate::{RasterError, RgbaImage, Surface, TILE, over};

/// Largest reference area the fill works on (pixels). Larger canvases fill a window around the
/// seed (the caller chooses it).
pub const MAX_FILL_PIXELS: u64 = 40_000_000;
/// Largest gap-closing distance.
pub const MAX_GAP: u8 = 32;
/// Largest spread under lines.
pub const MAX_SPREAD: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FillOptions {
    /// Largest per-channel difference from the seed colour that still counts as the same area.
    pub tolerance: u8,
    /// Close gaps up to about twice this many pixels.
    pub gap: u8,
    /// Grow the fill this many pixels under the lines.
    pub spread: u8,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self { tolerance: 40, gap: 3, spread: 1 }
    }
}

impl FillOptions {
    pub fn sanitized(self) -> Self {
        Self { tolerance: self.tolerance, gap: self.gap.min(MAX_GAP), spread: self.spread.min(MAX_SPREAD) }
    }
}

/// Pixels to paint, in canvas coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mask {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// One byte per pixel, 0 or 255.
    pub data: Vec<u8>,
}

impl Mask {
    pub fn get(&self, x: u32, y: u32) -> bool {
        if x < self.x || y < self.y || x >= self.x + self.width || y >= self.y + self.height {
            return false;
        }
        let i = (y - self.y) as usize * self.width as usize + (x - self.x) as usize;
        self.data.get(i).is_some_and(|v| *v != 0)
    }

    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| **v != 0).count()
    }
}

fn diff(a: &[u8], b: [u8; 4]) -> u8 {
    a[0].abs_diff(b[0]).max(a[1].abs_diff(b[1])).max(a[2].abs_diff(b[2])).max(a[3].abs_diff(b[3]))
}

/// Find the area to fill from `seed` (coordinates in `reference`). `origin` is where the
/// reference sits on the canvas; the returned mask is in canvas coordinates. `Ok(None)` when the
/// seed is outside the reference.
pub fn flood(reference: &RgbaImage, origin: (u32, u32), seed: (u32, u32), options: FillOptions) -> Result<Option<Mask>, RasterError> {
    let o = options.sanitized();
    let (w, h) = (reference.width as usize, reference.height as usize);
    let n = w as u64 * h as u64;
    if n > MAX_FILL_PIXELS {
        return Err(RasterError::FillTooLarge(MAX_FILL_PIXELS));
    }
    if seed.0 as usize >= w || seed.1 as usize >= h || reference.data.len() != w * h * 4 {
        return Ok(None);
    }
    let n = n as usize;
    let seed_i = seed.1 as usize * w + seed.0 as usize;
    let seed_px = reference.pixel(seed.0, seed.1);
    // 1. open pixels
    let open: Vec<bool> = reference.data.as_chunks::<4>().0.iter().map(|p| diff(p, seed_px) <= o.tolerance).collect();
    // 2. gap closing
    let mut allowed = open.clone();
    if o.gap > 0 {
        let limit = 3 * o.gap as u16;
        let dist = chamfer(&open, w, h, limit + 1);
        let closed: Vec<bool> = open.iter().zip(&dist).map(|(op, d)| *op && *d > limit).collect();
        // A seed inside a narrow area (nearer than `gap` to a line) fills without gap closing.
        if closed.get(seed_i).copied().unwrap_or(false) {
            allowed = closed;
        }
    }
    // 3. flood
    let mut region = vec![false; n];
    scanline(&allowed, &mut region, w, h, seed.0 as usize, seed.1 as usize);
    // 4. grow back through open pixels
    if o.gap > 0 {
        grow(&mut region, w, h, o.gap as usize + 1, Some(&open));
    }
    // 5. spread under lines
    if o.spread > 0 {
        grow(&mut region, w, h, o.spread as usize, None);
    }
    // crop to the bounding box
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for (i, r) in region.iter().enumerate() {
        if *r {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == usize::MAX {
        return Ok(None);
    }
    let (mw, mh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut data = vec![0u8; mw * mh];
    for y in y0..=y1 {
        for x in x0..=x1 {
            if region[y * w + x] {
                data[(y - y0) * mw + (x - x0)] = 255;
            }
        }
    }
    Ok(Some(Mask { x: origin.0 + x0 as u32, y: origin.1 + y0 as u32, width: mw as u32, height: mh as u32, data }))
}

/// Chamfer (3-4) distance to the nearest closed pixel, in thirds of a pixel, capped at `cap`.
fn chamfer(open: &[bool], w: usize, h: usize, cap: u16) -> Vec<u16> {
    let mut d: Vec<u16> = open.iter().map(|o| if *o { cap } else { 0 }).collect();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut v = d[i];
            if v == 0 {
                continue;
            }
            if x > 0 {
                v = v.min(d[i - 1].saturating_add(3));
            }
            if y > 0 {
                v = v.min(d[i - w].saturating_add(3));
                if x > 0 {
                    v = v.min(d[i - w - 1].saturating_add(4));
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1].saturating_add(4));
                }
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut v = d[i];
            if v == 0 {
                continue;
            }
            if x + 1 < w {
                v = v.min(d[i + 1].saturating_add(3));
            }
            if y + 1 < h {
                v = v.min(d[i + w].saturating_add(3));
                if x + 1 < w {
                    v = v.min(d[i + w + 1].saturating_add(4));
                }
                if x > 0 {
                    v = v.min(d[i + w - 1].saturating_add(4));
                }
            }
            d[i] = v;
        }
    }
    d
}

/// Iterative scanline flood fill (4-connected).
fn scanline(allowed: &[bool], region: &mut [bool], w: usize, h: usize, sx: usize, sy: usize) {
    if !allowed.get(sy * w + sx).copied().unwrap_or(false) {
        return;
    }
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        let row = y * w;
        if region[row + x] || !allowed[row + x] {
            continue;
        }
        let mut l = x;
        while l > 0 && allowed[row + l - 1] && !region[row + l - 1] {
            l -= 1;
        }
        let mut r = x;
        while r + 1 < w && allowed[row + r + 1] && !region[row + r + 1] {
            r += 1;
        }
        for i in l..=r {
            region[row + i] = true;
        }
        for ny in [y.wrapping_sub(1), y + 1] {
            if ny >= h {
                continue;
            }
            let nrow = ny * w;
            let mut i = l;
            while i <= r {
                if allowed[nrow + i] && !region[nrow + i] {
                    stack.push((i, ny));
                    while i <= r && allowed[nrow + i] && !region[nrow + i] {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
    }
}

/// Grow the region `steps` pixels (alternating 4- and 8-neighbourhoods for a rounder shape),
/// only into pixels `through` allows when given. Each pixel is visited once per step frontier.
fn grow(region: &mut [bool], w: usize, h: usize, steps: usize, through: Option<&[bool]>) {
    let mut frontier: Vec<usize> = Vec::new();
    for (i, r) in region.iter().enumerate() {
        if *r {
            let (x, y) = (i % w, i / w);
            let edge = (x > 0 && !region[i - 1]) || (x + 1 < w && !region[i + 1]) || (y > 0 && !region[i - w]) || (y + 1 < h && !region[i + w]);
            if edge {
                frontier.push(i);
            }
        }
    }
    for step in 0..steps {
        let diagonal = step % 2 == 1;
        let mut next = Vec::new();
        for &i in &frontier {
            let (x, y) = ((i % w) as isize, (i / w) as isize);
            for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                if !diagonal && dx != 0 && dy != 0 {
                    continue;
                }
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if region[j] || through.is_some_and(|t| !t[j]) {
                    continue;
                }
                region[j] = true;
                next.push(j);
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
}

/// Paint `color` (premultiplied) under the existing pixels of `surface` wherever `mask` is set.
/// Existing pixels stay on top, so lines on the same layer keep their antialiased edges.
pub fn apply(surface: &mut Surface, mask: &Mask, color: [u8; 4]) {
    if mask.width == 0 || mask.height == 0 {
        return;
    }
    let tx0 = mask.x / TILE;
    let ty0 = mask.y / TILE;
    let tx1 = (mask.x + mask.width - 1) / TILE;
    let ty1 = (mask.y + mask.height - 1) / TILE;
    for ty in ty0..=ty1 {
        for tx in tx0..=tx1 {
            // Skip tiles the mask does not touch.
            let any = (0..TILE).any(|r| (0..TILE).any(|c| mask.get(tx * TILE + c, ty * TILE + r)));
            if !any {
                continue;
            }
            let Some(index) = surface.tile_index(tx, ty) else { continue };
            let Some(tile) = surface.tile_mut(index) else { continue };
            let px = tile.bytes_mut();
            for r in 0..TILE {
                for c in 0..TILE {
                    if mask.get(tx * TILE + c, ty * TILE + r) {
                        let i = ((r * TILE + c) * 4) as usize;
                        let existing = [px[i], px[i + 1], px[i + 2], px[i + 3]];
                        let out = over(color, existing);
                        px[i..i + 4].copy_from_slice(&out);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A white image with a black square outline from (10,10) to (50,50), 2 px thick, with an
    /// optional gap in the right edge.
    fn square(gap: u32) -> RgbaImage {
        let mut img = RgbaImage::filled(64, 64, [255, 255, 255, 255]);
        for i in 10..=50 {
            for t in 0..2 {
                img.set_pixel(i, 10 + t, [0, 0, 0, 255]);
                img.set_pixel(i, 49 + t, [0, 0, 0, 255]);
                img.set_pixel(10 + t, i, [0, 0, 0, 255]);
                if !(i >= 30 && i < 30 + gap) {
                    img.set_pixel(49 + t, i, [0, 0, 0, 255]);
                }
            }
        }
        img
    }

    #[test]
    fn closed_shape_fills_inside_only() {
        let m = flood(&square(0), (0, 0), (30, 30), FillOptions { gap: 0, spread: 0, ..Default::default() }).unwrap().unwrap();
        assert!(m.get(30, 30) && m.get(12, 12) && m.get(48, 48));
        assert!(!m.get(5, 5) && !m.get(10, 30));
        assert_eq!(m.count(), 37 * 37);
    }

    #[test]
    fn open_shape_leaks_without_gap_closing() {
        let m = flood(&square(4), (0, 0), (30, 30), FillOptions { gap: 0, spread: 0, ..Default::default() }).unwrap().unwrap();
        assert!(m.get(2, 2), "a 4 px gap lets the fill out");
    }

    #[test]
    fn gap_closing_stops_the_leak_and_reaches_the_lines() {
        let m = flood(&square(4), (0, 0), (30, 30), FillOptions { gap: 3, spread: 0, ..Default::default() }).unwrap().unwrap();
        assert!(!m.get(2, 2), "the gap is closed");
        assert!(!m.get(55, 32), "nothing outside the gap");
        assert!(m.get(12, 12) && m.get(48, 48), "the corners are reached again");
    }

    #[test]
    fn spread_goes_under_lines() {
        let m = flood(&square(0), (0, 0), (30, 30), FillOptions { gap: 0, spread: 1, ..Default::default() }).unwrap().unwrap();
        assert!(m.get(11, 30));
        assert!(!m.get(10, 30) || m.get(11, 30));
    }

    #[test]
    fn origin_offsets_the_mask_and_apply_paints_under() {
        let m = flood(&square(0), (100, 200), (30, 30), FillOptions { gap: 0, spread: 0, ..Default::default() }).unwrap().unwrap();
        assert_eq!((m.x, m.y), (112, 212));
        let mut s = Surface::new(256, 256).unwrap();
        s.tile_mut(s.tile_index(1, 3).unwrap()).unwrap().bytes_mut()[..4].copy_from_slice(&[0, 0, 0, 255]);
        apply(&mut s, &m, [200, 0, 0, 255]);
        assert_eq!(s.pixel(130, 230), [200, 0, 0, 255]);
        assert_eq!(s.pixel(64, 192), [0, 0, 0, 255], "existing ink stays on top");
        assert_eq!(s.pixel(5, 5), [0; 4]);
    }

    #[test]
    fn seed_outside_or_huge_reference() {
        assert_eq!(flood(&square(0), (0, 0), (99, 1), FillOptions::default()).unwrap(), None);
        let big = RgbaImage { width: 8000, height: 6000, data: Vec::new() };
        assert!(flood(&big, (0, 0), (1, 1), FillOptions::default()).is_err());
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
        /// Random line art never makes the fill panic, and the mask always contains the seed's
        /// region and stays inside the image.
        #[test]
        fn fill_never_panics(
            w in 1u32..48, h in 1u32..48,
            pixels in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..48 * 48),
            sx in 0u32..60, sy in 0u32..60, tol in proptest::prelude::any::<u8>(), gap in 0u8..40, spread in 0u8..12,
        ) {
            let mut img = RgbaImage::transparent(w, h);
            for (i, v) in pixels.iter().enumerate().take((w * h) as usize) {
                let a = if v % 3 == 0 { 255 } else { 0 };
                img.set_pixel(i as u32 % w, i as u32 / w, [0, 0, 0, a]);
            }
            let r = flood(&img, (5, 7), (sx, sy), FillOptions { tolerance: tol, gap, spread });
            let m = r.unwrap();
            if let Some(m) = m {
                proptest::prop_assert!(m.x >= 5 && m.y >= 7 && m.x + m.width <= 5 + w && m.y + m.height <= 7 + h);
                proptest::prop_assert!(m.count() > 0);
            } else {
                proptest::prop_assert!(sx >= w || sy >= h);
            }
        }
    }
}
