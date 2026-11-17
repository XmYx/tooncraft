//! ToonCraft's raster engine (layer 0, UI-free).
//!
//! A [`Surface`] is a bounded canvas of 64×64 tiles of premultiplied RGBA8. Missing tiles are
//! transparent, so an empty drawing costs nothing and a sketch costs only the tiles it touches.
//! Tiles are `Arc`-shared: cloning a surface copies tile pointers, and writing to a tile copies
//! only that tile ([`Arc::make_mut`]). That makes undo snapshots, autosave snapshots and onion-skin
//! caching cheap. Every tile write gets a fresh, process-wide revision number, which the UI uses to
//! know when a cached texture is stale.
//!
//! [`fill`] is the flood fill with gap closing used by the paint bucket.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod fill;
mod image;
mod pixel;
pub mod select;
mod warp;

pub use image::RgbaImage;
pub use pixel::{BlendMode, blend, over, premultiply, scale_alpha, unpremultiply};
pub use select::sample as sample_bilinear;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Tile side in pixels.
pub const TILE: u32 = 64;
/// Pixels in one tile.
pub const TILE_PIXELS: usize = (TILE * TILE) as usize;
/// Bytes in one tile (premultiplied RGBA8).
pub const TILE_BYTES: usize = TILE_PIXELS * 4;
/// Largest canvas side ToonCraft accepts.
pub const MAX_SIDE: u32 = 16384;

/// Errors from the raster engine.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RasterError {
    #[error("canvas size {0}×{1} is outside 1…{MAX_SIDE} pixels per side")]
    BadSize(u32, u32),
    #[error("tile data has {0} bytes, expected {TILE_BYTES}")]
    BadTile(usize),
    #[error("tile index {0} is outside the canvas")]
    TileOutside(u32),
    #[error("image data has {got} bytes, expected {want}")]
    BadImage { got: usize, want: usize },
    #[error("the fill area is larger than {0} pixels")]
    FillTooLarge(u64),
}

static REVISION: AtomicU64 = AtomicU64::new(1);

fn next_revision() -> u64 {
    REVISION.fetch_add(1, Ordering::Relaxed)
}

/// One 64×64 tile of premultiplied RGBA8.
#[derive(Clone, Debug)]
pub struct Tile {
    px: Vec<u8>,
    rev: u64,
}

impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        self.px == other.px
    }
}

impl Tile {
    /// A transparent tile.
    pub fn transparent() -> Self {
        Self { px: vec![0; TILE_BYTES], rev: next_revision() }
    }

    /// A tile from exactly [`TILE_BYTES`] premultiplied bytes. Colour channels above alpha are
    /// clamped, so hostile data cannot break the premultiplied invariant.
    pub fn from_bytes(mut px: Vec<u8>) -> Result<Self, RasterError> {
        if px.len() != TILE_BYTES {
            return Err(RasterError::BadTile(px.len()));
        }
        for p in px.as_chunks_mut::<4>().0.iter_mut() {
            let a = p[3];
            p[0] = p[0].min(a);
            p[1] = p[1].min(a);
            p[2] = p[2].min(a);
        }
        Ok(Self { px, rev: next_revision() })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.px
    }

    /// Mutable pixels. Callers must keep colour ≤ alpha (premultiplied).
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.px
    }

    /// Revision: changes every time the tile is written through [`Surface::tile_mut`].
    pub fn revision(&self) -> u64 {
        self.rev
    }

    pub fn is_transparent(&self) -> bool {
        self.px.as_chunks::<4>().0.iter().all(|p| p[3] == 0)
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y.min(TILE - 1) * TILE + x.min(TILE - 1)) * 4) as usize;
        match self.px.get(i..i + 4) {
            Some(p) => [p[0], p[1], p[2], p[3]],
            None => [0; 4],
        }
    }
}

/// Saved tile states, keyed by tile index; `None` is a missing (transparent) tile.
pub type TileSnapshot = Vec<(u32, Option<Arc<Tile>>)>;

