//! Shared pieces: imported layers, bounded PNG decoding and file reading, safe file names.

use std::path::Path;

use tooncraft_doc::{Layer, Project};
use tooncraft_raster::{RgbaImage, premultiply};

use crate::{InteropError, Result};

/// Most bytes of decoded pixels one import keeps (all layers or drawings together).
pub const MAX_IMPORT_BYTES: u64 = 2 << 30;
/// Largest file an import reads.
pub const MAX_IMPORT_FILE: u64 = 512 << 20;

/// A layer read from another application's file, placed on that file's full canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedLayer {
    pub name: String,
    /// Shown (its own flag and every enclosing group's).
    pub visible: bool,
    /// 0…1 (its own opacity times every enclosing group's).
    pub opacity: f32,
    /// Premultiplied RGBA8, the size of the file's canvas.
    pub image: RgbaImage,
}

/// Running total of decoded bytes against [`MAX_IMPORT_BYTES`].
#[derive(Default)]
pub(crate) struct Budget(u64);

impl Budget {
    pub(crate) fn take(&mut self, w: u32, h: u32) -> Result<()> {
        let bytes = w as u64 * h as u64 * 4;
        self.0 = self.0.saturating_add(bytes);
        if self.0 > MAX_IMPORT_BYTES {
            return Err(InteropError::Limit(format!("more than {} MB of pixels", MAX_IMPORT_BYTES >> 20)));
        }
        Ok(())
    }
}

/// Straight RGBA8 → premultiplied, in place.
pub(crate) fn premultiply_in_place(data: &mut [u8]) {
    for p in data.as_chunks_mut::<4>().0.iter_mut() {
        *p = premultiply(*p);
    }
}

/// Decode a PNG (or any format craft-image-codecs reads) to premultiplied RGBA8, refusing
/// pictures wider or taller than `max_side` before any pixels are allocated.
pub fn decode_png(bytes: &[u8], max_side: u32) -> Result<RgbaImage> {
    let bad = |e: craft_image_codecs::CodecError| InteropError::Decode(e.to_string());
    let (w, h) = craft_image_codecs::probe_size(bytes).map_err(bad)?;
    if w == 0 || h == 0 || w > max_side || h > max_side {
        return Err(InteropError::Limit(format!("a {w}×{h} picture (at most {max_side} a side)")));
    }
    let limits = craft_image_codecs::Limits {
        max_width: max_side,
        max_height: max_side,
        max_pixels: max_side as u64 * max_side as u64,
        max_alloc: (max_side as u64 * max_side as u64 * 16).min(MAX_IMPORT_BYTES * 4),
    };
    let opts = craft_image_codecs::DecodeOptions { limits, ..Default::default() };
    let img = craft_image_codecs::decode_with(bytes, &opts).map_err(bad)?;
    let (w, h) = (img.width(), img.height());
    let mut data = img.to_rgba8();
    if w == 0 || h == 0 || data.len() != w as usize * h as usize * 4 {
        return Err(InteropError::Decode("the picture's pixel data has the wrong size".into()));
    }
    premultiply_in_place(&mut data);
    Ok(RgbaImage { width: w, height: h, data })
}

/// Read a whole file of at most [`MAX_IMPORT_FILE`] bytes.
pub(crate) fn read_file(path: &Path) -> Result<Vec<u8>> {
    let io = |e| InteropError::Io(path.display().to_string(), e);
    let len = std::fs::metadata(path).map_err(io)?.len();
    if len > MAX_IMPORT_FILE {
        return Err(InteropError::Limit(format!("{} is larger than {} MB", path.display(), MAX_IMPORT_FILE >> 20)));
    }
    std::fs::read(path).map_err(io)
}

pub(crate) fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    craft_atomic::atomic_write(path, bytes).map_err(|e| InteropError::Io(path.display().to_string(), e))
}

pub(crate) fn create_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| InteropError::Io(dir.display().to_string(), e))
}

pub(crate) fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    tooncraft_format::encode_png(img).map_err(|e| InteropError::Encode(e.to_string()))
}

/// `start..end` must be a non-empty range inside the shot.
pub(crate) fn check_range(p: &Project, start: u32, end: u32) -> Result<()> {
    if start >= end || end > p.length() {
        return Err(InteropError::Range(start, end));
    }
    Ok(())
}

