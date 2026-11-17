//! Brush presets (`.toonbrush`): a ZIP with `brush.json` (every setting) and, for image tips,
//! `tip.png` — portable between shots and machines.

use std::path::Path;

use tooncraft_brush::{Brush, MAX_TIP_SIDE, Tip};

pub const EXTENSION: &str = "toonbrush";
const MIMETYPE: &str = "application/vnd.storyteller.tooncraft-brush";

#[derive(serde::Serialize, serde::Deserialize)]
struct PresetJson {
    format: String,
    version: u32,
    name: String,
    brush: Brush,
    #[serde(default)]
    tip_name: Option<String>,
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

/// Write a preset file.
pub fn save(path: &Path, name: &str, brush: &Brush, tip: Option<&Tip>) -> Result<(), String> {
    let json = PresetJson {
        format: "tooncraft-brush".into(),
        version: 1,
        name: name.into(),
        brush: Brush { tip: None, ..*brush },
        tip_name: tip.map(|t| t.name.clone()),
    };
    let mut zip = craft_zip::Writer::new();
    let z = |e: craft_zip::Error| e.to_string();
    zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).map_err(z)?;
    zip.add("brush.json", &serde_json::to_vec_pretty(&json).map_err(|e| e.to_string())?, craft_zip::Compression::Deflated).map_err(z)?;
    if let Some(t) = tip {
        // black ink with the tip's coverage as alpha
        let mut img = tooncraft_raster::RgbaImage::transparent(t.width, t.height);
        for (i, a) in t.alpha.iter().enumerate() {
            img.set_pixel(i as u32 % t.width, i as u32 / t.width, [0, 0, 0, *a]);
        }
        let png = tooncraft_format::encode_png(&img).map_err(|e| e.to_string())?;
        zip.add("tip.png", &png, craft_zip::Compression::Stored).map_err(z)?;
    }
    craft_atomic::atomic_write(path, &zip.finish().map_err(z)?).map_err(|e| e.to_string())
}

/// Read a preset file: its name, brush (without a tip id) and tip.
pub fn load(path: &Path) -> Result<(String, Brush, Option<Tip>), String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 64 << 20 {
        return Err("the preset file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if craft_zip::stored_mimetype(&bytes) != Some(MIMETYPE.as_bytes()) {
        return Err("not a ToonCraft brush preset".into());
    }
    let mut zip = craft_zip::Archive::open(&bytes, craft_zip::Limits { max_entries: 8, max_total_bytes: 128 << 20 }).map_err(|e| e.to_string())?;
    let json = zip.read("brush.json", 1 << 20).map_err(|e| e.to_string())?.ok_or("brush.json is missing")?;
    let p: PresetJson = serde_json::from_slice(&json).map_err(|e| e.to_string())?;
    if p.format != "tooncraft-brush" || p.version > 1 {
        return Err("this brush preset needs a newer ToonCraft".into());
    }
    let tip = match zip.read("tip.png", 64 << 20).map_err(|e| e.to_string())? {
        Some(png) => Some(tip_from_bytes(&png, p.tip_name.as_deref().unwrap_or("tip"))?),
        None => None,
    };
    Ok((p.name, Brush { tip: None, ..p.brush }.sanitized(), tip))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tooncraft_brush::BrushKind;

    #[test]
    fn preset_round_trip() {
        let dir = std::env::temp_dir().join(format!("tooncraft-preset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
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
}
