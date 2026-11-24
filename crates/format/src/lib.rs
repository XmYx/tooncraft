//! ToonCraft's `.toon` project file (layer 2, UI-free).
//!
//! A `.toon` file is a ZIP archive (written and read with craft-zip):
//!
//! | Entry | Contents |
//! |---|---|
//! | `mimetype` | `application/vnd.storyteller.tooncraft`, stored first and uncompressed |
//! | `project.json` | format tag and version, canvas, timing, layers and their cells, onion skin, light table, the drawing list |
//! | `drawings/<id>.tiles` | one drawing: `TCT1`, a little-endian `u32` tile count, then per tile a `u32` tile index and 64×64 premultiplied RGBA8 |
//! | `preview.png` | the first frame, at most 256 px, for file browsers |
//!
//! Loading treats the file as hostile: entry count and expanded size are capped (craft-zip
//! limits), every number is range-checked, cells pointing at missing drawings are repaired, and
//! the result is a valid [`Project`] or an error, never a panic.
//!
//! Saving is crash-safe ([`save`]): a journal is written first, the previous file is kept as a
//! backup, the new bytes replace the file atomically (craft-atomic), are read back and compared,
//! and only then is the journal removed. A journal left behind means the last save was
//! interrupted; [`check`] says whether the file is good and where the backup is. [`autosave`]
//! writes recovery copies.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

pub mod autosave;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tooncraft_doc::{DrawingId, Layer, LightTable, OnionSkin, Paper, Project, RenderOptions};
use tooncraft_raster::{Surface, TILE_BYTES, Tile};

/// The ZIP `mimetype` entry.
pub const MIMETYPE: &str = "application/vnd.storyteller.tooncraft";
/// File extension (without the dot).
pub const EXTENSION: &str = "toon";
/// The format tag in `project.json`.
pub const FORMAT: &str = "tooncraft";
/// Current format version. Files from newer versions are refused with a clear message.
pub const VERSION: u32 = 1;
/// Most bytes a project may expand to when loaded.
pub const MAX_PROJECT_BYTES: u64 = 4 << 30;
/// Largest `project.json`.
pub const MAX_JSON_BYTES: u64 = 64 << 20;

const TILE_MAGIC: &[u8; 4] = b"TCT1";

#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    #[error("this is not a ToonCraft project ({0})")]
    NotToon(String),
    #[error("the project was saved by a newer ToonCraft (format {0}); update ToonCraft to open it")]
    Newer(u32),
    #[error("the project file is damaged: {0}")]
    Damaged(String),
    #[error("the project is too large to open ({0})")]
    TooLarge(String),
    #[error(transparent)]
    Doc(#[from] tooncraft_doc::DocError),
    #[error("could not write the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("the saved file did not read back the same; the previous version is in {0}")]
    Verify(String),
}

pub type Result<T> = std::result::Result<T, FormatError>;

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    #[serde(default)]
    app: String,
    #[serde(default)]
    saved_at: String,
    project: ProjectJson,
    #[serde(default)]
    drawings: Vec<DrawingEntry>,
    /// Files before sound clips: one soundtrack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    soundtrack: Option<SoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sounds: Vec<SoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    audio_tracks: Vec<tooncraft_doc::AudioTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tips: Vec<TipEntry>,
}

#[derive(Serialize, Deserialize)]
struct TipEntry {
    id: u64,
    name: String,
    width: u32,
    height: u32,
}

fn unit_gain() -> f32 {
    1.0
}

/// Each sound file once (clips may share one), with its entry name.
fn sound_files(p: &Project) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for s in &p.sounds {
        let ptr = std::sync::Arc::as_ptr(&s.bytes) as usize;
        if !out.iter().any(|(q, _)| *q == ptr) {
            out.push((ptr, format!("audio/sound-{}", out.len())));
        }
    }
    out
}

fn tip_entry_name(id: u64) -> String {
    format!("tips/{id}.alpha")
}

#[derive(Serialize, Deserialize)]
struct SoundEntry {
    name: String,
    offset: i32,
    #[serde(default)]
    track: u32,
    #[serde(default)]
    trim_start: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    length: Option<f64>,
    #[serde(default)]
    fade_in: f64,
    #[serde(default)]
    fade_out: f64,
    #[serde(default = "unit_gain")]
    volume: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    volume_keys: Vec<tooncraft_doc::VolumeKey>,
    #[serde(default)]
    muted: bool,
    entry: String,
}

