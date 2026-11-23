//! A flat premultiplied RGBA8 image (composites, exports, fill references).

use crate::pixel::{BlendMode, blend, over, unpremultiply};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Premultiplied RGBA8, row-major, `width·height·4` bytes.
    pub data: Vec<u8>,
}

impl RgbaImage {
    pub fn transparent(width: u32, height: u32) -> Self {
        Self { width, height, data: vec![0; width as usize * height as usize * 4] }
    }

    pub fn filled(width: u32, height: u32, premultiplied: [u8; 4]) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..width as usize * height as usize {
            data.extend_from_slice(&premultiplied);
        }
        Self { width, height, data }
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        match self.data.get(i..i + 4) {
            Some(p) if x < self.width => [p[0], p[1], p[2], p[3]],
            _ => [0; 4],
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, p: [u8; 4]) {
        if x >= self.width {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        if let Some(d) = self.data.get_mut(i..i + 4) {
            d.copy_from_slice(&p);
        }
    }

    /// Composite `top` (same size) over this image.
    pub fn over_image(&mut self, top: &RgbaImage) {
        if top.width != self.width || top.height != self.height {
            return;
        }
        for (d, s) in self.data.as_chunks_mut::<4>().0.iter_mut().zip(top.data.as_chunks::<4>().0.iter()) {
            if s[3] != 0 {
                let o = over([d[0], d[1], d[2], d[3]], [s[0], s[1], s[2], s[3]]);
                d.copy_from_slice(&o);
            }
        }
    }

    /// Composite `top` (same size) over this image with a blend mode.
    pub fn blend_image(&mut self, top: &RgbaImage, mode: BlendMode) {
        if top.width != self.width || top.height != self.height {
            return;
        }
        for (d, s) in self.data.as_chunks_mut::<4>().0.iter_mut().zip(top.data.as_chunks::<4>().0.iter()) {
            if s[3] != 0 {
                *d = blend(*d, *s, mode);
            }
        }
    }

    /// Composite `top` (same size) over this image at `opacity` with a blend mode.
    pub fn composite(&mut self, top: &RgbaImage, mode: BlendMode, opacity: f32) {
        if top.width != self.width || top.height != self.height {
            return;
        }
        let o = (if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 } * 255.0).round() as u8;
        if o == 0 {
            return;
        }
        for (d, s) in self.data.as_chunks_mut::<4>().0.iter_mut().zip(top.data.as_chunks::<4>().0.iter()) {
            if s[3] != 0 {
                let s = if o == 255 { *s } else { crate::pixel::scale_alpha(*s, o) };
                *d = blend(*d, s, mode);
            }
        }
    }