/// A bounded, sparse canvas of tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    width: u32,
    height: u32,
    cols: u32,
    rows: u32,
    tiles: BTreeMap<u32, Arc<Tile>>,
    capture: Option<BTreeMap<u32, Option<Arc<Tile>>>>,
}

impl Surface {
    pub fn new(width: u32, height: u32) -> Result<Self, RasterError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(RasterError::BadSize(width, height));
        }
        Ok(Self { width, height, cols: width.div_ceil(TILE), rows: height.div_ceil(TILE), tiles: BTreeMap::new(), capture: None })
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// Tile columns.
    pub fn cols(&self) -> u32 {
        self.cols
    }
    /// Tile rows.
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// Index of the tile at tile coordinates, if inside the canvas.
    pub fn tile_index(&self, tx: u32, ty: u32) -> Option<u32> {
        (tx < self.cols && ty < self.rows).then_some(ty * self.cols + tx)
    }

    /// Tile coordinates of a tile index.
    pub fn tile_coords(&self, index: u32) -> (u32, u32) {
        (index % self.cols.max(1), index / self.cols.max(1))
    }

    pub fn tile(&self, index: u32) -> Option<&Arc<Tile>> {
        self.tiles.get(&index)
    }

    /// Tiles that hold pixels, in index order.
    pub fn tiles(&self) -> impl Iterator<Item = (u32, &Arc<Tile>)> {
        self.tiles.iter().map(|(i, t)| (*i, t))
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Bytes of pixel data this surface references (shared tiles counted in full).
    pub fn memory_bytes(&self) -> u64 {
        self.tiles.len() as u64 * TILE_BYTES as u64
    }

    /// Start recording the state of every tile before its first write.
    pub fn begin_capture(&mut self) {
        self.capture = Some(BTreeMap::new());
    }

    /// Stop recording and return each written tile's state from before the capture began.
    pub fn take_capture(&mut self) -> TileSnapshot {
        self.capture.take().map(|m| m.into_iter().collect()).unwrap_or_default()
    }

    pub fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// The state of a tile before the running capture began (or now, without a capture).
    pub fn original(&self, index: u32) -> Option<Arc<Tile>> {
        if let Some(Some(saved)) = self.capture.as_ref().map(|c| c.get(&index)) {
            return saved.clone();
        }
        self.tiles.get(&index).cloned()
    }

    /// A writable tile, created transparent if missing. Bumps its revision and, during a capture,
    /// saves its previous state first. `None` when the index is outside the canvas.
    pub fn tile_mut(&mut self, index: u32) -> Option<&mut Tile> {
        if index >= self.cols.saturating_mul(self.rows) {
            return None;
        }
        if let Some(cap) = self.capture.as_mut() {
            cap.entry(index).or_insert_with(|| self.tiles.get(&index).cloned());
        }
        let tile = self.tiles.entry(index).or_insert_with(|| Arc::new(Tile::transparent()));
        let t = Arc::make_mut(tile);
        t.rev = next_revision();
        Some(t)
    }

    /// Put a tile state back (undo/redo). Returns the state it replaced.
    pub fn swap_tile(&mut self, index: u32, tile: Option<Arc<Tile>>) -> Result<Option<Arc<Tile>>, RasterError> {
        if index >= self.cols.saturating_mul(self.rows) {
            return Err(RasterError::TileOutside(index));
        }
        Ok(match tile {
            Some(t) => self.tiles.insert(index, t),
            None => self.tiles.remove(&index),
        })
    }

    /// Swap a whole snapshot in, returning the states it replaced (so the result undoes it).
    pub fn swap_snapshot(&mut self, snapshot: TileSnapshot) -> TileSnapshot {
        let mut out = Vec::with_capacity(snapshot.len());
        for (i, t) in snapshot {
            if let Ok(old) = self.swap_tile(i, t) {
                out.push((i, old));
            }
        }
        out
    }

    /// Drop tiles that became fully transparent.
    pub fn prune(&mut self) {
        self.tiles.retain(|_, t| !t.is_transparent());
    }

    /// Premultiplied pixel, transparent outside the canvas.
    pub fn pixel(&self, x: i64, y: i64) -> [u8; 4] {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return [0; 4];
        }
        let (x, y) = (x as u32, y as u32);
        match self.tile_index(x / TILE, y / TILE).and_then(|i| self.tiles.get(&i)) {
            Some(t) => t.pixel(x % TILE, y % TILE),
            None => [0; 4],
        }
    }

    /// Composite this surface over `dst` (same size, premultiplied) at `opacity`. With `tint`,
    /// colour is replaced by the tint (onion skin and light table silhouettes).
    pub fn composite_into(&self, dst: &mut RgbaImage, opacity: f32, tint: Option<[u8; 3]>) {
        if dst.width != self.width || dst.height != self.height {
            return;
        }
        self.composite_region(dst, 0, 0, opacity, tint);
    }

    /// Composite the part of this surface under `dst` placed at canvas position `(ox, oy)`.
    pub fn composite_region(&self, dst: &mut RgbaImage, ox: u32, oy: u32, opacity: f32, tint: Option<[u8; 3]>) {
        let o = (opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
        if o == 0 || dst.width == 0 || dst.height == 0 {
            return;
        }
        let (rx0, ry0) = (ox as u64, oy as u64);
        let (rx1, ry1) = (rx0 + dst.width as u64, ry0 + dst.height as u64);
        let w = dst.width as usize;
        for (index, tile) in &self.tiles {
            let (tx, ty) = self.tile_coords(*index);
            let (x0, y0) = (tx as u64 * TILE as u64, ty as u64 * TILE as u64);
            let x1 = (x0 + TILE as u64).min(self.width as u64).min(rx1);
            let y1 = (y0 + TILE as u64).min(self.height as u64).min(ry1);
            let (cx0, cy0) = (x0.max(rx0), y0.max(ry0));
            if cx0 >= x1 || cy0 >= y1 {
                continue;
            }
            let span = (x1 - cx0) as usize;
            for y in cy0..y1 {
                let trow = (y - y0) as usize;
                let tcol = (cx0 - x0) as usize;
                let src_start = (trow * TILE as usize + tcol) * 4;
                let Some(src_row) = tile.px.get(src_start..src_start + span * 4) else { continue };
                let start = ((y - ry0) as usize * w + (cx0 - rx0) as usize) * 4;
                let Some(dst_row) = dst.data.get_mut(start..start + span * 4) else { continue };
                for (d, s) in dst_row.as_chunks_mut::<4>().0.iter_mut().zip(src_row.as_chunks::<4>().0.iter()) {
                    if s[3] == 0 {
                        continue;
                    }
                    let mut px = [s[0], s[1], s[2], s[3]];
                    if let Some(t) = tint {
                        px = [mul(t[0], s[3]), mul(t[1], s[3]), mul(t[2], s[3]), s[3]];
                    }
                    if o < 255 {
                        px = scale_alpha(px, o as u8);
                    }
                    let out = over([d[0], d[1], d[2], d[3]], px);
                    d.copy_from_slice(&out);
                }
            }
        }
    }

    /// The whole surface as one premultiplied image.
    pub fn to_image(&self) -> RgbaImage {
        let mut img = RgbaImage::transparent(self.width, self.height);
        self.composite_into(&mut img, 1.0, None);
        img
    }

    /// Replace the surface's pixels with a premultiplied image of the same size.
    pub fn load_image(&mut self, img: &RgbaImage) -> Result<(), RasterError> {
        if img.width != self.width || img.height != self.height {
            return Err(RasterError::BadImage { got: img.data.len(), want: (self.width * self.height * 4) as usize });
        }
        self.tiles.clear();
        for ty in 0..self.rows {
            for tx in 0..self.cols {
                let mut tile = Tile::transparent();
                let mut any = false;
                for row in 0..TILE {
                    let y = ty * TILE + row;
                    if y >= self.height {
                        break;
                    }
                    for col in 0..TILE {
                        let x = tx * TILE + col;
                        if x >= self.width {
                            break;
                        }
                        let p = img.pixel(x, y);
                        if p[3] != 0 {
                            any = true;
                            let i = ((row * TILE + col) * 4) as usize;
                            tile.px[i..i + 4].copy_from_slice(&p);
                        }
                    }
                }
                if any && let Some(i) = self.tile_index(tx, ty) {
                    self.tiles.insert(i, Arc::new(tile));
                }
            }
        }
        Ok(())
    }

    /// Insert a loaded tile (file formats). Fails outside the canvas.
    pub fn insert_tile(&mut self, index: u32, tile: Tile) -> Result<(), RasterError> {
        if index >= self.cols.saturating_mul(self.rows) {
            return Err(RasterError::TileOutside(index));
        }
        if !tile.is_transparent() {
            self.tiles.insert(index, Arc::new(tile));
        }
        Ok(())
    }
}

