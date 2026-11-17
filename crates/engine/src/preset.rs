//! Brush presets (`.toonbrush`): a ZIP with `brush.json` (every setting), and for image tips
//! `tip.png`, for a grain image `grain.png` — portable between shots and machines. The format is
//! documented in `docs/toonbrush-format.md`.

use std::path::Path;

use tooncraft_brush::{Brush, GrainKind, MAX_TIP_SIDE, Tip};

pub const EXTENSION: &str = "toonbrush";
const MIMETYPE: &str = "application/vnd.storyteller.tooncraft-brush";
/// The version this build writes. Readers accept this and older versions.
pub const VERSION: u32 = 2;

#[derive(serde::Serialize, serde::Deserialize)]
struct PresetJson {
    format: String,
    version: u32,
    name: String,
    brush: Brush,
    #[serde(default)]
    tip_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grain_name: Option<String>,
}

/// A brush with its name and the images it needs.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: String,
    /// `tip` and `grain_image` are always `None` here: the images travel beside the brush and are
    /// given ids when the preset is put to use.
    pub brush: Brush,
    pub tip: Option<Tip>,
    /// A grain image: the value is the height, light is a peak.
    pub grain: Option<Tip>,
}

/// A tip from an image file (any format craft-image-codecs reads), scaled down to fit 1024 px.
pub fn tip_from_image(path: &str) -> Result<Tip, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    tip_from_bytes(&bytes, &std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "tip".into()))
}

fn tip_from_bytes(bytes: &[u8], name: &str) -> Result<Tip, String> {
    let (w, h) = craft_image_codecs::probe_size(bytes).map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("a {w}×{h} image is too large for a brush tip"));
    }
    let img = craft_image_codecs::decode(bytes).map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let mut rgba = tooncraft_raster::RgbaImage::transparent(w, h);
    for (i, p) in img.to_rgba8().as_chunks::<4>().0.iter().enumerate() {
        rgba.set_pixel(i as u32 % w, i as u32 / w, tooncraft_raster::premultiply(*p));
    }
    let small = rgba.downscaled(MAX_TIP_SIDE);
    Tip::from_rgba(name, small.width, small.height, &small.to_straight()).ok_or_else(|| "unusable tip image".to_string())
}

fn grain_from_bytes(bytes: &[u8], name: &str) -> Result<Tip, String> {
    let (w, h) = craft_image_codecs::probe_size(bytes).map_err(|e| e.to_string())?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("a {w}×{h} image is too large for a grain"));
    }
    let img = craft_image_codecs::decode(bytes).map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    let mut rgba = tooncraft_raster::RgbaImage::transparent(w, h);
    for (i, p) in img.to_rgba8().as_chunks::<4>().0.iter().enumerate() {
        rgba.set_pixel(i as u32 % w, i as u32 / w, tooncraft_raster::premultiply(*p));
    }
    let small = rgba.downscaled(MAX_TIP_SIDE);
    Tip::grain_from_rgba(name, small.width, small.height, &small.to_straight()).ok_or_else(|| "unusable grain image".to_string())
}

/// A grain from an image file (any format craft-image-codecs reads), scaled down to fit 1024 px.
pub fn grain_from_image(path: &str) -> Result<Tip, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    grain_from_bytes(&bytes, &std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "grain".into()))
}

impl Preset {
    pub fn new(name: &str, brush: &Brush, tip: Option<&Tip>, grain: Option<&Tip>) -> Self {
        Preset { name: name.chars().take(200).collect(), brush: Brush { tip: None, grain_image: None, ..*brush }, tip: tip.cloned(), grain: grain.cloned() }
    }

