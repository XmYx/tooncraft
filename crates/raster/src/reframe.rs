//! Moving a surface onto a canvas of another size (canvas resize and crop) and outlines of a
//! drawing's paint (onion skin drawn as outlines).

use std::sync::Arc;

use crate::{RasterError, Surface, TILE, Tile};

impl Surface {
    /// This surface's pixels on a new `width`×`height` canvas, moved by `(dx, dy)` whole pixels;
    /// what falls outside is cut. Nothing is resampled. A move by whole tiles reuses the tiles.
    pub fn reframed(&self, width: u32, height: u32, dx: i64, dy: i64) -> Result<Surface, RasterError> {
        let mut out = Surface::new(width, height)?;
        let t = TILE as i64;
        let aligned = dx.rem_euclid(t) == 0 && dy.rem_euclid(t) == 0;
        for (index, tile) in self.tiles() {
            let (tx, ty) = self.tile_coords(index);
            let (x0, y0) = (tx as i64 * t, ty as i64 * t);
            if aligned && width.is_multiple_of(TILE) && height.is_multiple_of(TILE) && self.width.is_multiple_of(TILE) && self.height.is_multiple_of(TILE) {
                let (nx, ny) = ((x0 + dx) / t, (y0 + dy) / t);
                if nx >= 0
                    && ny >= 0
                    && let Some(i) = out.tile_index(nx as u32, ny as u32)
                {
                    out.tiles.insert(i, Arc::clone(tile));
                }
                continue;
            }
            for row in 0..TILE {
                for col in 0..TILE {
                    let (sx, sy) = (x0 + col as i64, y0 + row as i64);
                    if sx >= self.width as i64 || sy >= self.height as i64 {
                        continue;
                    }
                    let p = tile.pixel(col, row);
                    if p[3] == 0 {
                        continue;
                    }
                    let (nx, ny) = (sx + dx, sy + dy);
                    if nx < 0 || ny < 0 || nx >= width as i64 || ny >= height as i64 {
                        continue;
                    }
                    let (nx, ny) = (nx as u32, ny as u32);
                    let Some(i) = out.tile_index(nx / TILE, ny / TILE) else { continue };
                    let Some(dst) = out.tile_mut(i) else { continue };
                    let o = (((ny % TILE) * TILE + nx % TILE) * 4) as usize;
                    if let Some(px) = dst.bytes_mut().get_mut(o..o + 4) {
                        px.copy_from_slice(&p);
                    }
                }
            }
        }
        out.prune();
        Ok(out)
    }
}

impl Surface {
    /// The outlines of this surface's paint (see [`outline`]).
    pub fn outlined(&self) -> Surface {
        let mut out = self.clone();
        out.capture = None;
        for t in out.tiles.values_mut() {
            *t = Arc::new(outline(t));
        }
        out
    }
}

/// The outline of a tile's paint: pixels with paint next to a pixel without, at full coverage;
/// everything else transparent. Neighbours outside the tile count as painted, so paint running
/// across tile edges draws no seam. Used for onion skin drawn as outlines.
pub fn outline(tile: &Tile) -> Tile {
    let mut out = Tile::transparent();
    let a = |x: i64, y: i64| -> u8 {
        if x < 0 || y < 0 || x >= TILE as i64 || y >= TILE as i64 {
            return 255;
        }
        tile.pixel(x as u32, y as u32)[3]
    };
    for y in 0..TILE as i64 {
        for x in 0..TILE as i64 {
            let here = a(x, y);
            if here < 64 {
                continue;
            }
            if [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(ox, oy)| a(x + ox, y + oy) < 64) {
                let p = tile.pixel(x as u32, y as u32);
                let k = ((y as u32 * TILE + x as u32) * 4) as usize;
                // full coverage, colour un-premultiplied to opaque
                let un = crate::unpremultiply(p);
                out.px[k..k + 4].copy_from_slice(&[un[0], un[1], un[2], 255]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(s: &mut Surface, x0: u32, y0: u32, n: u32) {
        for y in y0..y0 + n {
            for x in x0..x0 + n {
                let i = s.tile_index(x / TILE, y / TILE).unwrap();
                let o = (((y % TILE) * TILE + x % TILE) * 4) as usize;
                s.tile_mut(i).unwrap().bytes_mut()[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }

    #[test]
    fn reframing_moves_and_cuts_without_resampling() {
        let mut s = Surface::new(200, 100).unwrap();
        square(&mut s, 10, 10, 5);
        let big = s.reframed(400, 300, 100, 50).unwrap();
        assert_eq!(big.pixel(112, 62), [0, 0, 0, 255]);
        assert_eq!(big.pixel(12, 12), [0, 0, 0, 0]);
        let crop = s.reframed(20, 20, -8, -8).unwrap();
        assert_eq!(crop.pixel(2, 2), [0, 0, 0, 255]);
        assert_eq!(crop.pixel(10, 10), [0, 0, 0, 0]);
        let gone = s.reframed(50, 50, -100, 0).unwrap();
        assert!(gone.is_empty());
        // whole-tile moves share the tiles
        let mut t = Surface::new(128, 128).unwrap();
        square(&mut t, 3, 3, 4);
        let moved = t.reframed(256, 128, 64, 0).unwrap();
        assert_eq!(moved.pixel(68, 4), [0, 0, 0, 255]);
        assert!(Arc::ptr_eq(moved.tile(moved.tile_index(1, 0).unwrap()).unwrap(), t.tile(0).unwrap()));
    }

    #[test]
    fn outlines_keep_only_the_edge() {
        let mut s = Surface::new(64, 64).unwrap();
        square(&mut s, 10, 10, 10);
        let o = outline(s.tile(0).unwrap());
        assert_eq!(o.pixel(10, 15)[3], 255, "left edge");
        assert_eq!(o.pixel(15, 15)[3], 0, "inside");
        assert_eq!(o.pixel(5, 5)[3], 0, "outside");
    }
}
