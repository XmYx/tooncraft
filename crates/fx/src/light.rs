//! Lighting: tone or highlight a drawing where another layer (a matte) covers it, and light it
//! from normals, taken from a normal-map layer or worked out from the drawing's own shape.

use tooncraft_raster::{RgbaImage, premultiply, select, unpremultiply};

use crate::{blur, par};

/// Shade (multiply by `colour`) or, with `highlight`, light (screen `colour`) the drawing where
/// the matte (`map`, another layer's pixels, the same size and place as `img`) covers it, moved
/// by `offset` and softened by `softness` pixels; `amount` of the way.
pub(crate) fn matte_tone(img: &mut RgbaImage, map: &RgbaImage, colour: [u8; 3], amount: f32, offset: (f32, f32), softness: f32, highlight: bool) {
    if amount <= 0.0 || map.width != img.width || map.height != img.height {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let mut plane: Vec<u8> = map.data.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    blur::gaussian_plane(&mut plane, w, h, softness / 2.0);
    let m = RgbaImage { width: img.width, height: img.height, data: plane.iter().flat_map(|a| [0, 0, 0, *a]).collect() };
    let c = colour.map(|v| v as f32 / 255.0);
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let k = select::sample(&m, x as f32 + 0.5 - offset.0, y as f32 + 0.5 - offset.1)[3] as f32 / 255.0 * amount;
            if k <= 0.0 {
                continue;
            }
            let s = unpremultiply([p[0], p[1], p[2], p[3]]);
            let mut o = s;
            for ch in 0..3 {
                let v = s[ch] as f32 / 255.0;
                let t = if highlight { 1.0 - (1.0 - v) * (1.0 - c[ch]) } else { v * c[ch] };
                o[ch] = ((v + (t - v) * k) * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            p.copy_from_slice(&premultiply(o));
        }
    });
}

/// How to light from normals.
pub(crate) struct Light {
    /// Direction to the light across the picture, degrees (0: from the right, 90: from below).
    pub angle: f32,
    /// Height of the light above the picture, degrees (90: straight on).
    pub elevation: f32,
    /// How rounded the drawing's own shape is, pixels (unused with a normal map).
    pub depth: f32,
    pub intensity: f32,
    pub colour: [u8; 3],
}

