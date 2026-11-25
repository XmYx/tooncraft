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
    /// Like [`Affine::about`], with a skew (shear) along x and y (radians) applied first.
    pub fn about_skewed(pivot: (f32, f32), dx: f32, dy: f32, sx: f32, sy: f32, angle: f32, skew: (f32, f32)) -> Affine {
        let (kx, ky) = (skew.0.clamp(-1.4, 1.4).tan(), skew.1.clamp(-1.4, 1.4).tan());
        let (px, py) = pivot;
        let shear = Affine { a: 1.0, b: kx, c: ky, d: 1.0, tx: -(px + kx * py) + px, ty: -(ky * px + py) + py };
        shear.then(&Affine::about(pivot, dx, dy, sx, sy, angle))
    }

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

    /// Apply `self`, then `next`.
    pub fn then(&self, next: &Affine) -> Affine {
        Affine {
            a: next.a * self.a + next.b * self.c,
            b: next.a * self.b + next.b * self.d,
            c: next.c * self.a + next.d * self.c,
            d: next.c * self.b + next.d * self.d,
            tx: next.a * self.tx + next.b * self.ty + next.tx,
            ty: next.c * self.tx + next.d * self.ty + next.ty,
        }
    }

    /// A move by `(dx, dy)`.
    pub fn translate(dx: f32, dy: f32) -> Affine {
        Affine { tx: dx, ty: dy, ..Affine::IDENTITY }
    }

    /// Within `eps` of the identity (a pure copy, no resampling needed).
    pub fn is_identity(&self, eps: f32) -> bool {
        (self.a - 1.0).abs() <= eps && self.b.abs() <= eps && self.c.abs() <= eps && (self.d - 1.0).abs() <= eps && self.tx.abs() <= eps && self.ty.abs() <= eps
    }
}

/// Pixels whose centres lie inside the polygon (even-odd rule), clipped to `w`×`h`.
pub fn polygon_mask(w: u32, h: u32, poly: &[(f32, f32)]) -> Option<Mask> {
    rings_mask(w, h, &[poly])
}