    /// The file's bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let json = PresetJson {
            format: "tooncraft-brush".into(),
            version: VERSION,
            name: self.name.clone(),
            brush: self.brush,
            tip_name: self.tip.as_ref().map(|t| t.name.clone()),
            grain_name: self.grain.as_ref().map(|t| t.name.clone()),
        };
        let mut zip = craft_zip::Writer::new();
        let z = |e: craft_zip::Error| e.to_string();
        zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).map_err(z)?;
        zip.add("brush.json", &serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?, craft_zip::Compression::Deflated).map_err(z)?;
        if let Some(t) = &self.tip {
            // black ink with the tip's coverage as alpha
            let mut img = tooncraft_raster::RgbaImage::transparent(t.width, t.height);
            for (i, a) in t.alpha.iter().enumerate() {
                img.set_pixel(i as u32 % t.width, i as u32 / t.width, [0, 0, 0, *a]);
            }
            let png = tooncraft_format::encode_png(&img).map_err(|e| e.to_string())?;
            zip.add("tip.png", &png, craft_zip::Compression::Stored).map_err(z)?;
        }
        if let Some(g) = &self.grain {
            // an opaque grey image: the height as the brightness
            let mut img = tooncraft_raster::RgbaImage::transparent(g.width, g.height);
            for (i, v) in g.alpha.iter().enumerate() {
                img.set_pixel(i as u32 % g.width, i as u32 / g.width, [*v, *v, *v, 255]);
            }
            let png = tooncraft_format::encode_png(&img).map_err(|e| e.to_string())?;
            zip.add("grain.png", &png, craft_zip::Compression::Stored).map_err(z)?;
        }
        zip.finish().map_err(z)
    }

    /// Parse a preset file's bytes; every setting is sanitized.
    pub fn from_bytes(bytes: &[u8]) -> Result<Preset, String> {
        if craft_zip::stored_mimetype(bytes) != Some(MIMETYPE.as_bytes()) {
            return Err("not a ToonCraft brush preset".into());
        }
        let mut zip = craft_zip::Archive::open(bytes, craft_zip::Limits { max_entries: 8, max_total_bytes: 128 << 20 }).map_err(|e| e.to_string())?;
        let json = zip.read("brush.json", 1 << 20).map_err(|e| e.to_string())?.ok_or("brush.json is missing")?;
        let p: PresetJson = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
        if p.format != "tooncraft-brush" || p.version > VERSION {
            return Err("this brush preset needs a newer ToonCraft".into());
        }
        let tip = match zip.read("tip.png", 64 << 20).map_err(|e| e.to_string())? {
            Some(png) => Some(tip_from_bytes(&png, p.tip_name.as_deref().unwrap_or("tip"))?),
            None => None,
        };
        let grain = match zip.read("grain.png", 64 << 20).map_err(|e| e.to_string())? {
            Some(png) => Some(grain_from_bytes(&png, p.grain_name.as_deref().unwrap_or("grain"))?),
            None => None,
        };
        let mut brush = Brush { tip: None, grain_image: None, ..p.brush }.sanitized();
        if brush.grain_kind == GrainKind::Image && grain.is_none() {
            brush.grain_kind = GrainKind::Paper;
        }
        Ok(Preset { name: p.name.chars().take(200).collect(), brush, tip, grain })
    }

    /// Write the preset to `path` (atomically).
    pub fn save(&self, path: &Path) -> Result<(), String> {
        craft_atomic::atomic_write(path, &self.to_bytes()?).map_err(|e| e.to_string())
    }

    /// Read a preset file.
    pub fn load(path: &Path) -> Result<Preset, String> {
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if meta.len() > 64 << 20 {
            return Err("the preset file is too large".into());
        }
        Preset::from_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
    }
}

/// Write a preset file with a tip and no grain image.
pub fn save(path: &Path, name: &str, brush: &Brush, tip: Option<&Tip>) -> Result<(), String> {
    Preset::new(name, brush, tip, None).save(path)
}