/// Largest sound file kept in a shot.
pub const MAX_SOUND_BYTES: u64 = 512 << 20;

#[derive(Serialize, Deserialize)]
struct ProjectJson {
    name: String,
    width: u32,
    height: u32,
    fps: u32,
    length: u32,
    #[serde(default)]
    paper: Paper,
    layers: Vec<Layer>,
    #[serde(default)]
    onion: OnionSkin,
    #[serde(default)]
    light_table: LightTable,
    #[serde(default)]
    camera: tooncraft_doc::Camera,
    #[serde(default)]
    folders: Vec<tooncraft_doc::Folder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pegs: Vec<tooncraft_doc::Peg>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    marks: Vec<MarkEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    markers: Vec<MarkerEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    names: Vec<NameEntry>,
    #[serde(default, skip_serializing_if = "is_off")]
    symmetry: tooncraft_doc::symmetry::Symmetry,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    palettes: Vec<tooncraft_doc::palette::Palette>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    library: Vec<tooncraft_doc::library::Item>,
    #[serde(default, skip_serializing_if = "no_rulers")]
    rulers: tooncraft_doc::rulers::Rulers,
}

fn no_rulers(r: &tooncraft_doc::rulers::Rulers) -> bool {
    *r == tooncraft_doc::rulers::Rulers::default()
}

fn is_off(s: &tooncraft_doc::symmetry::Symmetry) -> bool {
    !s.is_on()
}

#[derive(Serialize, Deserialize)]
struct NameEntry {
    drawing: DrawingId,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct MarkerEntry {
    frame: u32,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct MarkEntry {
    drawing: DrawingId,
    mark: tooncraft_doc::Mark,
}

#[derive(Serialize, Deserialize)]
struct DrawingEntry {
    id: DrawingId,
    tiles: u32,
}

fn entry_name(id: DrawingId) -> String {
    format!("drawings/{}.tiles", id.0)
}

/// Encode a project as `.toon` bytes.
pub fn to_bytes(p: &Project) -> Result<Vec<u8>> {
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        app: format!("ToonCraft {}", env!("CARGO_PKG_VERSION")),
        saved_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        project: ProjectJson {
            name: p.name.clone(),
            width: p.width(),
            height: p.height(),
            fps: p.fps(),
            length: p.length(),
            paper: p.paper,
            layers: p.layers.clone(),
            onion: p.onion,
            light_table: p.light_table.clone(),
            symmetry: p.symmetry,
            palettes: p.palettes.clone(),
            library: p.library.clone(),
            rulers: p.rulers.clone(),
            camera: p.camera.clone(),
            folders: p.folders.clone(),
            pegs: p.pegs.clone(),
            marks: p.marks.iter().map(|(d, m)| MarkEntry { drawing: *d, mark: *m }).collect(),
            markers: p.markers.iter().map(|(f, t)| MarkerEntry { frame: *f, text: t.clone() }).collect(),
            names: p.drawing_names.iter().map(|(d, n)| NameEntry { drawing: *d, name: n.clone() }).collect(),
        },
        drawings: Vec::new(),
        soundtrack: None,
        sounds: p
            .sounds
            .iter()
            .map(|s| {
                let ptr = std::sync::Arc::as_ptr(&s.bytes) as usize;
                let entry = sound_files(p).into_iter().find(|(q, _)| *q == ptr).map(|(_, e)| e).unwrap_or_default();
                SoundEntry {
                    name: s.name.clone(),
                    offset: s.offset,
                    track: s.track,
                    trim_start: s.trim_start,
                    length: s.length,
                    fade_in: s.fade_in,
                    fade_out: s.fade_out,
                    volume: s.volume,
                    volume_keys: s.volume_keys.clone(),
                    muted: s.muted,
                    entry,
                }
            })
            .collect(),
        audio_tracks: p.audio_tracks.clone(),
        tips: p.tips.iter().map(|(id, t)| TipEntry { id: *id, name: t.name.clone(), width: t.width, height: t.height }).collect(),
    };
    let mut manifest = manifest;
    let mut zip = craft_zip::Writer::new();
    let z = |e: craft_zip::Error| FormatError::Damaged(e.to_string());
    zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).map_err(z)?;
    // Drawings one at a time: a paged drawing is read back for its entry and dropped again, so
    // saving a long shot does not load it all.
    let mut entries = Vec::with_capacity(p.drawing_count());
    for id in p.drawing_ids() {
        let Some(s) = p.drawing_owned(id)? else { continue };
        entries.push(DrawingEntry { id, tiles: s.tile_count() as u32 });
        let mut blob = Vec::with_capacity(8 + s.tile_count() * (4 + TILE_BYTES));
        blob.extend_from_slice(TILE_MAGIC);
        blob.extend_from_slice(&(s.tile_count() as u32).to_le_bytes());
        for (index, tile) in s.tiles() {
            blob.extend_from_slice(&index.to_le_bytes());
            blob.extend_from_slice(tile.bytes());
        }
        zip.add(&entry_name(id), &blob, craft_zip::Compression::Level(1)).map_err(z)?;
    }
    manifest.drawings = entries;
    let json = serde_json::to_vec_pretty(&manifest).map_err(|e| FormatError::Damaged(e.to_string()))?;
    zip.add("project.json", &json, craft_zip::Compression::Deflated).map_err(z)?;
    for (id, t) in &p.tips {
        zip.add(&tip_entry_name(*id), &t.alpha, craft_zip::Compression::Deflated).map_err(z)?;
    }
    for (ptr, entry) in sound_files(p) {
        if let Some(s) = p.sounds.iter().find(|s| std::sync::Arc::as_ptr(&s.bytes) as usize == ptr) {
            // Compressed formats gain nothing from deflate; WAV and AIFF do.
            zip.add(&entry, &s.bytes, craft_zip::Compression::Level(1)).map_err(z)?;
        }
    }
    let preview = tooncraft_doc::render_frame(p, 0, RenderOptions::default()).downscaled(256);
    if let Ok(png) = encode_png(&preview) {
        zip.add("preview.png", &png, craft_zip::Compression::Stored).map_err(z)?;
    }
    zip.finish().map_err(z)
}

