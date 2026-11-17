//! More ways out: still-image sequences (WebP, TGA, TIFF) through craft-image-codecs, an
//! animated lossless WebP (craft-webp's container around craft-image-codecs' lossless frames),
//! and a multi-frame Photoshop document (PSD, or PSB for large canvases) with one layer per frame.
//!
//! Every frame is rendered through the camera, as the other exports are.

use std::path::Path;

use craft_image_codecs::{ChannelLayout, EncodeOptions, Format, Image, SampleType};
use craft_psd::{LayerSpec, PixelData, PsdBuilder, Version};
use tooncraft_doc::{Project, RenderOptions, render_camera};
use tooncraft_raster::RgbaImage;

use crate::{InteropError, Result};

/// A still-image format for sequences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Still {
    WebP,
    Tga,
    Tiff,
}

impl Still {
    pub fn extension(self) -> &'static str {
        match self {
            Still::WebP => "webp",
            Still::Tga => "tga",
            Still::Tiff => "tif",
        }
    }

    fn format(self) -> Format {
        match self {
            Still::WebP => Format::WebP,
            Still::Tga => Format::Tga,
            Still::Tiff => Format::Tiff,
        }
    }
}

/// Encode one premultiplied image as a still of `format` (straight alpha).
pub fn encode_still(img: &RgbaImage, format: Still) -> Result<Vec<u8>> {
    let image =
        Image::from_raw(img.width, img.height, ChannelLayout::Rgba, SampleType::U8, img.to_straight()).map_err(|e| InteropError::Encode(e.to_string()))?;
    craft_image_codecs::encode(&image, format.format(), &EncodeOptions::default()).map_err(|e| InteropError::Encode(e.to_string()))
}

fn check_range(p: &Project, start: u32, end: u32) -> Result<()> {
    if start >= end || end > p.length() {
        return Err(InteropError::Range(start, end));
    }
    Ok(())
}

fn frame(p: &Project, f: u32, paper: bool) -> RgbaImage {
    render_camera(p, f, RenderOptions { paper, ..Default::default() })
}

/// Write frames `start..end` as `frame_0001.<ext>` … into `dir`. Returns the files written.
pub fn write_sequence(p: &Project, dir: &Path, start: u32, end: u32, paper: bool, format: Still) -> Result<usize> {
    check_range(p, start, end)?;
    std::fs::create_dir_all(dir).map_err(|e| InteropError::Io(dir.display().to_string(), e))?;
    for f in start..end {
        let bytes = encode_still(&frame(p, f, paper), format)?;
        let path = dir.join(format!("frame_{:04}.{}", f + 1, format.extension()));
        craft_atomic::atomic_write(&path, &bytes).map_err(|e| InteropError::Io(path.display().to_string(), std::io::Error::other(e.to_string())))?;
    }
    Ok((end - start) as usize)
}

/// The `VP8L` bitstream inside a still lossless WebP file.
fn vp8l(webp: &[u8]) -> Result<&[u8]> {
    let bad = || InteropError::Encode("the WebP encoder did not write a lossless frame".into());
    if webp.len() < 12 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return Err(bad());
    }
    let mut at = 12usize;
    while at + 8 <= webp.len() {
        let id = &webp[at..at + 4];
        let size = u32::from_le_bytes([webp[at + 4], webp[at + 5], webp[at + 6], webp[at + 7]]) as usize;
        let body = webp.get(at + 8..at + 8 + size).ok_or_else(bad)?;
        if id == b"VP8L" {
            return Ok(body);
        }
        at += 8 + size + (size & 1);
    }
    Err(bad())
}

/// An animated, lossless WebP of frames `start..end` at the shot's frame rate; a frame that
/// repeats the one before it lengthens it instead of being stored again.
pub fn animated_webp(p: &Project, start: u32, end: u32, paper: bool) -> Result<Vec<u8>> {
    check_range(p, start, end)?;
    let (w, h) = (p.width(), p.height());
    let ms = |frames: u32| ((frames as u64 * 1000) / p.fps().max(1) as u64).clamp(1, (1 << 24) - 1) as u32;
    // (still bytes, frames shown)
    let mut stills: Vec<(Vec<u8>, u32)> = Vec::new();
    let mut last: Option<RgbaImage> = None;
    for f in start..end {
        let img = frame(p, f, paper);
        if last.as_ref() == Some(&img)
            && let Some(s) = stills.last_mut()
        {
            s.1 += 1;
            continue;
        }
        stills.push((encode_still(&img, Still::WebP)?, 1));
        last = Some(img);
    }
    let mut frames = Vec::with_capacity(stills.len());
    for (bytes, n) in &stills {
        frames.push(craft_webp::anim::AnimFrame {
            x: 0,
            y: 0,
            width: w,
            height: h,
            duration_ms: ms(*n),
            blend: false,
            dispose_to_background: false,
            payload: craft_webp::anim::Payload::Lossless { vp8l: vp8l(bytes)? },
        });
    }
    let opts = craft_webp::anim::AnimOptions { loop_count: 0, alpha: !paper, ..Default::default() };
    craft_webp::anim::animation(w, h, &frames, &opts).map_err(|e| InteropError::Encode(e.to_string()))
}