fn mul(a: u8, b: u8) -> u8 {
    pixel::mul8(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_checked() {
        assert!(Surface::new(0, 10).is_err());
        assert!(Surface::new(MAX_SIDE + 1, 10).is_err());
        let s = Surface::new(100, 70).unwrap();
        assert_eq!((s.cols(), s.rows()), (2, 2));
        assert!(s.is_empty());
    }

    #[test]
    fn copy_on_write_and_capture() {
        let mut s = Surface::new(128, 128).unwrap();
        s.tile_mut(0).unwrap().bytes_mut()[0..4].copy_from_slice(&[10, 20, 30, 255]);
        let snapshot = s.clone();
        s.begin_capture();
        s.tile_mut(0).unwrap().bytes_mut()[0..4].copy_from_slice(&[255, 0, 0, 255]);
        s.tile_mut(3).unwrap().bytes_mut()[0..4].copy_from_slice(&[0, 0, 255, 255]);
        assert_eq!(snapshot.pixel(0, 0), [10, 20, 30, 255], "the clone is untouched");
        let cap = s.take_capture();
        assert_eq!(cap.len(), 2);
        let redo = s.swap_snapshot(cap);
        assert_eq!(s, snapshot);
        s.swap_snapshot(redo);
        assert_eq!(s.pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(s.pixel(64, 64), [0, 0, 255, 255]);
    }

    #[test]
    fn revisions_change_on_write() {
        let mut s = Surface::new(64, 64).unwrap();
        let a = s.tile_mut(0).unwrap().revision();
        let b = s.tile_mut(0).unwrap().revision();
        assert!(b > a);
    }

    #[test]
    fn image_round_trip_and_prune() {
        let mut img = RgbaImage::transparent(70, 65);
        img.set_pixel(69, 64, [40, 40, 40, 80]);
        let mut s = Surface::new(70, 65).unwrap();
        s.load_image(&img).unwrap();
        assert_eq!(s.tile_count(), 1);
        assert_eq!(s.to_image(), img);
        s.tile_mut(0);
        s.prune();
        assert_eq!(s.tile_count(), 1);
    }

    #[test]
    fn hostile_tile_bytes_are_clamped() {
        let t = Tile::from_bytes(vec![255; TILE_BYTES].into_iter().enumerate().map(|(i, v)| if i % 4 == 3 { 10 } else { v }).collect()).unwrap();
        assert_eq!(t.pixel(0, 0), [10, 10, 10, 10]);
        assert!(Tile::from_bytes(vec![0; 3]).is_err());
    }

    #[test]
    fn tint_composites_silhouette() {
        let mut s = Surface::new(8, 8).unwrap();
        s.tile_mut(0).unwrap().bytes_mut()[0..4].copy_from_slice(&[0, 0, 0, 255]);
        let mut img = RgbaImage::transparent(8, 8);
        s.composite_into(&mut img, 0.5, Some([255, 0, 0]));
        assert_eq!(img.pixel(0, 0), [128, 0, 0, 128]);
    }
}
