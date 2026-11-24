//! Undo entries: lists of self-inverse swaps.

use tooncraft_doc::{DrawingId, Layer, Project};
use tooncraft_raster::{Surface, TILE_BYTES, TileSnapshot};

pub(crate) enum Part {
    /// Tiles of one drawing.
    Tiles {
        drawing: DrawingId,
        tiles: TileSnapshot,
    },
    /// The whole layer stack with its cells (small: no pixels).
    Layers(Vec<Layer>),
    /// A drawing in the library (`None`: absent).
    Drawing {
        id: DrawingId,
        surface: Option<Surface>,
    },
    /// Shot length (and the layer cells, which a length change cuts).
    Length {
        length: u32,
        layers: Vec<Layer>,
    },
    Fps(u32),
    Sounds(Vec<tooncraft_doc::Soundtrack>, Vec<tooncraft_doc::AudioTrack>),
    Camera(tooncraft_doc::Camera),
    Folders(Vec<tooncraft_doc::Folder>),
    Pegs(Vec<tooncraft_doc::Peg>),
    /// The whole project (canvas size changes; tiles are shared, so this is cheap).
    Whole(Box<Project>),
    Marks(std::collections::BTreeMap<tooncraft_doc::DrawingId, tooncraft_doc::Mark>),
    /// Frame markers.
    Markers(std::collections::BTreeMap<u32, String>),
    /// Drawing names.
    Names(std::collections::BTreeMap<tooncraft_doc::DrawingId, String>),
    Tip {
        id: u64,
        tip: Option<std::sync::Arc<tooncraft_brush::Tip>>,
    },
    /// The shot's drawing library (its items, not their drawings).
    Library(Vec<tooncraft_doc::library::Item>),
}

/// One undo step.
pub struct Entry {
    pub label: String,
    pub(crate) parts: Vec<Part>,
}

impl Entry {
    /// Bytes of tile data this step keeps alive (shared tiles counted in full).
    pub fn bytes(&self) -> u64 {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Tiles { tiles, .. } => tiles.iter().filter(|(_, t)| t.is_some()).count() as u64 * TILE_BYTES as u64,
                Part::Drawing { surface: Some(s), .. } => s.memory_bytes(),
                Part::Whole(p) => p.memory_bytes(),
                _ => 0,
            })
            .sum()
    }

    /// Swap every part with the project's current state, last part first. Returns the entry
    /// that swaps back.
    pub(crate) fn swap(self, p: &mut Project) -> Entry {
        let mut inverse = Vec::with_capacity(self.parts.len());
        for part in self.parts.into_iter().rev() {
            let inv = match part {
                Part::Tiles { drawing, tiles } => match p.drawing_mut(drawing) {
                    Some(s) => Part::Tiles { drawing, tiles: s.swap_snapshot(tiles) },
                    None => Part::Tiles { drawing, tiles },
                },
                Part::Layers(layers) => Part::Layers(std::mem::replace(&mut p.layers, layers)),
                Part::Drawing { id, surface } => Part::Drawing { id, surface: p.swap_drawing(id, surface) },
                Part::Length { length, layers } => {
                    let old = Part::Length { length: p.length(), layers: p.layers.clone() };
                    if p.set_length(length).is_ok() {
                        p.layers = layers;
                    }
                    old
                }
                Part::Tip { id, tip } => {
                    let old = match tip {
                        Some(t) => p.tips.insert(id, t),
                        None => p.tips.remove(&id),
                    };
                    Part::Tip { id, tip: old }
                }
                Part::Folders(f) => Part::Folders(std::mem::replace(&mut p.folders, f)),
                Part::Library(l) => Part::Library(std::mem::replace(&mut p.library, l)),
                Part::Pegs(g) => Part::Pegs(std::mem::replace(&mut p.pegs, g)),
                Part::Whole(w) => Part::Whole(Box::new(std::mem::replace(p, *w))),
                Part::Marks(m) => Part::Marks(std::mem::replace(&mut p.marks, m)),
                Part::Markers(m) => Part::Markers(std::mem::replace(&mut p.markers, m)),
                Part::Names(m) => Part::Names(std::mem::replace(&mut p.drawing_names, m)),
                Part::Camera(c) => Part::Camera(std::mem::replace(&mut p.camera, c)),
                Part::Sounds(s, t) => Part::Sounds(std::mem::replace(&mut p.sounds, s), std::mem::replace(&mut p.audio_tracks, t)),
                Part::Fps(fps) => {
                    let old = Part::Fps(p.fps());
                    let _ = p.set_fps(fps);
                    old
                }
            };
            inverse.push(inv);
        }
        Entry { label: self.label, parts: inverse }
    }
}
