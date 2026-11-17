//! BoardCraft boards and animatics (`.boardcraft`), read as a layout for animation: each panel's
//! layers flattened into one picture, its duration, and its camera keys.
//!
//! The file is BoardCraft's documented project format (its `docs/file-format.md`): a ZIP with
//! `mimetype` `application/vnd.storytold.boardcraft+zip`, `project.json` (format
//! `boardcraft.project`, schema 1 or 2) and `layers/<layer id>.png`. Read here: the canvas size and
//! frame rate, scenes and their panels in order, each panel's `duration`, its layers (bottom to
//! top, with visibility and opacity) and its camera keys (`frame`, `center` 0…1, `zoom`,
//! `rotation`). Not read: annotations, sound clips, dialogue and notes beyond the panel label.
//! The file is hostile: entry counts, sizes and every number are bounded.

use serde::Deserialize;
use tooncraft_raster::RgbaImage;

use crate::common::{Budget, decode_png};
use crate::{InteropError, Result};

/// The format tag in `project.json`.
pub const FORMAT: &str = "boardcraft.project";
/// The newest schema this reader knows.
pub const SCHEMA: u32 = 2;
const MIMETYPE: &str = "application/vnd.storytold.boardcraft+zip";
/// Most panels read.
pub const MAX_PANELS: usize = 10_000;
/// Largest board canvas side (BoardCraft's own limit).
pub const MAX_SIDE: u32 = 8192;

/// One camera key of a panel: `frame` inside the panel, centre as 0…1 of the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoardKey {
    pub frame: u32,
    pub center: (f32, f32),
    pub zoom: f32,
    pub rotation: f32,
}

/// One panel, ready to become a drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardPanel {
    /// `Panel <scene>.<panel>`, as BoardCraft names it in exported timelines.
    pub label: String,
    pub duration: u32,
    /// The panel's visible layers flattened (premultiplied RGBA8, the board's canvas size).
    pub image: RgbaImage,
    pub camera: Vec<BoardKey>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub panels: Vec<BoardPanel>,
}

impl Board {
    /// Frames of all panels together.
    pub fn frames(&self) -> u64 {
        self.panels.iter().map(|p| p.duration as u64).sum()
    }
}

#[derive(Deserialize)]
struct Envelope {
    format: String,
    #[serde(default)]
    schema_version: u32,
    project: ProjectJson,
}

#[derive(Deserialize)]
struct ProjectJson {
    #[serde(default)]
    name: String,
    settings: Settings,
    #[serde(default)]
    scenes: Vec<SceneJson>,
}

#[derive(Deserialize)]
struct Settings {
    width: u32,
    height: u32,
    #[serde(default = "fps24")]
    fps: u32,
}

fn fps24() -> u32 {
    24
}

#[derive(Deserialize)]
struct SceneJson {
    #[serde(default)]
    number: String,
    #[serde(default)]
    panels: Vec<PanelJson>,
}

#[derive(Deserialize)]
struct PanelJson {
    #[serde(default = "one")]
    duration: u32,
    #[serde(default)]
    layers: Vec<LayerJson>,
    #[serde(default)]
    camera: CameraJson,
}

fn one() -> u32 {
    1
}

#[derive(Deserialize)]
struct LayerJson {
    id: u64,
    #[serde(default = "yes")]
    visible: bool,
    #[serde(default = "full")]
    opacity: f32,
}

fn yes() -> bool {
    true
}
fn full() -> f32 {
    1.0
}

#[derive(Deserialize, Default)]
struct CameraJson {
    #[serde(default)]
    keys: Vec<KeyJson>,
}

#[derive(Deserialize)]
struct KeyJson {
    #[serde(default)]
    frame: u32,
    #[serde(default = "centre")]
    center: [f32; 2],
    #[serde(default = "full")]
    zoom: f32,
    #[serde(default)]
    rotation: f32,
}

fn centre() -> [f32; 2] {
    [0.5, 0.5]
}

fn finite(v: f32, lo: f32, hi: f32, d: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { d }
}