/// A name usable as one file or folder name on every desktop system: path separators, control
/// and reserved characters become `_`, leading/trailing dots and spaces go, reserved Windows
/// device names get a `_` prefix, at most 100 characters. Never empty.
pub fn safe_file_name(name: &str) -> String {
    let mut s: String =
        name.chars().map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c }).take(100).collect();
    s = s.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
    if s.is_empty() {
        return "_".into();
    }
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes().get(3).is_some_and(u8::is_ascii_digit));
    if reserved { format!("_{s}") } else { s }
}

/// Track names for the project's layers (bottom to top): the layer names, made unique so each
/// one also gives a distinct folder ([`safe_file_name`], compared without case).
pub(crate) fn track_names(layers: &[Layer]) -> Vec<String> {
    let mut used = std::collections::BTreeSet::new();
    let mut out = Vec::with_capacity(layers.len());
    for l in layers {
        let base = if l.name.trim().is_empty() { "Layer".to_string() } else { l.name.clone() };
        let mut name = base.clone();
        let mut n = 2u32;
        while !used.insert(safe_file_name(&name).to_lowercase()) {
            name = format!("{base} {n}");
            n += 1;
        }
        out.push(name);
    }
    out
}

/// The drawing a layer shows at `frame` as an image at full opacity (`None` when blank).
pub(crate) fn drawing_image(p: &Project, layer: usize, frame: u32) -> Option<RgbaImage> {
    p.drawing_at(layer, frame).and_then(|d| p.drawing(d)).map(|s| s.to_image())
}

/// Copy premultiplied `src` (`sw`×`sh`) onto `dst` with its top-left at `(left, top)`, clipped.
pub(crate) fn place(dst: &mut RgbaImage, src: &[u8], sw: u32, sh: u32, left: i64, top: i64) {
    let (dw, dh) = (dst.width as i64, dst.height as i64);
    let x0 = left.max(0);
    let x1 = (left + sw as i64).min(dw);
    if x0 >= x1 {
        return;
    }
    for sy in 0..sh as i64 {
        let y = top + sy;
        if y < 0 || y >= dh {
            continue;
        }
        let s = ((sy * sw as i64 + (x0 - left)) * 4) as usize;
        let d = ((y * dw + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        if let (Some(from), Some(to)) = (src.get(s..s + n), dst.data.get_mut(d..d + n)) {
            to.copy_from_slice(from);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names() {
        assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(safe_file_name(""), "_");
        assert_eq!(safe_file_name(" .. "), "_");
        assert_eq!(safe_file_name("con.png"), "_con.png");
        assert_eq!(safe_file_name("COM1"), "_COM1");
        assert_eq!(safe_file_name("Line Art"), "Line Art");
        assert_eq!(safe_file_name("a\0b:c"), "a_b_c");
    }

    #[test]
    fn unique_track_names() {
        let mut p = Project::new("t", 8, 8, 24, 1).unwrap();
        p.add_layer("A/B", 1).unwrap();
        p.add_layer("a_b", 2).unwrap();
        p.add_layer("", 3).unwrap();
        let names = track_names(&p.layers);
        assert_eq!(names, ["Layer 1", "A/B", "a_b 2", "Layer"]);
    }

    #[test]
    fn place_clips() {
        let mut d = RgbaImage::transparent(4, 4);
        let src = vec![9u8; 3 * 3 * 4];
        place(&mut d, &src, 3, 3, -1, 2);
        assert_eq!(d.pixel(0, 2), [9; 4]);
        assert_eq!(d.pixel(1, 3), [9; 4]);
        assert_eq!(d.pixel(2, 3), [0; 4]);
        assert_eq!(d.pixel(0, 1), [0; 4]);
        place(&mut d, &src, 3, 3, i32::MIN as i64, i32::MAX as i64);
        place(&mut d, &src, 3, 3, 10, 0);
    }

    #[test]
    fn png_limits_and_garbage() {
        let img = RgbaImage::filled(5, 3, [10, 20, 30, 255]);
        let png = encode_png(&img).unwrap();
        assert_eq!(decode_png(&png, 16).unwrap(), img);
        assert!(matches!(decode_png(&png, 4), Err(InteropError::Limit(_))));
        assert!(decode_png(&png[..png.len() / 2], 16).is_err());
        assert!(decode_png(b"nonsense", 16).is_err());
    }
}