/// Encode a premultiplied image as PNG (straight alpha).
pub fn encode_png(img: &tooncraft_raster::RgbaImage) -> Result<Vec<u8>> {
    use craft_image_codecs::{ChannelLayout, EncodeOptions, Format, Image, SampleType};
    let image =
        Image::from_raw(img.width, img.height, ChannelLayout::Rgba, SampleType::U8, img.to_straight()).map_err(|e| FormatError::Damaged(e.to_string()))?;
    craft_image_codecs::encode(&image, Format::Png, &EncodeOptions::default()).map_err(|e| FormatError::Damaged(e.to_string()))
}

/// Decode `.toon` bytes. Hostile input gives an error, never a panic.
pub fn from_bytes(bytes: &[u8]) -> Result<Project> {
    from_bytes_with(bytes, None)
}

/// Decode `.toon` bytes, paging drawings out to `paging.0` whenever the drawings in memory pass
/// `paging.1` bytes (long shots).
pub fn from_bytes_with(bytes: &[u8], paging: Option<(&std::sync::Arc<tooncraft_doc::paging::PageFile>, u64)>) -> Result<Project> {
    decode(bytes, paging, false)
}

/// Whether `.toon` bytes load, decoding every drawing but keeping none (memory stays at one
/// drawing however long the shot is). Saving uses it to decide whether the old file is worth
/// keeping as the backup.
pub fn validate(bytes: &[u8]) -> Result<()> {
    decode(bytes, None, true).map(|_| ())
}