/// Read a preset file: its name, brush (without image ids) and tip. See [`Preset::load`] for the
/// grain image too.
pub fn load(path: &Path) -> Result<(String, Brush, Option<Tip>), String> {
    let p = Preset::load(path)?;
    Ok((p.name, p.brush, p.tip))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tooncraft_brush::{BrushKind, Curve, Source, Target};

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("tooncraft-preset-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn preset_round_trip() {
        let dir = temp("rt");
        let path = dir.join("leaf.toonbrush");
        let brush = Brush { size: 33.0, grain: 0.4, ..Brush::preset(BrushKind::Chalk) };
        let tip = Tip::new("leaf", 3, 2, vec![0, 128, 255, 255, 128, 0]).unwrap();
        save(&path, "Leaf", &brush, Some(&tip)).unwrap();
        let (name, b, t) = load(&path).unwrap();
        assert_eq!(name, "Leaf");
        assert_eq!(b, brush);
        assert_eq!(t.unwrap().alpha, tip.alpha);
        std::fs::write(&path, b"junk").unwrap();
        assert!(load(&path).is_err());
    }

    #[test]
    fn every_new_setting_survives_a_round_trip() {
        let mut brush =
            Brush { roundness: 0.4, angle: -35.0, grain_kind: GrainKind::Image, grain: 0.7, grain_scale: 3.0, ..Brush::preset(BrushKind::Watercolour) };
        brush.wet.pickup = 0.6;
        brush.dynamics.response.set(Target::Size, Source::Tilt, -0.5);
        brush.dynamics.response.get_mut(Target::Size, Source::Tilt).curve = Curve::s_curve();
        brush.dynamics.response.set(Target::Scatter, Source::Random, 0.25);
        brush.dynamics.response.set(Target::Angle, Source::Rotation, 1.0);
        let grain = Tip::new("weave", 2, 2, vec![0, 255, 255, 0]).unwrap();
        let bytes = Preset::new("Wash", &brush, None, Some(&grain)).to_bytes().unwrap();
        let back = Preset::from_bytes(&bytes).unwrap();
        assert_eq!(back.brush, brush);
        assert_eq!(back.name, "Wash");
        assert_eq!(back.grain.map(|g| g.alpha), Some(grain.alpha));
        assert!(back.tip.is_none());
    }

    #[test]
    fn a_missing_grain_image_falls_back_to_paper() {
        let brush = Brush { grain_kind: GrainKind::Image, ..Brush::preset(BrushKind::Pencil) };
        let back = Preset::from_bytes(&Preset::new("x", &brush, None, None).to_bytes().unwrap()).unwrap();
        assert_eq!(back.brush.grain_kind, GrainKind::Paper);
    }

    /// A version-1 file, as the previous release wrote it, still opens.
    #[test]
    fn version_one_files_open() {
        let old = r#"{"format":"tooncraft-brush","version":1,"name":"Old Ink","brush":{"kind":"ink","size":7.0,"opacity":1.0,"flow":1.0,"hardness":1.0,"spacing":0.08,"grain":0.0,"grain_scale":2.0,
            "dynamics":{"pressure_size":1.0,"pressure_opacity":0.0,"tilt":0.0,"velocity_size":0.25,"min_size":0.08},
            "smoothing":{"average":0.5,"string":0.0,"catch_up":true},"color":[24,22,30,255]}}"#;
        let mut zip = craft_zip::Writer::new();
        zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
        zip.add("brush.json", old.as_bytes(), craft_zip::Compression::Deflated).unwrap();
        let p = Preset::from_bytes(&zip.finish().unwrap()).unwrap();
        assert_eq!((p.name.as_str(), p.brush.size, p.brush.kind), ("Old Ink", 7.0, BrushKind::Ink));
        assert_eq!((p.brush.dynamics.pressure_size(), p.brush.dynamics.velocity_size(), p.brush.dynamics.min_size), (1.0, 0.25, 0.08));
        assert_eq!((p.brush.roundness, p.brush.angle, p.brush.wet.pickup), (1.0, 0.0, 0.0));
    }

    #[test]
    fn newer_and_hostile_files_are_refused_with_a_message() {
        let mut zip = craft_zip::Writer::new();
        zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
        zip.add("brush.json", br#"{"format":"tooncraft-brush","version":99,"name":"x","brush":{}}"#, craft_zip::Compression::Deflated).unwrap();
        assert!(Preset::from_bytes(&zip.finish().unwrap()).unwrap_err().contains("newer"));
        // nonsense numbers are clamped, not trusted
        let mut zip = craft_zip::Writer::new();
        zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
        let wild = r#"{"format":"tooncraft-brush","version":2,"name":"x","brush":{"size":1e30,"roundness":-4,"angle":1e30,"wet":{"pickup":9},"dynamics":{"response":{"size":{"pressure":{"amount":1e9,"curve":[[0,1e9]]}}}}}}"#;
        zip.add("brush.json", wild.as_bytes(), craft_zip::Compression::Deflated).unwrap();
        let b = Preset::from_bytes(&zip.finish().unwrap()).unwrap().brush;
        assert!(b.size <= tooncraft_brush::MAX_SIZE && b.roundness >= 0.1 && b.angle.abs() <= 180.0 && b.wet.pickup <= 1.0);
        assert!(b.dynamics.pressure_size() <= 1.0);
        for cut in [0, 10, 100] {
            let bytes = Preset::new("x", &Brush::default(), None, None).to_bytes().unwrap();
            assert!(Preset::from_bytes(&bytes[..cut.min(bytes.len())]).is_err());
        }
    }
}
