//! Brush tips from images: a grey mask stamped instead of the round dab (dark is ink).

/// Largest tip side in pixels.
pub const MAX_TIP_SIDE: u32 = 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Coverage 0 … 255, row-major.
    pub alpha: Vec<u8>,
}

impl Tip {
    pub fn new(name: &str, width: u32, height: u32, alpha: Vec<u8>) -> Option<Tip> {
        if width == 0 || height == 0 || width > MAX_TIP_SIDE || height > MAX_TIP_SIDE || alpha.len() != width as usize * height as usize {
            return None;
        }
        Some(Tip { name: name.chars().take(200).collect(), width, height, alpha })
    }

    /// From straight RGBA: coverage is opacity times darkness, so black-on-white and black-on-
    /// transparent images both work.
    pub fn from_rgba(name: &str, width: u32, height: u32, rgba: &[u8]) -> Option<Tip> {
        if rgba.len() != width as usize * height as usize * 4 {
            return None;
        }
        let alpha = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let lum = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
                (p[3] as f32 * (1.0 - lum)).round().clamp(0.0, 255.0) as u8
            })
            .collect();
        Tip::new(name, width, height, alpha)
    }

    /// A grain image from straight RGBA: the height is the luminance of the pixel laid over
    /// mid-grey, so light is a peak and dark a valley, and a transparent pixel is neutral.
    /// (Stored the same way as a tip, in the shot's tip table; the brush says which role it has.)
    pub fn grain_from_rgba(name: &str, width: u32, height: u32, rgba: &[u8]) -> Option<Tip> {
        if rgba.len() != width as usize * height as usize * 4 {
            return None;
        }
        let alpha = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                let lum = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
                let a = p[3] as f32 / 255.0;
                ((lum * a + 0.5 * (1.0 - a)) * 255.0).round().clamp(0.0, 255.0) as u8
            })
            .collect();
        Tip::new(name, width, height, alpha)
    }

    /// The image as a repeating grain: its value (0 … 1) at canvas position `(x, y)` when one
    /// image pixel covers `scale` canvas pixels; bilinear, wrapping at the edges.
    pub fn tile_height(&self, x: f32, y: f32, scale: f32) -> f32 {
        let (w, h) = (self.width as usize, self.height as usize);
        if w == 0 || h == 0 || self.alpha.len() < w * h {
            return 0.5;
        }
        let s = scale.max(0.25);
        let fx = x / s - 0.5;
        let fy = y / s - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let wrap = |v: f32, n: usize| (v as i64).rem_euclid(n as i64) as usize;
        let (ix, iy) = (wrap(x0, w), wrap(y0, h));
        let (jx, jy) = ((ix + 1) % w, (iy + 1) % h);
        let get = |x: usize, y: usize| self.alpha.get(y * w + x).copied().unwrap_or(128) as f32 / 255.0;
        let top = get(ix, iy) + (get(jx, iy) - get(ix, iy)) * tx;
        let bottom = get(ix, jy) + (get(jx, jy) - get(ix, jy)) * tx;
        top + (bottom - top) * ty
    }

    /// Coverage at `(u, v)` in -1 … 1 across the tip (bilinear), 0 outside.
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        if !(-1.0..=1.0).contains(&u) || !(-1.0..=1.0).contains(&v) {
            return 0.0;
        }
        let x = (u + 1.0) * 0.5 * self.width as f32 - 0.5;
        let y = (v + 1.0) * 0.5 * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (tx, ty) = (x - x0, y - y0);
        let get = |x: f32, y: f32| -> f32 {
            if x < 0.0 || y < 0.0 || x >= self.width as f32 || y >= self.height as f32 {
                return 0.0;
            }
            self.alpha.get(y as usize * self.width as usize + x as usize).copied().unwrap_or(0) as f32 / 255.0
        };
        let top = get(x0, y0) + (get(x0 + 1.0, y0) - get(x0, y0)) * tx;
        let bottom = get(x0, y0 + 1.0) + (get(x0 + 1.0, y0 + 1.0) - get(x0, y0 + 1.0)) * tx;
        top + (bottom - top) * ty
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tips() {
        let rgba: Vec<u8> = [[0u8, 0, 0, 255], [255, 255, 255, 255], [0, 0, 0, 0], [0, 0, 0, 255]].concat();
        let t = Tip::from_rgba("t", 2, 2, &rgba).unwrap();
        assert_eq!(t.alpha, vec![255, 0, 0, 255]);
        assert!(t.sample(-0.5, -0.5) > 0.99 && t.sample(0.5, -0.5) < 0.01);
        assert_eq!(t.sample(2.0, 0.0), 0.0);
        assert!(Tip::new("x", 0, 1, vec![]).is_none());
        assert!(Tip::new("x", 2, 2, vec![0; 3]).is_none());
    }

    #[test]
    fn grain_images_tile_and_read_light_as_peaks() {
        let rgba: Vec<u8> = [[255u8, 255, 255, 255], [0, 0, 0, 255], [0, 0, 0, 255], [255, 255, 255, 255]].concat();
        let g = Tip::grain_from_rgba("checks", 2, 2, &rgba).unwrap_or_else(|| Tip { name: String::new(), width: 1, height: 1, alpha: vec![0] });
        assert_eq!(g.alpha, vec![255, 0, 0, 255]);
        // pixel centres at scale 1 are at .5; one image period is 2 canvas pixels
        assert!(g.tile_height(0.5, 0.5, 1.0) > 0.99 && g.tile_height(1.5, 0.5, 1.0) < 0.01);
        assert!((g.tile_height(0.5, 0.5, 1.0) - g.tile_height(4.5, 6.5, 1.0)).abs() < 1e-5, "wraps");
        assert!((g.tile_height(-1.5, 0.5, 1.0) - g.tile_height(0.5, 0.5, 1.0)).abs() < 1e-5, "wraps backwards");
        let clear = Tip::grain_from_rgba("c", 1, 1, &[0, 0, 0, 0]).map(|t| t.alpha).unwrap_or_default();
        assert_eq!(clear, vec![128]);
        assert!(Tip::grain_from_rgba("bad", 2, 2, &[0; 3]).is_none());
    }
}