/// Canvases larger than this a side are written as PSB (the large document format).
pub const PSD_MAX_SIDE: u32 = 30_000;

/// A Photoshop document with one layer per frame of `start..end`, named `Frame 1` …, bottom to
/// top in time order; only the first is visible, and the merged picture is the first frame.
/// `psb` (or a canvas over [`PSD_MAX_SIDE`]) writes the large document format.
pub fn frames_psd(p: &Project, start: u32, end: u32, paper: bool, psb: bool) -> Result<Vec<u8>> {
    check_range(p, start, end)?;
    if end - start > tooncraft_doc::MAX_LAYERS as u32 * 4 {
        return Err(InteropError::Limit(format!("{} frames in one document", end - start)));
    }
    let (w, h) = (p.width(), p.height());
    let large = psb || w > PSD_MAX_SIDE || h > PSD_MAX_SIDE;
    let mut b = PsdBuilder::new(w, h).resolution(72.0);
    if large {
        b = b.version(Version::Psb);
    }
    let mut first = None;
    for f in start..end {
        let img = frame(p, f, paper);
        let mut spec = LayerSpec::new(format!("Frame {}", f + 1), 0, 0, w, h, PixelData::Rgba8(img.to_straight()));
        spec.visible = f == start;
        b.push_layer(spec);
        if first.is_none() {
            first = Some(img);
        }
    }
    if let Some(img) = first {
        b.composite(PixelData::Rgba8(img.to_straight()));
    }
    b.to_bytes().map_err(|e| InteropError::Encode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tooncraft_doc::Cell;

    fn shot() -> Project {
        let mut p = Project::new("t", 32, 16, 12, 3).unwrap();
        let a = p.add_drawing(None).unwrap();
        let b = p.add_drawing(None).unwrap();
        for (d, x) in [(a, 2usize), (b, 20)] {
            let t = p.drawing_mut(d).unwrap().tile_mut(0).unwrap();
            let o = (4 * 64 + x) * 4;
            t.bytes_mut()[o..o + 4].copy_from_slice(&[200, 0, 0, 255]);
        }
        p.layers[0].cells = vec![Cell::Key(a), Cell::Hold, Cell::Key(b)];
        p
    }

    #[test]
    fn still_sequences_decode_back() {
        let p = shot();
        for format in [Still::WebP, Still::Tga, Still::Tiff] {
            let img = render_camera(&p, 0, RenderOptions { paper: false, ..Default::default() });
            let bytes = encode_still(&img, format).unwrap();
            let back = crate::decode_png(&bytes, 64).unwrap();
            assert_eq!((back.width, back.height), (32, 16), "{format:?}");
            assert_eq!(back.pixel(2, 4), [200, 0, 0, 255], "{format:?}");
            assert_eq!(back.pixel(5, 5)[3], 0, "{format:?} keeps transparency");
        }
        let dir = std::env::temp_dir().join(format!("tooncraft-seq-{}", std::process::id()));
        assert_eq!(write_sequence(&p, &dir, 0, 3, true, Still::Tga).unwrap(), 3);
        assert!(dir.join("frame_0003.tga").is_file());
        assert!(write_sequence(&p, &dir, 2, 9, true, Still::Tga).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn animated_webp_merges_holds() {
        let p = shot();
        let bytes = animated_webp(&p, 0, 3, false).unwrap();
        let info = craft_webp::scan(&bytes, 16).unwrap();
        assert!(info.is_animation());
        assert_eq!(info.frames.len(), 2, "the held frame is one frame twice as long");
        assert_eq!(info.frames[0].duration_ms, 166);
        let frames = craft_image_codecs::decode_webp_frames(&bytes, &Default::default()).unwrap();
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn multi_frame_psd_has_a_layer_per_frame() {
        let p = shot();
        for psb in [false, true] {
            let bytes = frames_psd(&p, 0, 3, true, psb).unwrap();
            let file = craft_psd::PsdFile::from_bytes(&bytes).unwrap();
            assert_eq!(file.header.version == Version::Psb, psb);
            let names: Vec<String> = file.layers().iter().map(|l| l.name()).collect();
            assert_eq!(names, ["Frame 1", "Frame 2", "Frame 3"]);
        }
        assert!(frames_psd(&p, 3, 3, true, false).is_err());
    }
}