fn decode(bytes: &[u8], paging: Option<(&std::sync::Arc<tooncraft_doc::paging::PageFile>, u64)>, discard: bool) -> Result<Project> {
    if craft_zip::stored_mimetype(bytes) != Some(MIMETYPE.as_bytes()) {
        return Err(FormatError::NotToon("no ToonCraft mimetype entry".into()));
    }
    let limits = craft_zip::Limits { max_entries: tooncraft_doc::MAX_DRAWINGS as u64 + 16, max_total_bytes: MAX_PROJECT_BYTES };
    let mut zip = craft_zip::Archive::open(bytes, limits)
        .map_err(|e| if e.is_limit() { FormatError::TooLarge(e.to_string()) } else { FormatError::Damaged(e.to_string()) })?;
    let read = |zip: &mut craft_zip::Archive, name: &str, max: u64| -> Result<Vec<u8>> {
        match zip.read(name, max) {
            Ok(Some(b)) => Ok(b),
            Ok(None) => Err(FormatError::Damaged(format!("{name} is missing"))),
            Err(e) if e.is_limit() => Err(FormatError::TooLarge(e.to_string())),
            Err(e) => Err(FormatError::Damaged(e.to_string())),
        }
    };
    let json = read(&mut zip, "project.json", MAX_JSON_BYTES)?;
    let head: serde_json::Value = serde_json::from_slice(&json).map_err(|e| FormatError::Damaged(format!("project.json: {e}")))?;
    if head.get("format").and_then(|f| f.as_str()) != Some(FORMAT) {
        return Err(FormatError::NotToon("project.json has no ToonCraft format tag".into()));
    }
    let version = head.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version > VERSION as u64 {
        return Err(FormatError::Newer(version.min(u32::MAX as u64) as u32));
    }
    let m: Manifest = serde_json::from_value(head).map_err(|e| FormatError::Damaged(format!("project.json: {e}")))?;
    let pj = m.project;
    // Size check before any drawing is allocated.
    let blank = Surface::new(pj.width, pj.height).map_err(|e| FormatError::Damaged(e.to_string()))?;
    if m.drawings.len() > tooncraft_doc::MAX_DRAWINGS {
        return Err(FormatError::TooLarge(format!("{} drawings", m.drawings.len())));
    }
    if pj.layers.len() > tooncraft_doc::MAX_LAYERS || pj.layers.iter().any(|l| l.cells.len() > tooncraft_doc::MAX_FRAMES as usize) {
        return Err(FormatError::TooLarge("too many layers or frames".into()));
    }
    let max_tiles = blank.cols() as u64 * blank.rows() as u64;
    // Check the list first, then build the project around empty stand-ins and read the drawings
    // in one at a time, so a paged load never holds more than its budget.
    let mut placeholders = BTreeMap::new();
    for d in &m.drawings {
        if placeholders.contains_key(&d.id) {
            return Err(FormatError::Damaged(format!("drawing {} is listed twice", d.id.0)));
        }
        if d.tiles as u64 > max_tiles {
            return Err(FormatError::Damaged(format!("drawing {} has more tiles than the canvas", d.id.0)));
        }
        placeholders.insert(d.id, blank.clone());
    }
    let mut p = Project::from_parts(pj.name, (pj.width, pj.height), pj.fps, pj.length, pj.paper, pj.layers, placeholders, pj.onion, pj.light_table)?;
    p.symmetry = pj.symmetry.sanitized();
    p.palettes = tooncraft_doc::palette::sanitize_all(pj.palettes);
    p.rulers = pj.rulers.sanitized();
    let none = std::collections::BTreeSet::new();
    for d in &m.drawings {
        let want = 8 + d.tiles as u64 * (4 + TILE_BYTES as u64);
        let blob = read(&mut zip, &entry_name(d.id), want)?;
        let s = decode_tiles(&blob, d.tiles, blank.clone()).map_err(|e| FormatError::Damaged(format!("drawing {}: {e}", d.id.0)))?;
        if discard {
            continue;
        }
        p.swap_drawing(d.id, Some(s));
        if let Some((file, budget)) = paging
            && p.memory_bytes() > budget
        {
            p.page_out(file, &none, budget / 2)?;
        }
    }
    let ids: std::collections::BTreeSet<DrawingId> = p.drawing_ids().collect();
    p.library = tooncraft_doc::library::sanitize_all(pj.library, |d| ids.contains(&d));
    p.camera = pj.camera.sanitized(p.width(), p.height());
    p.set_pegs(pj.pegs);
    for n in pj.names.into_iter().take(tooncraft_doc::MAX_MARKERS) {
        // a name for a missing drawing, or a repeated name, is dropped
        let _ = p.set_drawing_name(n.drawing, &n.name);
    }
    for m in pj.markers.into_iter().take(tooncraft_doc::MAX_MARKERS) {
        // hostile files: bad frames and over-long notes are dropped or cut
        let _ = p.set_marker(m.frame, &m.text);
    }
    p.marks = pj.marks.into_iter().filter(|m| p.drawing(m.drawing).is_some()).map(|m| (m.drawing, m.mark)).collect();
    p.set_folders(
        pj.folders
            .into_iter()
            .take(tooncraft_doc::MAX_FOLDERS)
            .map(|mut f| {
                f.name = f.name.chars().take(200).collect();
                f.parent = f.parent.map(|p| p.chars().take(200).collect());
                f
            })
            .collect(),
    );
    if m.tips.len() > 256 {
        return Err(FormatError::TooLarge(format!("{} brush tips", m.tips.len())));
    }
    for t in &m.tips {
        if t.width == 0 || t.height == 0 || t.width > tooncraft_brush::MAX_TIP_SIDE || t.height > tooncraft_brush::MAX_TIP_SIDE {
            return Err(FormatError::Damaged(format!("brush tip {} has a bad size", t.id)));
        }
        let alpha = read(&mut zip, &tip_entry_name(t.id), t.width as u64 * t.height as u64)?;
        let tip = tooncraft_brush::Tip::new(&t.name, t.width, t.height, alpha).ok_or_else(|| FormatError::Damaged(format!("brush tip {}", t.id)))?;
        p.tips.insert(t.id, std::sync::Arc::new(tip));
    }
    let mut entries: Vec<SoundEntry> = m.sounds;
    if let Some(s) = m.soundtrack {
        if s.entry != "audio/soundtrack" {
            return Err(FormatError::Damaged("unexpected sound entry".into()));
        }
        entries.insert(0, s);
    }
    if entries.len() > tooncraft_doc::sound::MAX_SOUNDS {
        return Err(FormatError::TooLarge("too many sound clips".into()));
    }
    let mut files: Vec<(String, std::sync::Arc<Vec<u8>>)> = Vec::new();
    let mut total = 0u64;
    for s in entries {
        if s.entry != "audio/soundtrack" && !(s.entry.starts_with("audio/sound-") && s.entry[12..].bytes().all(|b| b.is_ascii_digit())) {
            return Err(FormatError::Damaged("unexpected sound entry".into()));
        }
        let bytes = match files.iter().find(|(e, _)| *e == s.entry) {
            Some((_, b)) => b.clone(),
            None => {
                let b = std::sync::Arc::new(read(&mut zip, &s.entry, MAX_SOUND_BYTES)?);
                total += b.len() as u64;
                if total > MAX_SOUND_BYTES * 4 {
                    return Err(FormatError::TooLarge("the sound files are too large".into()));
                }
                files.push((s.entry.clone(), b.clone()));
                b
            }
        };
        let mut clip = tooncraft_doc::Soundtrack::new(&s.name, s.offset, bytes);
        clip.track = s.track;
        clip.trim_start = s.trim_start;
        clip.length = s.length;
        clip.fade_in = s.fade_in;
        clip.fade_out = s.fade_out;
        clip.volume = s.volume;
        clip.volume_keys = s.volume_keys;
        clip.muted = s.muted;
        p.sounds.push(clip.sanitized());
    }
    p.audio_tracks = m.audio_tracks.into_iter().take(tooncraft_doc::sound::MAX_AUDIO_TRACKS).map(tooncraft_doc::AudioTrack::sanitized).collect();
    Ok(p)
}