/// Pixels whose centres lie inside the rings together (even-odd over all their edges, so a ring
/// inside another is a hole), clipped to `w`×`h`. Rings of fewer than 3 points are skipped.
pub fn rings_mask(w: u32, h: u32, rings: &[&[(f32, f32)]]) -> Option<Mask> {
    let rings: Vec<&[(f32, f32)]> = rings.iter().copied().filter(|r| r.len() >= 3).collect();
    if rings.is_empty() || rings.iter().any(|r| r.iter().any(|p| !p.0.is_finite() || !p.1.is_finite())) {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in rings.iter().flat_map(|r| r.iter()) {
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
        for poly in &rings {
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                if (a.1 <= cy) != (b.1 <= cy) {
                    xs.push(a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
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

/// How a new selection combines with the one there is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Combine {
    /// The new one replaces it.
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

/// The mask over the whole `w`×`h` canvas (one byte a pixel), or `None` past the size limit.
fn full(m: Option<&Mask>, w: u32, h: u32) -> Option<Vec<u8>> {
    if w as u64 * h as u64 > MAX_SELECTION_PIXELS {
        return None;
    }
    let mut out = vec![0u8; w as usize * h as usize];
    if let Some(m) = m {
        for y in 0..m.height {
            let cy = m.y + y;
            if cy >= h {
                break;
            }
            for x in 0..m.width {
                let cx = m.x + x;
                if cx < w && m.data[(y * m.width + x) as usize] != 0 {
                    out[(cy * w + cx) as usize] = 255;
                }
            }
        }
    }
    Some(out)
}

/// A whole-canvas buffer back to a mask cropped to what is set (`None` when nothing is).
fn crop(buf: &[u8], w: u32, h: u32) -> Option<Mask> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if buf[(y * w + x) as usize] != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 >= x1 {
        return None;
    }
    let (mw, mh) = (x1 - x0, y1 - y0);
    let mut data = vec![0u8; (mw * mh) as usize];
    for y in 0..mh {
        let src = ((y0 + y) * w + x0) as usize;
        data[(y * mw) as usize..((y + 1) * mw) as usize].copy_from_slice(&buf[src..src + mw as usize]);
    }
    Some(Mask { x: x0, y: y0, width: mw, height: mh, data })
}

/// Combine a new mask with the current one (`None`: nothing selected yet).
pub fn combine(current: Option<&Mask>, new: &Mask, op: Combine, w: u32, h: u32) -> Option<Mask> {
    let a = full(current, w, h)?;
    let b = full(Some(new), w, h)?;
    let out: Vec<u8> = a
        .iter()
        .zip(&b)
        .map(|(a, b)| {
            let (a, b) = (*a != 0, *b != 0);
            let on = match op {
                Combine::Replace => b,
                Combine::Add => a || b,
                Combine::Subtract => a && !b,
                Combine::Intersect => a && b,
            };
            if on { 255 } else { 0 }
        })
        .collect();
    crop(&out, w, h)
}

/// Everything on the `w`×`h` canvas that is not in `m`.
pub fn invert(m: Option<&Mask>, w: u32, h: u32) -> Option<Mask> {
    let mut buf = full(m, w, h)?;
    for v in &mut buf {
        *v = if *v == 0 { 255 } else { 0 };
    }
    crop(&buf, w, h)
}

/// Largest grow or shrink, in pixels.
pub const MAX_GROW: i32 = 64;

/// Grow (`n` > 0) or shrink (`n` < 0) the mask by about `n` pixels (a square reach, clamped to
/// [`MAX_GROW`]). Linear in the canvas size whatever `n` is.
pub fn grow(m: &Mask, n: i32, w: u32, h: u32) -> Option<Mask> {
    let n = n.clamp(-MAX_GROW, MAX_GROW);
    let mut buf = full(Some(m), w, h)?;
    if n == 0 {
        return crop(&buf, w, h);
    }
    let r = n.unsigned_abs() as usize;
    let grow_on = n > 0;
    // a pixel is on after growing when any pixel within r (per axis) is on; shrinking is the same
    // on the inverted mask. Each pass counts set pixels in a sliding window.
    let pass = |buf: &mut Vec<u8>, len: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        let mut line = vec![0u8; len];
        for l in 0..lines {
            let mut count = 0usize;
            let on = |v: u8| (v != 0) == grow_on;
            for i in 0..r.min(len) {
                count += on(buf[at(l, i)]) as usize;
            }
            for i in 0..len {
                if i + r < len {
                    count += on(buf[at(l, i + r)]) as usize;
                }
                if i > r {
                    count -= on(buf[at(l, i - r - 1)]) as usize;
                }
                let hit = count > 0;
                line[i] = if hit == grow_on { 255 } else { 0 };
            }
            for i in 0..len {
                buf[at(l, i)] = line[i];
            }
        }
    };
    let (wu, hu) = (w as usize, h as usize);
    pass(&mut buf, wu, hu, &|l, i| l * wu + i);
    pass(&mut buf, hu, wu, &|l, i| i * wu + l);
    crop(&buf, w, h)
}

/// Largest feather radius, in pixels.
pub const MAX_FEATHER: f32 = 64.0;

/// A soft-edged copy of the mask: the edge fades over about `radius` pixels on each side (three
/// box blurs, close to a Gaussian), so moving, erasing, copying or filling the selection blends
/// into what is around it. `radius` is clamped to [`MAX_FEATHER`]; below half a pixel the mask
/// comes back as it is. Work is linear in the mask's box (grown by the radius) whatever the
/// radius; `None` past [`MAX_SELECTION_PIXELS`] or when nothing is left.
pub fn feather(m: &Mask, radius: f32, w: u32, h: u32) -> Option<Mask> {
    let radius = if radius.is_finite() { radius.clamp(0.0, MAX_FEATHER) } else { 0.0 };
    if radius < 0.5 || m.width == 0 || m.height == 0 {
        return Some(m.clone());
    }
    // three boxes of radius b have a spread of about b·1.4; reach the full radius with them
    let b = ((radius / 1.5).round() as u32).max(1);
    let pad = 3 * b;
    let x0 = m.x.saturating_sub(pad);
    let y0 = m.y.saturating_sub(pad);
    let x1 = m.x.saturating_add(m.width).saturating_add(pad).min(w);
    let y1 = m.y.saturating_add(m.height).saturating_add(pad).min(h);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let (bw, bh) = ((x1 - x0) as usize, (y1 - y0) as usize);
    if bw as u64 * bh as u64 > MAX_SELECTION_PIXELS {
        return None;
    }
    // values ×255 kept as u32 so three passes do not lose the faint ends of the edge
    let mut buf = vec![0u32; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            buf[y * bw + x] = m.value(x0 + x as u32, y0 + y as u32) as u32 * 255;
        }
    }
    let r = b as usize;
    let mut line = Vec::new();
    let mut blur = |buf: &mut Vec<u32>, len: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        let div = (2 * r + 1) as u64;
        line.resize(len, 0u32);
        for l in 0..lines {
            // sliding sum over [i - r, i + r]; outside the box counts as unselected
            let mut sum: u64 = (0..r.min(len)).map(|i| buf[at(l, i)] as u64).sum();
            for i in 0..len {
                if i + r < len {
                    sum += buf[at(l, i + r)] as u64;
                }
                if i > r {
                    sum -= buf[at(l, i - r - 1)] as u64;
                }
                line[i] = (sum / div) as u32;
            }
            for (i, v) in line.iter().enumerate() {
                buf[at(l, i)] = *v;
            }
        }
    };
    for _ in 0..3 {
        blur(&mut buf, bw, bh, &|l, i| l * bw + i);
        blur(&mut buf, bh, bw, &|l, i| i * bw + l);
    }
    let data: Vec<u8> = buf.iter().map(|v| ((*v + 127) / 255).min(255) as u8).collect();
    // crop to what is still set
    let (mut cx0, mut cy0, mut cx1, mut cy1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..bh {
        for x in 0..bw {
            if data[y * bw + x] != 0 {
                cx0 = cx0.min(x);
                cy0 = cy0.min(y);
                cx1 = cx1.max(x + 1);
                cy1 = cy1.max(y + 1);
            }
        }
    }
    if cx0 >= cx1 {
        return None;
    }
    let (mw, mh) = (cx1 - cx0, cy1 - cy0);
    let mut out = Vec::with_capacity(mw * mh);
    for y in cy0..cy1 {
        out.extend_from_slice(&data[y * bw + cx0..y * bw + cx1]);
    }
    Some(Mask { x: x0 + cx0 as u32, y: y0 + cy0 as u32, width: mw as u32, height: mh as u32, data: out })
}

/// Most corner points the traced outline of a selection may have.
pub const MAX_TRACE_POINTS: usize = 200_000;

/// The outline of a mask as closed rings along pixel edges (outer edges and holes), for drawing
/// it and for the polygon commands (even-odd gives the mask back). `None` past
/// [`MAX_TRACE_POINTS`].
pub fn trace(m: &Mask) -> Option<Vec<Vec<(f32, f32)>>> {
    use std::collections::HashMap;
    let on = |x: i64, y: i64| x >= 0 && y >= 0 && (x as u32) < m.width && (y as u32) < m.height && m.data[(y as u32 * m.width + x as u32) as usize] != 0;
    // directed edges with the inside on the right: top edges go east, right edges south, …
    let mut next: HashMap<(i64, i64), Vec<(i64, i64)>> = HashMap::new();
    let mut edges = 0usize;
    for y in 0..m.height as i64 {
        for x in 0..m.width as i64 {
            if !on(x, y) {
                continue;
            }
            let mut add = |a: (i64, i64), b: (i64, i64)| {
                next.entry(a).or_default().push(b);
                edges += 1;
            };
            if !on(x, y - 1) {
                add((x, y), (x + 1, y));
            }
            if !on(x + 1, y) {
                add((x + 1, y), (x + 1, y + 1));
            }
            if !on(x, y + 1) {
                add((x + 1, y + 1), (x, y + 1));
            }
            if !on(x - 1, y) {
                add((x, y + 1), (x, y));
            }
            if edges > MAX_TRACE_POINTS {
                return None;
            }
        }
    }
    let mut rings = Vec::new();
    let mut starts: Vec<(i64, i64)> = next.keys().copied().collect();
    starts.sort();
    for s in starts {
        while next.get(&s).is_some_and(|v| !v.is_empty()) {
            let mut ring: Vec<(i64, i64)> = vec![s];
            let mut cur = s;
            let mut prev_dir = (0i64, 0i64);
            while let Some(outs) = next.get_mut(&cur).filter(|o| !o.is_empty()) {
                // at a corner where two outlines touch, turn right first (keeps rings simple)
                let pick = if outs.len() > 1 {
                    let right = (-prev_dir.1, prev_dir.0);
                    outs.iter().position(|o| (o.0 - cur.0, o.1 - cur.1) == right).unwrap_or(0)
                } else {
                    0
                };
                let to = outs.swap_remove(pick);
                prev_dir = (to.0 - cur.0, to.1 - cur.1);
                cur = to;
                if cur == s {
                    break;
                }
                ring.push(cur);
                if ring.len() > MAX_TRACE_POINTS {
                    return None;
                }
            }
            // keep only the corners (drop points along straight runs)
            let n = ring.len();
            let simple: Vec<(f32, f32)> = (0..n)
                .filter(|&i| {
                    let (a, b, c) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
                    (b.0 - a.0) * (c.1 - b.1) != (b.1 - a.1) * (c.0 - b.0)
                })
                .map(|i| ((m.x as i64 + ring[i].0) as f32, (m.y as i64 + ring[i].1) as f32))
                .collect();
            if simple.len() >= 3 {
                rings.push(simple);
            }
        }
    }
    Some(rings)
}

/// How pixels are resampled when a selection is moved, scaled, turned or distorted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resample {
    /// The nearest pixel: hard edges, for pixel art and flat colours.
    Nearest,
    #[default]
    Bilinear,
    /// Sharper when enlarging.
    Bicubic,
}

/// A projective map (`x' = (a x + b y + c) / (g x + h y + 1)`, …) for free distort, solved in
/// coordinates centred on each quad (so a horizon through the canvas origin is no problem).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography {
    h: [f32; 8],
    from: (f32, f32),
    to: (f32, f32),
}

impl Homography {
    /// The map from the four corners `src` to the four corners `dst` (in order), if they are not
    /// degenerate.
    pub fn from_quads(src: [(f32, f32); 4], dst: [(f32, f32); 4]) -> Option<Homography> {
        let centre = |q: [(f32, f32); 4]| (q.iter().map(|p| p.0).sum::<f32>() / 4.0, q.iter().map(|p| p.1).sum::<f32>() / 4.0);
        let (from, to) = (centre(src), centre(dst));
        // solve the 8×8 system by Gaussian elimination with partial pivoting
        let mut m = [[0f64; 9]; 8];
        for i in 0..4 {
            let (x, y) = ((src[i].0 - from.0) as f64, (src[i].1 - from.1) as f64);
            let (u, v) = ((dst[i].0 - to.0) as f64, (dst[i].1 - to.1) as f64);
            m[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
            m[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
        }
        for col in 0..8 {
            let piv = (col..8).max_by(|a, b| m[*a][col].abs().total_cmp(&m[*b][col].abs()))?;
            if m[piv][col].abs() < 1e-9 {
                return None;
            }
            m.swap(col, piv);
            for row in 0..8 {
                if row != col {
                    let f = m[row][col] / m[col][col];
                    for k in col..9 {
                        m[row][k] -= f * m[col][k];
                    }
                }
            }
        }
        let mut h = [0f32; 8];
        for i in 0..8 {
            h[i] = (m[i][8] / m[i][i]) as f32;
        }
        (h.iter().all(|v| v.is_finite()) && from.0.is_finite() && to.0.is_finite()).then_some(Homography { h, from, to })
    }

    pub fn apply(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let h = &self.h;
        let (x, y) = (x - self.from.0, y - self.from.1);
        let w = h[6] * x + h[7] * y + 1.0;
        if w.abs() < 1e-6 {
            return None;
        }
        let p = ((h[0] * x + h[1] * y + h[2]) / w + self.to.0, (h[3] * x + h[4] * y + h[5]) / w + self.to.1);
        (p.0.is_finite() && p.1.is_finite()).then_some(p)
    }
}

/// The pixels under the mask, as an image the size of the mask's box.
pub fn copy(surface: &Surface, mask: &Mask) -> RgbaImage {
    let mut img = RgbaImage::transparent(mask.width, mask.height);
    surface.composite_region(&mut img, mask.x, mask.y, 1.0, None);
    for (i, m) in mask.data.iter().enumerate() {
        if *m != 255 {
            let at = i * 4;
            if let Some(px) = img.data.get_mut(at..at + 4) {
                let w = crate::fill::weigh([px[0], px[1], px[2], px[3]], *m);
                px.copy_from_slice(&w);
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
                    let m = mask.value(tx * TILE + c, ty * TILE + r);
                    if m != 0 {
                        let i = ((r * TILE + c) * 4) as usize;
                        let kept = crate::fill::weigh([px[i], px[i + 1], px[i + 2], px[i + 3]], 255 - m);
                        px[i..i + 4].copy_from_slice(&kept);
                    }
                }
            }
        }
    }
    surface.prune();
}

/// Sample a premultiplied image at a position in pixels (centres at +0.5) with `r`;
/// transparent outside.
pub fn sample_with(img: &RgbaImage, x: f32, y: f32, r: Resample) -> [u8; 4] {
    match r {
        Resample::Bilinear => sample(img, x, y),
        Resample::Nearest => {
            let (fx, fy) = (x.floor(), y.floor());
            if fx < 0.0 || fy < 0.0 || fx >= img.width as f32 || fy >= img.height as f32 {
                return [0; 4];
            }
            img.pixel(fx as u32, fy as u32)
        }
        Resample::Bicubic => {
            let (fx, fy) = (x - 0.5, y - 0.5);
            let (x0, y0) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - x0, fy - y0);
            // Catmull-Rom weights
            let wt = |t: f32| -> [f32; 4] {
                let (t2, t3) = (t * t, t * t * t);
                [(-t3 + 2.0 * t2 - t) * 0.5, (3.0 * t3 - 5.0 * t2 + 2.0) * 0.5, (-3.0 * t3 + 4.0 * t2 + t) * 0.5, (t3 - t2) * 0.5]
            };
            let (wx, wy) = (wt(tx), wt(ty));
            let mut acc = [0f32; 4];
            for (j, wyj) in wy.iter().enumerate() {
                for (i, wxi) in wx.iter().enumerate() {
                    let (sx, sy) = (x0 + i as f32 - 1.0, y0 + j as f32 - 1.0);
                    if sx < 0.0 || sy < 0.0 || sx >= img.width as f32 || sy >= img.height as f32 {
                        continue;
                    }
                    let p = img.pixel(sx as u32, sy as u32);
                    for k in 0..4 {
                        acc[k] += p[k] as f32 * wxi * wyj;
                    }
                }
            }
            let a = acc[3].round().clamp(0.0, 255.0) as u8;
            let c = |v: f32| (v.round().clamp(0.0, 255.0) as u8).min(a);
            [c(acc[0]), c(acc[1]), c(acc[2]), a]
        }
    }
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
    draw_transformed_with(surface, img, origin, t, Resample::Bilinear)
}

/// [`draw_transformed`] with a choice of resampling.
pub fn draw_transformed_with(surface: &mut Surface, img: &RgbaImage, origin: (f32, f32), t: Affine, r: Resample) -> Result<(), RasterError> {
    let inv = t.inverse().ok_or(RasterError::BadImage { got: 0, want: 1 })?;
    let corners = [(0.0, 0.0), (img.width as f32, 0.0), (0.0, img.height as f32), (img.width as f32, img.height as f32)];
    let dst: Vec<(f32, f32)> = corners.iter().map(|(cx, cy)| t.apply(cx + origin.0, cy + origin.1)).collect();
    draw_mapped(surface, img, &dst, r, &|x, y| Some(inv.apply(x, y)), origin)
}

/// Draw `img` mapped so its corners land on `quad` (top-left, top-right, bottom-right,
/// bottom-left): a free distort or perspective.
pub fn draw_distorted(surface: &mut Surface, img: &RgbaImage, origin: (f32, f32), quad: [(f32, f32); 4], r: Resample) -> Result<(), RasterError> {
    if quad.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return Err(RasterError::BadImage { got: 0, want: 1 });
    }
    let (w, h) = (img.width as f32, img.height as f32);
    let src = [(origin.0, origin.1), (origin.0 + w, origin.1), (origin.0 + w, origin.1 + h), (origin.0, origin.1 + h)];
    let back = Homography::from_quads(quad, src).ok_or(RasterError::BadImage { got: 0, want: 1 })?;
    draw_mapped(surface, img, &quad, r, &|x, y| back.apply(x, y), origin)
}

