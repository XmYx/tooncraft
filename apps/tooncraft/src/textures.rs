//! GPU tile textures. Every 64×64 tile shown on the canvas is a texture, uploaded once per tile
//! revision and reused while the tile is unchanged: drawing re-uploads only the tiles a stroke
//! touched, and scrubbing or playing back re-uploads nothing. Onion-skin and light-table
//! silhouettes are their own tinted textures, cached the same way. Memory is bounded by an LRU
//! budget.

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureId, TextureOptions};
use tooncraft_doc::DrawingId;
use tooncraft_raster::{TILE, TILE_BYTES, Tile};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    drawing: DrawingId,
    tile: u32,
    tint: Option<[u8; 3]>,
    outline: bool,
}

struct Entry {
    rev: u64,
    tex: TextureHandle,
    used: u64,
}

/// What the paper texture was made from: rebuilt only when one of these changes.
type PaperKey = ([u8; 3], tooncraft_doc::PaperTexture, Option<usize>);

pub struct TileTextures {
    map: HashMap<Key, Entry>,
    paper: Option<(PaperKey, TextureHandle)>,
    frame: u64,
    /// Most texture bytes kept.
    pub budget: usize,
    /// Uploads in the current frame (statistics).
    pub uploads: usize,
}

const OPTIONS: TextureOptions = TextureOptions {
    magnification: egui::TextureFilter::Linear,
    minification: egui::TextureFilter::Linear,
    wrap_mode: egui::TextureWrapMode::ClampToEdge,
    mipmap_mode: Some(egui::TextureFilter::Linear),
};

impl Default for TileTextures {
    fn default() -> Self {
        Self { map: HashMap::new(), paper: None, frame: 0, budget: 768 << 20, uploads: 0 }
    }
}

impl TileTextures {
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        self.uploads = 0;
    }

    pub fn bytes(&self) -> usize {
        self.map.len() * TILE_BYTES
    }

    /// The texture for a tile, uploading it when its revision changed.
    /// With `outline`, the texture shows only the outline of the tile's paint (onion skin).
    pub fn get(&mut self, ctx: &egui::Context, drawing: DrawingId, index: u32, tile: &Tile, tint: Option<[u8; 3]>, outline: bool) -> TextureId {
        let key = Key { drawing, tile: index, tint, outline };
        let img = |t: &Tile| if outline { image(&tooncraft_raster::reframe::outline(t), tint) } else { image(t, tint) };
        let frame = self.frame;
        if let Some(e) = self.map.get_mut(&key) {
            e.used = frame;
            if e.rev != tile.revision() {
                e.tex.set(img(tile), OPTIONS);
                e.rev = tile.revision();
                self.uploads += 1;
            }
            return e.tex.id();
        }
        let tex = ctx.load_texture(format!("tile-{}-{index}-{tint:?}-{outline}", drawing.0), img(tile), OPTIONS);
        let id = tex.id();
        self.uploads += 1;
        self.map.insert(key, Entry { rev: tile.revision(), tex, used: frame });
        id
    }

    /// Drop the least recently used textures over the budget (never ones used this frame).
    pub fn end_frame(&mut self) {
        if self.bytes() <= self.budget {
            return;
        }
        let mut old: Vec<(u64, Key)> = self.map.iter().filter(|(_, e)| e.used < self.frame).map(|(k, e)| (e.used, *k)).collect();
        old.sort_by_key(|(u, _)| *u);
        let excess = (self.bytes() - self.budget).div_ceil(TILE_BYTES);
        for (_, k) in old.into_iter().take(excess) {
            self.map.remove(&k);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.paper = None;
    }

    /// The repeating paper texture (`tooncraft_doc::PAPER_TILE` square, wrapping), or `None` for
    /// plain paper. Made once per change of the paper's settings, not per frame.
    pub fn paper(&mut self, ctx: &egui::Context, p: &tooncraft_doc::Project) -> Option<TextureId> {
        let t = p.paper.texture.sanitized();
        if t.is_off() {
            self.paper = None;
            return None;
        }
        // the image's identity: a replaced image under the same id makes a new texture
        let image = t.image.and_then(|i| p.tips.get(&i)).map(|a| std::sync::Arc::as_ptr(a) as usize);
        let key: PaperKey = (p.paper.color, t, image);
        if let Some((k, tex)) = &self.paper
            && *k == key
        {
            return Some(tex.id());
        }
        let n = tooncraft_doc::PAPER_TILE as usize;
        let img = ColorImage::from_rgba_unmultiplied([n, n], &p.paper_look().tile());
        let options = TextureOptions { wrap_mode: egui::TextureWrapMode::Repeat, ..OPTIONS };
        let tex = ctx.load_texture("paper", img, options);
        let id = tex.id();
        self.uploads += 1;
        self.paper = Some((key, tex));
        Some(id)
    }
}

fn image(tile: &Tile, tint: Option<[u8; 3]>) -> ColorImage {
    let size = [TILE as usize, TILE as usize];
    match tint {
        None => ColorImage::from_rgba_premultiplied(size, tile.bytes()),
        Some(t) => {
            let mut px = Vec::with_capacity(TILE_BYTES);
            for p in tile.bytes().as_chunks::<4>().0.iter() {
                let a = p[3] as u32;
                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                px.extend_from_slice(&[m(t[0]), m(t[1]), m(t[2]), p[3]]);
            }
            ColorImage::from_rgba_premultiplied(size, &px)
        }
    }
}