fn decode_tiles(blob: &[u8], count: u32, mut surface: Surface) -> std::result::Result<Surface, String> {
    if blob.get(..4) != Some(TILE_MAGIC.as_slice()) {
        return Err("bad tile header".into());
    }
    let n = blob.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("truncated")?;
    if n != count || blob.len() as u64 != 8 + n as u64 * (4 + TILE_BYTES as u64) {
        return Err("tile count does not match the data".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for chunk in blob[8..].as_chunks::<{ 4 + TILE_BYTES }>().0 {
        let index = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if !seen.insert(index) {
            return Err(format!("tile {index} appears twice"));
        }
        let tile = Tile::from_bytes(chunk[4..].to_vec()).map_err(|e| e.to_string())?;
        surface.insert_tile(index, tile).map_err(|e| e.to_string())?;
    }
    Ok(surface)
}

/// Where the save journal of `path` lives.
pub fn journal_path(path: &Path) -> PathBuf {
    sibling(path, ".journal")
}

/// Where the previous version of `path` is kept.
pub fn backup_path(path: &Path) -> PathBuf {
    sibling(path, ".bak")
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

#[derive(Serialize, Deserialize)]
struct Journal {
    target: String,
    started_at: String,
    bytes: u64,
}

/// What a save did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveReport {
    pub bytes: u64,
    pub backup: Option<PathBuf>,
}

/// Save crash-safely: journal → backup of the previous file → atomic replace → read-back
/// verification → journal removed.
pub fn save(path: &Path, p: &Project) -> Result<SaveReport> {
    let bytes = to_bytes(p)?;
    save_bytes(path, &bytes)
}

/// [`save`] for bytes already encoded (the autosave thread encodes off the UI thread).
pub fn save_bytes(path: &Path, bytes: &[u8]) -> Result<SaveReport> {
    let journal = journal_path(path);
    let j = Journal {
        target: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        started_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        bytes: bytes.len() as u64,
    };
    craft_atomic::atomic_write(&journal, &serde_json::to_vec(&j).map_err(|e| FormatError::Damaged(e.to_string()))?)?;
    let mut backup = None;
    if path.is_file() {
        let old = std::fs::read(path)?;
        // Only a file that loads is worth keeping as the backup.
        if validate(&old).is_ok() {
            let b = backup_path(path);
            craft_atomic::atomic_write(&b, &old)?;
            backup = Some(b);
        }
    }
    craft_atomic::atomic_write(path, bytes)?;
    let back = std::fs::read(path)?;
    if back != bytes {
        let where_ = backup.as_ref().map(|b| b.display().to_string()).unwrap_or_else(|| "no backup".into());
        return Err(FormatError::Verify(where_));
    }
    let _ = std::fs::remove_file(&journal);
    Ok(SaveReport { bytes: bytes.len() as u64, backup })
}

/// The next free versioned name next to `path`: `shot.toon` → `shot_v002.toon`,
/// `shot_v002.toon` → `shot_v003.toon` (skipping names that exist).
pub fn next_version_path(path: &Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "shot".into());
    let (base, mut n) = match stem.rsplit_once("_v") {
        Some((b, v)) if !b.is_empty() && v.len() >= 2 && v.chars().all(|c| c.is_ascii_digit()) => (b.to_string(), v.parse::<u32>().unwrap_or(1)),
        _ => (stem.clone(), 1),
    };
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    loop {
        n = n.saturating_add(1);
        let candidate = dir.join(format!("{base}_v{n:03}.{EXTENSION}"));
        if !candidate.exists() || n == u32::MAX {
            return candidate;
        }
    }
}

/// Open a project file.
pub fn load(path: &Path) -> Result<Project> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err(FormatError::TooLarge(format!("{} bytes", meta.len())));
    }
    from_bytes(&std::fs::read(path)?)
}

/// [`load`] with paging (see [`from_bytes_with`]).
pub fn load_with(path: &Path, paging: Option<(&std::sync::Arc<tooncraft_doc::paging::PageFile>, u64)>) -> Result<Project> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err(FormatError::TooLarge(format!("{} bytes", meta.len())));
    }
    from_bytes_with(&std::fs::read(path)?, paging)
}

/// The state of a project file after a possible crash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// No interrupted save.
    Clean,
    /// A save was interrupted after the new file was complete; the file is good (the stale
    /// journal was removed).
    Recovered,
    /// A save was interrupted and the file does not load. The backup, if any, is the last good
    /// version.
    Interrupted { backup: Option<PathBuf> },
}

/// Check for an interrupted save of `path` (call before opening it).
pub fn check(path: &Path) -> FileState {
    let journal = journal_path(path);
    if !journal.is_file() {
        return FileState::Clean;
    }
    if load(path).is_ok() {
        let _ = std::fs::remove_file(&journal);
        return FileState::Recovered;
    }
    let b = backup_path(path);
    FileState::Interrupted { backup: (b.is_file() && load(&b).is_ok()).then_some(b) }
}

#[cfg(test)]
mod tests;