/// Light the drawing from its normals: from `normals` (a normal map: red across, green up, blue
/// towards the viewer) where given and painted, else from the drawing's own shape rounded over
/// `depth` pixels. Faces turned to the light brighten (tinted by `colour`), faces turned away
/// darken; flat areas facing the viewer stay as they are.
pub(crate) fn normal_light(img: &mut RgbaImage, normals: Option<&RgbaImage>, l: &Light) {
    if l.intensity <= 0.0 {
        return;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let (az, el) = (l.angle.to_radians(), l.elevation.clamp(5.0, 90.0).to_radians());
    let light = (el.cos() * az.cos(), el.cos() * az.sin(), el.sin());
    // height from the drawing's coverage
    let mut height: Vec<u8> = img.data.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    blur::gaussian_plane(&mut height, w, h, (l.depth / 2.0).max(0.5));
    let hgt = |x: usize, y: usize| height.get(y.min(h.saturating_sub(1)) * w + x.min(w.saturating_sub(1))).copied().unwrap_or(0) as f32 / 255.0;
    let normals = normals.filter(|n| n.width == img.width && n.height == img.height);
    let tint = l.colour.map(|v| v as f32 / 255.0);
    par::rows(&mut img.data, w * 4, |y, row| {
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if p[3] == 0 {
                continue;
            }
            let from_map = normals.and_then(|n| n.data.get((y * w + x) * 4..(y * w + x) * 4 + 4)).filter(|q| q[3] > 0).map(|q| {
                let s = unpremultiply([q[0], q[1], q[2], q[3]]);
                (s[0] as f32 / 127.5 - 1.0, -(s[1] as f32 / 127.5 - 1.0), s[2] as f32 / 255.0)
            });
            let (nx, ny, nz) = from_map.unwrap_or_else(|| {
                let dx = hgt(x + 1, y) - hgt(x.saturating_sub(1), y);
                let dy = hgt(x, y + 1) - hgt(x, y.saturating_sub(1));
                // the coverage rises into the drawing: its slope points the surface outwards
                let k = l.depth.max(1.0) / 2.0;
                (-dx * k, -dy * k, 1.0)
            });
            let len = (nx * nx + ny * ny + nz * nz).sqrt().max(1e-4);
            let d = (nx * light.0 + ny * light.1 + nz * light.2) / len;
            // relative to a flat face, so flat areas are unchanged
            let rel = (d / light.2.max(0.1)).clamp(0.0, 2.0);
            let shade = 1.0 + l.intensity * (rel - 1.0);
            let s = unpremultiply([p[0], p[1], p[2], p[3]]);
            let mut o = s;
            for ch in 0..3 {
                let v = s[ch] as f32 / 255.0;
                let lit = if shade >= 1.0 { v + (shade - 1.0) * tint[ch] * (1.0 - v) * 2.0 } else { v * shade.max(0.0) };
                o[ch] = (lit * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            p.copy_from_slice(&premultiply(o));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matte_shades_and_highlights_only_where_it_covers() {
        let mut img = RgbaImage::filled(40, 20, [200, 200, 200, 255]);
        let mut map = RgbaImage::transparent(40, 20);
        for y in 0..20 {
            for x in 0..20 {
                map.set_pixel(x, y, [0, 0, 0, 255]);
            }
        }
        let mut shade = img.clone();
        matte_tone(&mut shade, &map, [0, 0, 255], 1.0, (0.0, 0.0), 0.0, false);
        assert_eq!(shade.pixel(5, 5), [0, 0, 200, 255], "multiplied by blue inside the matte");
        assert_eq!(shade.pixel(35, 5), [200, 200, 200, 255], "untouched outside");
        // moved 10 px right: the toned band starts later
        let mut moved = img.clone();
        matte_tone(&mut moved, &map, [0, 0, 0], 1.0, (10.0, 0.0), 0.0, false);
        assert_eq!(moved.pixel(5, 5)[0], 200);
        assert_eq!(moved.pixel(15, 5)[0], 0);
        matte_tone(&mut img, &map, [255, 255, 255], 0.5, (0.0, 0.0), 0.0, true);
        assert!(img.pixel(5, 5)[0] > 220, "a highlight lightens: {:?}", img.pixel(5, 5));
    }

    #[test]
    fn light_from_the_left_brightens_the_left_edge() {
        let mut disc = RgbaImage::transparent(60, 60);
        for y in 0..60u32 {
            for x in 0..60u32 {
                if (x as f32 - 30.0).hypot(y as f32 - 30.0) < 20.0 {
                    disc.set_pixel(x, y, premultiply([128, 128, 128, 255]));
                }
            }
        }
        let l = Light { angle: 180.0, elevation: 45.0, depth: 12.0, intensity: 1.0, colour: [255, 255, 255] };
        let mut lit = disc.clone();
        normal_light(&mut lit, None, &l);
        assert!(lit.pixel(13, 30)[0] > 128, "the left edge faces the light: {:?}", lit.pixel(13, 30));
        assert!(lit.pixel(47, 30)[0] < 128, "the right edge faces away: {:?}", lit.pixel(47, 30));
        assert_eq!(lit.pixel(30, 30)[0], 128, "the flat middle is unchanged");
        // a normal map facing the light everywhere brightens the middle too
        let map = RgbaImage::filled(60, 60, [0, 128, 180, 255]);
        let mut by_map = disc.clone();
        normal_light(&mut by_map, Some(&map), &l);
        assert!(by_map.pixel(30, 30)[0] > 128, "{:?}", by_map.pixel(30, 30));
    }
}