/// Read a `.boardcraft` file.
pub fn read(bytes: &[u8]) -> Result<Board> {
    let bad = |m: String| InteropError::Decode(m);
    if craft_zip::stored_mimetype(bytes) != Some(MIMETYPE.as_bytes()) {
        return Err(bad("not a BoardCraft board (no BoardCraft mimetype entry)".into()));
    }
    let limits = craft_zip::Limits { max_entries: 200_000, max_total_bytes: crate::MAX_IMPORT_BYTES * 2 };
    let mut zip = craft_zip::Archive::open(bytes, limits).map_err(|e| bad(e.to_string()))?;
    let json = zip.read("project.json", 256 << 20).map_err(|e| bad(e.to_string()))?.ok_or_else(|| bad("project.json is missing".into()))?;
    let env: Envelope = serde_json::from_slice(&json).map_err(|e| bad(format!("project.json: {e}")))?;
    if env.format != FORMAT {
        return Err(bad("project.json is not a BoardCraft project".into()));
    }
    if env.schema_version > SCHEMA {
        return Err(bad(format!("the board was saved with schema {} (this ToonCraft reads up to {SCHEMA})", env.schema_version)));
    }
    let p = env.project;
    let (w, h) = (p.settings.width, p.settings.height);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(InteropError::Limit(format!("a {w}×{h} board (at most {MAX_SIDE} a side)")));
    }
    let mut budget = Budget::default();
    let mut panels = Vec::new();
    for (si, scene) in p.scenes.iter().enumerate() {
        let number = if scene.number.trim().is_empty() { (si + 1).to_string() } else { scene.number.chars().take(20).collect() };
        for (pi, panel) in scene.panels.iter().enumerate() {
            if panels.len() >= MAX_PANELS {
                return Err(InteropError::Limit(format!("more than {MAX_PANELS} panels")));
            }
            budget.take(w, h)?;
            let mut image = RgbaImage::transparent(w, h);
            for layer in panel.layers.iter().take(256) {
                if !layer.visible {
                    continue;
                }
                let Some(png) = zip.read(&format!("layers/{}.png", layer.id), 512 << 20).map_err(|e| bad(e.to_string()))? else { continue };
                let img = decode_png(&png, MAX_SIDE)?;
                if (img.width, img.height) != (w, h) {
                    return Err(bad(format!("layer {} is {}×{}, not the board's {w}×{h}", layer.id, img.width, img.height)));
                }
                image.over_image(&img.scaled(finite(layer.opacity, 0.0, 1.0, 1.0)));
            }
            let mut camera: Vec<BoardKey> = panel
                .camera
                .keys
                .iter()
                .take(10_000)
                .map(|k| BoardKey {
                    frame: k.frame.min(panel.duration.saturating_sub(1)),
                    center: (finite(k.center[0], -1.0, 2.0, 0.5), finite(k.center[1], -1.0, 2.0, 0.5)),
                    zoom: finite(k.zoom, 0.05, 16.0, 1.0),
                    rotation: finite(k.rotation, -720.0, 720.0, 0.0),
                })
                .collect();
            camera.sort_by_key(|k| k.frame);
            camera.dedup_by_key(|k| k.frame);
            panels.push(BoardPanel {
                label: format!("Panel {number}.{}", pi + 1),
                duration: panel.duration.clamp(1, tooncraft_doc::MAX_FRAMES),
                image,
                camera,
            });
        }
    }
    Ok(Board { name: p.name.chars().take(200).collect(), width: w, height: h, fps: p.settings.fps.clamp(1, 240), panels })
}

/// A small two-scene board in BoardCraft's documented format, written from scratch (tests and
/// examples): three panels of 6, 4 and 2 frames on a 64×32 canvas, the first with a camera move.
pub fn demo() -> Result<Vec<u8>> {
    let enc = |e: craft_zip::Error| InteropError::Encode(e.to_string());
    let png = |c: [u8; 4], x0: u32| -> Result<Vec<u8>> {
        let mut img = RgbaImage::transparent(64, 32);
        for y in 4..12 {
            for x in x0..x0 + 8 {
                img.set_pixel(x, y, tooncraft_raster::premultiply(c));
            }
        }
        tooncraft_format::encode_png(&img).map_err(|e| InteropError::Encode(e.to_string()))
    };
    let json = r#"{"format":"boardcraft.project","schema_version":2,"project":{"name":"Boats",
        "settings":{"width":64,"height":32,"fps":12},
        "scenes":[{"id":1,"number":"1","panels":[
            {"id":2,"duration":6,"layers":[{"id":10,"name":"Rough","visible":true,"opacity":1.0},{"id":11,"name":"Hidden","visible":false,"opacity":1.0}],
             "camera":{"keys":[{"frame":0,"center":[0.5,0.5],"zoom":1.0},{"frame":5,"center":[0.25,0.5],"zoom":2.0,"rotation":10.0}]}},
            {"id":3,"duration":4,"layers":[{"id":12,"name":"Ink","visible":true,"opacity":0.5}]}]},
          {"id":4,"number":"2A","panels":[{"id":5,"duration":2,"layers":[]}]}],
        "next_id":20}}"#;
    let mut z = craft_zip::Writer::new();
    z.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).map_err(enc)?;
    z.add("project.json", json.as_bytes(), craft_zip::Compression::Deflated).map_err(enc)?;
    z.add("layers/10.png", &png([200, 0, 0, 255], 4)?, craft_zip::Compression::Stored).map_err(enc)?;
    z.add("layers/11.png", &png([0, 200, 0, 255], 20)?, craft_zip::Compression::Stored).map_err(enc)?;
    z.add("layers/12.png", &png([0, 0, 200, 255], 40)?, craft_zip::Compression::Stored).map_err(enc)?;
    z.finish().map_err(enc)
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::demo as sample_result;

    fn sample() -> Vec<u8> {
        sample_result().unwrap()
    }

    #[test]
    fn reads_panels_timings_and_cameras() {
        let b = read(&sample()).unwrap();
        assert_eq!((b.name.as_str(), b.width, b.height, b.fps, b.frames()), ("Boats", 64, 32, 12, 12));
        let labels: Vec<&str> = b.panels.iter().map(|p| p.label.as_str()).collect();
        assert_eq!(labels, ["Panel 1.1", "Panel 1.2", "Panel 2A.1"]);
        let first = &b.panels[0];
        assert_eq!(first.image.pixel(6, 6), [200, 0, 0, 255]);
        assert_eq!(first.image.pixel(22, 6)[3], 0, "hidden layers are left out");
        assert_eq!(b.panels[1].image.pixel(42, 6)[3], 128, "layer opacity carries over");
        assert_eq!(first.camera.len(), 2);
        assert_eq!(first.camera[1], BoardKey { frame: 5, center: (0.25, 0.5), zoom: 2.0, rotation: 10.0 });
    }

    #[test]
    fn hostile_boards_are_refused() {
        assert!(read(b"PK\x03\x04garbage").is_err());
        assert!(read(&[]).is_err());
        let mut bytes = sample();
        // flip a byte in the middle: damaged entries give an error, never a panic
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
        let _ = read(&bytes);
    }
}