    /// Composite `top` with its top-left corner at (`x`, `y`) of this image (clipped).
    pub fn composite_at(&mut self, top: &RgbaImage, x: u32, y: u32, mode: BlendMode, opacity: f32) {
        let o = (if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 } * 255.0).round() as u8;
        if o == 0 || x >= self.width || y >= self.height {
            return;
        }
        let w = top.width.min(self.width - x) as usize;
        let (sw, tw) = (self.width as usize * 4, top.width as usize * 4);
        for row in 0..top.height.min(self.height - y) as usize {
            let d0 = (y as usize + row) * sw + x as usize * 4;
            let (Some(dst), Some(src)) = (self.data.get_mut(d0..d0 + w * 4), top.data.get(row * tw..row * tw + w * 4)) else { continue };
            for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0.iter()) {
                if s[3] != 0 {
                    let s = if o == 255 { *s } else { crate::pixel::scale_alpha(*s, o) };
                    *d = blend(*d, s, mode);
                }
            }
        }
    }

    /// The smallest rectangle `(x0, y0, x1, y1)` (end exclusive) holding every pixel that is not
    /// fully transparent, or `None` for a transparent image.
    pub fn opaque_bounds(&self) -> Option<(u32, u32, u32, u32)> {
        let w = self.width as usize;
        let mut b: Option<(usize, usize, usize, usize)> = None;
        for (y, row) in self.data.chunks_exact(w.max(1) * 4).enumerate() {
            let px = row.as_chunks::<4>().0;
            let Some(first) = px.iter().position(|p| p[3] != 0) else { continue };
            let last = px.iter().rposition(|p| p[3] != 0).unwrap_or(first);
            b = Some(match b {
                None => (first, y, last + 1, y + 1),
                Some((x0, y0, x1, _)) => (x0.min(first), y0, x1.max(last + 1), y + 1),
            });
        }
        b.map(|(x0, y0, x1, y1)| (x0 as u32, y0 as u32, x1 as u32, y1 as u32))
    }

    /// The `w`×`h` rectangle at (`x`, `y`), clipped to the image.
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
        let x = x.min(self.width);
        let y = y.min(self.height);
        let w = w.min(self.width - x);
        let h = h.min(self.height - y);
        let mut out = RgbaImage::transparent(w, h);
        let (sw, ow) = (self.width as usize * 4, w as usize * 4);
        for row in 0..h as usize {
            let s = (y as usize + row) * sw + x as usize * 4;
            if let (Some(src), Some(dst)) = (self.data.get(s..s + ow), out.data.get_mut(row * ow..(row + 1) * ow)) {
                dst.copy_from_slice(src);
            }
        }
        out
    }

    /// Composite `top` (same size) onto this image only where it already has paint, keeping
    /// this image's coverage ("atop": clipping layers on their base).
    pub fn atop_image(&mut self, top: &RgbaImage, mode: BlendMode) {
        if top.width != self.width || top.height != self.height {
            return;
        }
        for (d, s) in self.data.as_chunks_mut::<4>().0.iter_mut().zip(top.data.as_chunks::<4>().0.iter()) {
            if s[3] == 0 || d[3] == 0 {
                continue;
            }
            let src = crate::scale_alpha(*s, d[3]);
            let mut b = blend(*d, src, mode);
            b[3] = d[3];
            for k in 0..3 {
                b[k] = b[k].min(b[3]);
            }
            *d = b;
        }
    }

    /// A copy with every pixel's coverage scaled by `opacity` (0…1).
    pub fn scaled(&self, opacity: f32) -> RgbaImage {
        let o = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let mut out = self.clone();
        if o < 255 {
            for d in out.data.as_chunks_mut::<4>().0.iter_mut() {
                *d = crate::scale_alpha(*d, o);
            }
        }
        out
    }

    /// Straight (non-premultiplied) RGBA bytes, for encoders.
    pub fn to_straight(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len());
        for p in self.data.as_chunks::<4>().0.iter() {
            out.extend_from_slice(&unpremultiply([p[0], p[1], p[2], p[3]]));
        }
        out
    }

    /// Box-filtered downscale so the longer side is at most `max_side` (thumbnails).
    pub fn downscaled(&self, max_side: u32) -> RgbaImage {
        let long = self.width.max(self.height).max(1);
        if long <= max_side || max_side == 0 {
            return self.clone();
        }
        self.reduced(long.div_ceil(max_side))
    }

    /// The image `f` times smaller on each side (rounded up), each pixel the average of the block
    /// it covers. `f` of 0 or 1 is a copy.
    pub fn reduced(&self, f: u32) -> RgbaImage {
        if f <= 1 {
            return self.clone();
        }
        let (w, h) = (self.width.div_ceil(f).max(1), self.height.div_ceil(f).max(1));
        let mut out = RgbaImage::transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 4];
                let mut n = 0u32;
                for sy in y * f..((y + 1) * f).min(self.height) {
                    for sx in x * f..((x + 1) * f).min(self.width) {
                        let p = self.pixel(sx, sy);
                        for c in 0..4 {
                            acc[c] += p[c] as u32;
                        }
                        n += 1;
                    }
                }
                let n = n.max(1);
                out.set_pixel(x, y, [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8, (acc[3] / n) as u8]);
            }
        }
        out
    }
}