/// Draw `img` over the surface where `inverse` maps each destination pixel centre back into the
/// source (canvas coordinates); `dst` bounds the area touched.
fn draw_mapped(
    surface: &mut Surface,
    img: &RgbaImage,
    dst: &[(f32, f32)],
    resample: Resample,
    inverse: &dyn Fn(f32, f32) -> Option<(f32, f32)>,
    origin: (f32, f32),
) -> Result<(), RasterError> {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(x, y) in dst {
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
                    let Some((sx, sy)) = inverse(x as f32 + 0.5, y as f32 + 0.5) else { continue };
                    let s = sample_with(img, sx - origin.0, sy - origin.1, resample);
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
    transform_with(surface, mask, t, Resample::Bilinear)
}

/// [`transform`] with a choice of resampling.
pub fn transform_with(surface: &mut Surface, mask: &Mask, t: Affine, r: Resample) -> Result<(), RasterError> {
    if !t.is_finite() {
        return Err(RasterError::BadImage { got: 0, want: 1 });
    }
    let img = copy(surface, mask);
    clear(surface, mask);
    draw_transformed_with(surface, &img, (mask.x as f32, mask.y as f32), t, r)
}

/// Distort the pixels under `mask` so the corners of its box land on `quad`.
pub fn distort(surface: &mut Surface, mask: &Mask, quad: [(f32, f32); 4], r: Resample) -> Result<(), RasterError> {
    let img = copy(surface, mask);
    let before = surface.clone();
    clear(surface, mask);
    let out = draw_distorted(surface, &img, (mask.x as f32, mask.y as f32), quad, r);
    if out.is_err() {
        *surface = before;
    }
    out
}

/// Most cells a mesh warp grid has along each side.
pub const MAX_MESH_CELLS: u32 = 16;

/// Where `p` sits in the quad `a b c d` (corners at (0,0), (1,0), (1,1), (0,1)) by inverting
/// the bilinear map; `None` outside it or when the quad is degenerate there.
pub fn inverse_bilinear(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> Option<(f32, f32)> {
    let cross = |u: (f32, f32), v: (f32, f32)| u.0 * v.1 - u.1 * v.0;
    let e = (b.0 - a.0, b.1 - a.1);
    let f = (d.0 - a.0, d.1 - a.1);
    let g = (a.0 - b.0 + c.0 - d.0, a.1 - b.1 + c.1 - d.1);
    let h = (p.0 - a.0, p.1 - a.1);
    let (k2, k1, k0) = (cross(g, f), cross(e, f) + cross(h, g), cross(h, e));
    let u_for = |v: f32| -> Option<f32> {
        let (dx, dy) = (e.0 + g.0 * v, e.1 + g.1 * v);
        if dx.abs() >= dy.abs() { (dx.abs() > 1e-8).then(|| (h.0 - f.0 * v) / dx) } else { Some((h.1 - f.1 * v) / dy) }
    };
    let inside = |t: f32| (-1e-4..=1.0 + 1e-4).contains(&t);
    let candidates: Vec<f32> = if k2.abs() < 1e-8 {
        if k1.abs() < 1e-8 {
            return None;
        }
        vec![-k0 / k1]
    } else {
        let disc = k1 * k1 - 4.0 * k0 * k2;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        vec![(-k1 - s) / (2.0 * k2), (-k1 + s) / (2.0 * k2)]
    };
    candidates.into_iter().filter(|v| v.is_finite() && inside(*v)).find_map(|v| u_for(v).filter(|u| u.is_finite() && inside(*u)).map(|u| (u, v)))
}

/// Warp the pixels under `mask` with a grid of `cols` × `rows` cells over its box: `points`
/// (row by row, (cols + 1) × (rows + 1) of them) are where the grid's points land. Each cell's
/// pixels follow its four corners bilinearly.
pub fn mesh_warp(surface: &mut Surface, mask: &Mask, cols: u32, rows: u32, points: &[(f32, f32)], r: Resample) -> Result<(), RasterError> {
    let bad = RasterError::BadImage { got: points.len(), want: ((cols + 1) * (rows + 1)) as usize };
    if cols == 0 || rows == 0 || cols > MAX_MESH_CELLS || rows > MAX_MESH_CELLS || points.len() != ((cols + 1) * (rows + 1)) as usize {
        return Err(bad);
    }
    if points.iter().any(|p| !p.0.is_finite() || !p.1.is_finite() || p.0.abs() > 1.0e6 || p.1.abs() > 1.0e6) {
        return Err(bad);
    }
    let img = copy(surface, mask);
    let before = surface.clone();
    clear(surface, mask);
    let origin = (mask.x as f32, mask.y as f32);
    let (w, h) = (img.width as f32, img.height as f32);
    let at = |i: u32, j: u32| points.get((j * (cols + 1) + i) as usize).copied().unwrap_or((0.0, 0.0));
    let mut out = Ok(());
    'cells: for j in 0..rows {
        for i in 0..cols {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
            // half-open cells, so a pixel on a shared edge is drawn once
            let (last_col, last_row) = (i + 1 == cols, j + 1 == rows);
            let inverse = |x: f32, y: f32| -> Option<(f32, f32)> {
                let (u, v) = inverse_bilinear((x, y), a, b, c, d)?;
                if (u >= 1.0 && !last_col) || (v >= 1.0 && !last_row) {
                    return None;
                }
                let (u, v) = (u.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
                Some((origin.0 + (i as f32 + u) / cols as f32 * w, origin.1 + (j as f32 + v) / rows as f32 * h))
            };
            if let Err(e) = draw_mapped(surface, &img, &[a, b, c, d], r, &inverse, origin) {
                out = Err(e);
                break 'cells;
            }
        }
    }
    if out.is_err() {
        *surface = before;
    }
    out
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
    fn a_flat_mesh_leaves_the_pixels_and_a_moved_point_bends_them() {
        let mut s = dot_surface();
        let m = polygon_mask(200, 200, &[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)]).unwrap();
        let grid = |dx: f32| -> Vec<(f32, f32)> {
            (0..3).flat_map(|j| (0..3).map(move |i| (i as f32 * 30.0 + if i == 1 && j == 1 { dx } else { 0.0 }, j as f32 * 30.0))).collect()
        };
        let before = s.clone();
        mesh_warp(&mut s, &m, 2, 2, &grid(0.0), Resample::Nearest).unwrap();
        assert_eq!(s.pixel(30, 30), before.pixel(30, 30), "an unmoved grid keeps the dot");
        assert_eq!(s.pixel(10, 10)[3], 0);
        // pull the middle point 20 px right: the dot (20…40) follows towards the right
        mesh_warp(&mut s, &m, 2, 2, &grid(20.0), Resample::Nearest).unwrap();
        assert!(s.pixel(45, 30)[3] > 0, "pulled right");
        assert_eq!(s.pixel(22, 50)[3], 0, "the bottom edge stayed: no paint low down");
        assert!(mesh_warp(&mut s, &m, 2, 2, &grid(0.0)[..5], Resample::Nearest).is_err(), "wrong point count");
        assert!(mesh_warp(&mut s, &m, 0, 2, &[], Resample::Nearest).is_err());
        let mut nan = grid(0.0);
        nan[4] = (f32::NAN, 1.0);
        assert!(mesh_warp(&mut s, &m, 2, 2, &nan, Resample::Nearest).is_err());
        assert_eq!(inverse_bilinear((5.0, 5.0), (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)), Some((0.5, 0.5)));
        assert_eq!(inverse_bilinear((50.0, 5.0), (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)), None);
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

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
        vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
    }

    #[test]
    fn rings_combine_invert_grow_and_trace_round_trip() {
        let (w, h) = (100, 80);
        // a ring inside a ring is a hole
        let outer = rect(10.0, 10.0, 50.0, 50.0);
        let hole = rect(20.0, 20.0, 30.0, 30.0);
        let m = rings_mask(w, h, &[&outer, &hole]).unwrap();
        assert_eq!(m.count(), 40 * 40 - 10 * 10);
        assert!(!m.get(25, 25) && m.get(15, 15));
        // add, subtract, intersect
        let b = polygon_mask(w, h, &rect(40.0, 10.0, 70.0, 20.0)).unwrap();
        let a = polygon_mask(w, h, &outer).unwrap();
        assert_eq!(combine(Some(&a), &b, Combine::Add, w, h).unwrap().count(), 1600 + 300 - 100);
        assert_eq!(combine(Some(&a), &b, Combine::Subtract, w, h).unwrap().count(), 1600 - 100);
        assert_eq!(combine(Some(&a), &b, Combine::Intersect, w, h).unwrap().count(), 100);
        assert!(combine(Some(&a), &polygon_mask(w, h, &rect(80.0, 60.0, 90.0, 70.0)).unwrap(), Combine::Intersect, w, h).is_none());
        assert_eq!(combine(None, &b, Combine::Add, w, h).unwrap().count(), 300);
        // invert
        assert_eq!(invert(Some(&a), w, h).unwrap().count(), (w * h) as usize - 1600);
        assert_eq!(invert(None, w, h).unwrap().count(), (w * h) as usize);
        // grow and shrink by a square reach
        assert_eq!(grow(&a, 2, w, h).unwrap().count(), 44 * 44);
        assert_eq!(grow(&a, -2, w, h).unwrap().count(), 36 * 36);
        assert!(grow(&a, -30, w, h).is_none(), "shrunk away");
        assert_eq!(grow(&a, 10_000, w, h).unwrap().count(), (w * h) as usize, "clamped to the canvas");
        // trace gives rings whose even-odd fill is the mask again
        let rings = trace(&m).unwrap();
        assert_eq!(rings.len(), 2, "the outline and the hole");
        let refs: Vec<&[(f32, f32)]> = rings.iter().map(|r| r.as_slice()).collect();
        assert_eq!(rings_mask(w, h, &refs).unwrap().data, m.data);
        // two blobs touching at a corner trace to rings that fill back the same
        let mut d = vec![0u8; 16];
        d[0] = 255;
        d[5] = 255;
        let diag = Mask { x: 3, y: 4, width: 4, height: 4, data: d };
        let rings = trace(&diag).unwrap();
        let refs: Vec<&[(f32, f32)]> = rings.iter().map(|r| r.as_slice()).collect();
        let back = rings_mask(w, h, &refs).unwrap();
        assert_eq!(back.count(), 2);
        assert!(back.get(3, 4) && back.get(4, 5));
    }

    #[test]
    fn resampling_and_distort() {
        let mut img = RgbaImage::transparent(2, 1);
        img.set_pixel(0, 0, [0, 0, 0, 255]);
        img.set_pixel(1, 0, [255, 255, 255, 255]);
        assert_eq!(sample_with(&img, 0.9, 0.5, Resample::Nearest), [0, 0, 0, 255]);
        assert!(sample_with(&img, 1.0, 0.5, Resample::Bilinear)[0] > 100);
        let _ = sample_with(&img, 1.0, 0.5, Resample::Bicubic);
        // a distort to the same corners leaves the picture where it was
        let mut s = dot_surface();
        let m = polygon_mask(200, 200, &rect(10.0, 10.0, 50.0, 50.0)).unwrap();
        distort(&mut s, &m, [(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)], Resample::Nearest).unwrap();
        assert_eq!(s.pixel(30, 30), [0, 0, 0, 255]);
        // a perspective: the top edge pulled in; the picture stays inside the new quad
        distort(&mut s, &m, [(25.0, 10.0), (35.0, 10.0), (60.0, 60.0), (0.0, 60.0)], Resample::Bilinear).unwrap();
        assert_eq!(s.pixel(5, 12)[3], 0);
        // a degenerate quad is refused and the drawing is unchanged
        let before = s.clone();
        assert!(distort(&mut s, &m, [(0.0, 0.0); 4], Resample::Bilinear).is_err());
        assert!(distort(&mut s, &m, [(f32::NAN, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], Resample::Bilinear).is_err());
        assert_eq!(s, before);
        // skew keeps the pivot
        let t = Affine::about_skewed((30.0, 30.0), 0.0, 0.0, 1.0, 1.0, 0.0, (0.5, 0.0));
        let p = t.apply(30.0, 30.0);
        assert!((p.0 - 30.0).abs() < 1e-3 && (p.1 - 30.0).abs() < 1e-3);
        assert!(t.apply(30.0, 40.0).0 > 30.0, "points below the pivot shift right");
    }

    #[test]
    fn feather_softens_the_edge_and_weights_erase_and_copy() {
        let (w, h) = (100, 100);
        let hard = polygon_mask(w, h, &rect(30.0, 30.0, 70.0, 70.0)).unwrap();
        assert!(feather(&hard, 0.2, w, h).unwrap() == hard, "under half a pixel: unchanged");
        let soft = feather(&hard, 6.0, w, h).unwrap();
        assert_eq!(soft.value(50, 50), 255, "the middle stays fully selected");
        assert_eq!(soft.value(5, 5), 0, "far outside stays out");
        let edge = soft.value(30, 50);
        assert!(edge > 60 && edge < 200, "the edge is half selected, got {edge}");
        assert!(soft.value(26, 50) > 0 && soft.value(26, 50) < edge, "it fades outside the outline");
        assert!(soft.value(34, 50) > edge && soft.value(34, 50) < 255, "and inside it");
        assert!(soft.x < hard.x && soft.width > hard.width, "the box grows with the fade");
        // hostile radii are clamped, and a radius past the cap costs no more than the cap
        assert!(feather(&hard, f32::NAN, w, h).unwrap() == hard);
        let capped = feather(&hard, 1e9, w, h).unwrap();
        assert_eq!(capped, feather(&hard, MAX_FEATHER, w, h).unwrap());
        // erase: fully inside clears, the edge keeps part of the paint, outside keeps it all
        let mut s = Surface::new(w, h).unwrap();
        let red = [255, 0, 0, 255];
        let all = Mask { x: 0, y: 0, width: w, height: h, data: vec![255; (w * h) as usize] };
        crate::fill::paint_over(&mut s, &all, red);
        let copied = copy(&s, &soft);
        let at = |img: &RgbaImage, x: u32, y: u32| img.pixel(x - soft.x, y - soft.y);
        assert_eq!(at(&copied, 50, 50), red);
        assert_eq!(at(&copied, 30, 50)[3], edge, "the copy carries the mask's softness");
        clear(&mut s, &soft);
        assert_eq!(s.pixel(50, 50)[3], 0);
        assert_eq!(s.pixel(30, 50)[3], 255 - edge);
        assert_eq!(s.pixel(5, 5), red);
        // a soft fill paints less at the edge
        let mut s = Surface::new(w, h).unwrap();
        crate::fill::paint_over(&mut s, &soft, red);
        assert_eq!(s.pixel(50, 50), red);
        assert_eq!(s.pixel(30, 50)[3], edge);
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(48))]
        #[test]
        fn hostile_feather_never_panics(pts in proptest::collection::vec((-500f32..700.0, -500f32..700.0), 0..12), r in -1e6f32..1e6) {
            if let Some(m) = polygon_mask(200, 150, &pts)
                && let Some(f) = feather(&m, r, 200, 150)
            {
                proptest::prop_assert!(f.x + f.width <= 200 && f.y + f.height <= 150);
                proptest::prop_assert_eq!(f.data.len(), (f.width * f.height) as usize);
                let mut s = dot_surface();
                let _ = transform(&mut s, &f, Affine::translate(3.0, -2.0));
            }
        }

        #[test]
        fn hostile_selection_ops_never_panic(pts in proptest::collection::vec((-500f32..700.0, -500f32..700.0), 0..12), n in -200i32..200, q in proptest::collection::vec((-1e5f32..1e5, -1e5f32..1e5), 4)) {
            if let Some(m) = polygon_mask(200, 150, &pts) {
                let _ = grow(&m, n, 200, 150);
                let _ = invert(Some(&m), 200, 150);
                if let Some(r) = trace(&m) {
                    let refs: Vec<&[(f32, f32)]> = r.iter().map(|r| r.as_slice()).collect();
                    proptest::prop_assert_eq!(rings_mask(200, 150, &refs).map(|b| b.count()), Some(m.count()));
                }
                let mut s = dot_surface();
                let _ = distort(&mut s, &m, [q[0], q[1], q[2], q[3]], Resample::Bicubic);
            }
        }
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
