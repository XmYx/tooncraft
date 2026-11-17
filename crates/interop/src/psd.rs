//! Adobe Photoshop documents (`.psd`), through craft-psd (written from Adobe's public
//! "Photoshop File Formats Specification").
//!
//! **Export** ([`export_frame`]): one frame as an 8-bit RGB PSD. The paper is the bottom pixel
//! layer "Paper" (when the project exports its paper), then one pixel layer per ToonCraft layer
//! bottom to top (PSD stores the bottom layer first), each cropped to its drawn pixels and
//! carrying its name, opacity and visibility. The merged composite is the frame as ToonCraft
//! renders it, paper included, so viewers that only read the composite show the right picture.
//!
//! **Export of many frames** ([`export_frames`]): the same, but one layer group per frame ("Frame
//! 0001", …), each holding that frame's layers. Only the first group is visible, so the merged
//! composite and a viewer that ignores visibility show the first frame; switching groups on and
//! off steps through the animation. A shot too wide for a PSD (30000 px) or asked for as a PSB is
//! written as a PSB.
//!
//! **Import** ([`import`]): pixel layers of any colour mode and depth craft-psd decodes (RGB,
//! grayscale, CMYK, Lab at 8/16/32 bits) become premultiplied RGBA8 layers on the full canvas,
//! placed at their offsets. Group folders are flattened (their visibility and opacity carry
//! over to the layers inside); group markers, adjustment layers and fill layers are skipped.
//! Not carried over: blend modes, masks, clipping, layer effects and text (text layers import
//! as their stored pixels).

use craft_psd::{GroupSpec, LayerSpec, PixelData, PsdBuilder, PsdFile, SectionType, Version};
use tooncraft_doc::{Project, RenderOptions, render_frame};
use tooncraft_raster::{RgbaImage, premultiply};

use crate::common::{Budget, ImportedLayer, drawing_image, place, premultiply_in_place};
use crate::{InteropError, Result};

/// Most layers one import keeps.
pub const MAX_LAYERS: usize = tooncraft_doc::MAX_LAYERS;

fn encode_err(e: craft_psd::PsdError) -> InteropError {
    InteropError::Encode(e.to_string())
}

/// Bounding box `(x, y, w, h)` of the pixels with any alpha, `None` when all transparent.
fn bounds(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..img.height {
        for x in 0..img.width {
            if img.pixel(x, y)[3] != 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x0 < x1).then_some((x0, y0, x1 - x0, y1 - y0))
}

/// Straight RGBA8 of the rectangle `(x, y, w, h)` of a premultiplied image.
fn crop_straight(img: &RgbaImage, (x, y, w, h): (u32, u32, u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for yy in y..y + h {
        for xx in x..x + w {
            out.extend_from_slice(&tooncraft_raster::unpremultiply(img.pixel(xx, yy)));
        }
    }
    out
}

/// Encode `frame` of the shot as a layered PSD.
pub fn export_frame(p: &Project, frame: u32) -> Result<Vec<u8>> {
    if frame >= p.length() {
        return Err(InteropError::Range(frame, frame.saturating_add(1)));
    }
    let (w, h) = (p.width(), p.height());
    let mut b = PsdBuilder::new(w, h).resolution(72.0);
    if p.paper.export {
        let c = p.paper.color;
        let paper = RgbaImage::filled(w, h, premultiply([c[0], c[1], c[2], 255]));
        b.push_layer(LayerSpec::new("Paper", 0, 0, w, h, PixelData::Rgba8(paper.to_straight())));
    }
    for (i, layer) in p.layers.iter().enumerate() {
        let img = drawing_image(p, i, frame);
        let rect = img.as_ref().and_then(bounds);
        let mut spec = match (img, rect) {
            (Some(img), Some(r)) => LayerSpec::new(layer.name.clone(), r.0 as i32, r.1 as i32, r.2, r.3, PixelData::Rgba8(crop_straight(&img, r))),
            // An empty layer still keeps its place, name and settings.
            _ => LayerSpec::new(layer.name.clone(), 0, 0, 0, 0, PixelData::Rgba8(Vec::new())),
        };
        spec.opacity = (layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        spec.visible = layer.visible;
        b.push_layer(spec);
    }
    let merged = render_frame(p, frame, RenderOptions { paper: true, ..Default::default() });
    b.composite(PixelData::Rgba8(merged.to_straight()));
    b.to_bytes().map_err(encode_err)
}

/// Most frames one multi-frame file holds (each frame is a group of layers).
pub const MAX_FRAMES_PER_FILE: u32 = 2000;
/// Most layer records one multi-frame file holds.
pub const MAX_RECORDS: usize = 20_000;
/// Most pixel bytes the layers of one multi-frame file hold while it is assembled.
const MAX_PIXEL_BYTES: u64 = 2 << 30;

/// Encode the frames `start..end` of the shot as one layered file with a layer group per frame.
/// `large` writes a PSB (64-bit lengths); a canvas over 30000 px does it anyway.
pub fn export_frames(p: &Project, start: u32, end: u32, large: bool) -> Result<Vec<u8>> {
    if start >= end || end > p.length() {
        return Err(InteropError::Range(start, end));
    }
    if end - start > MAX_FRAMES_PER_FILE {
        return Err(InteropError::Limit(format!("{} frames in one file (at most {MAX_FRAMES_PER_FILE}): export a shorter range", end - start)));
    }
    let records = (end - start) as usize * (p.layers.len() + 2) + 2;
    if records > MAX_RECORDS {
        return Err(InteropError::Limit(format!("{records} layer records (at most {MAX_RECORDS}): export a shorter range")));
    }
    let (w, h) = (p.width(), p.height());
    let mut b = PsdBuilder::new(w, h).resolution(72.0);
    if large || w > 30_000 || h > 30_000 {
        b = b.version(Version::Psb);
    }
    if p.paper.export {
        let c = p.paper.color;
        let paper = RgbaImage::filled(w, h, premultiply([c[0], c[1], c[2], 255]));
        b.push_layer(LayerSpec::new("Paper", 0, 0, w, h, PixelData::Rgba8(paper.to_straight())));
    }
    let mut bytes = 0u64;
    for f in start..end {
        let mut group = GroupSpec::new(format!("Frame {:04}", f + 1));
        group.visible = f == start;
        group.open = false;
        b.begin_group(group);
        for (i, layer) in p.layers.iter().enumerate() {
            let img = drawing_image(p, i, f);
            let rect = img.as_ref().and_then(bounds);
            let mut spec = match (img, rect) {
                (Some(img), Some(r)) => {
                    bytes += r.2 as u64 * r.3 as u64 * 4;
                    LayerSpec::new(layer.name.clone(), r.0 as i32, r.1 as i32, r.2, r.3, PixelData::Rgba8(crop_straight(&img, r)))
                }
                _ => LayerSpec::new(layer.name.clone(), 0, 0, 0, 0, PixelData::Rgba8(Vec::new())),
            };
            if bytes > MAX_PIXEL_BYTES {
                return Err(InteropError::Limit("more pixel data than one file can be assembled from: export a shorter range".into()));
            }
            spec.opacity = (layer.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            spec.visible = layer.visible;
            b.push_layer(spec);
        }
        b.end_group().map_err(encode_err)?;
    }
    let merged = render_frame(p, start, RenderOptions { paper: true, ..Default::default() });
    b.composite(PixelData::Rgba8(merged.to_straight()));
    b.to_bytes().map_err(encode_err)
}

/// Read the pixel layers of a PSD or PSB (bottom to top), each on the full canvas. Canvases
/// wider or taller than `max_side` are refused before any pixels are decoded.
pub fn import(bytes: &[u8], max_side: u32) -> Result<Vec<ImportedLayer>> {
    let bad = |e: craft_psd::PsdError| InteropError::Decode(e.to_string());
    let file = PsdFile::from_bytes(bytes).map_err(bad)?;
    let (w, h) = (file.header.width, file.header.height);
    if w == 0 || h == 0 || w > max_side || h > max_side {
        return Err(InteropError::Limit(format!("a {w}×{h} canvas (at most {max_side} a side)")));
    }
    // Group visibility and opacity, walking top to bottom: a folder record sits above (after,
    // in file order) its contents and a bounding divider below them.
    let records = file.layers();
    let mut inherited = vec![(true, 1.0f32); records.len()];
    let mut stack: Vec<(bool, f32)> = Vec::new();
    for (i, r) in records.iter().enumerate().rev() {
        match r.section_type() {
            SectionType::OpenFolder | SectionType::ClosedFolder => {
                let (pv, po) = stack.last().copied().unwrap_or((true, 1.0));
                stack.push((pv && r.is_visible(), po * r.opacity as f32 / 255.0));
            }
            SectionType::BoundingDivider => {
                stack.pop();
            }
            _ => {
                if let Some(slot) = inherited.get_mut(i) {
                    *slot = stack.last().copied().unwrap_or((true, 1.0));
                }
            }
        }
    }
    let mut budget = Budget::default();
    let mut out = Vec::new();
    for layer in file.iter_layers() {
        let r = layer.record;
        let pixel_layer = matches!(r.section_type(), SectionType::Other | SectionType::Unknown(_))
            && r.adjustment().is_none()
            && r.solid_color().is_none()
            && !r.rect.is_empty()
            && r.channels.iter().any(|c| c.id >= 0);
        if !pixel_layer {
            continue;
        }
        if out.len() >= MAX_LAYERS {
            return Err(InteropError::Limit(format!("more than {MAX_LAYERS} layers")));
        }
        let (lw, lh) = (r.rect.width(), r.rect.height());
        if lw <= 0 || lh <= 0 || lw > 4 * max_side as i64 || lh > 4 * max_side as i64 {
            continue;
        }
        budget.take(w, h)?;
        budget.take(lw as u32, lh as u32)?;
        let mut px = layer.rgba8().map_err(bad)?;
        premultiply_in_place(&mut px.data);
        let mut image = RgbaImage::transparent(w, h);
        place(&mut image, &px.data, px.width, px.height, px.left as i64, px.top as i64);
        let (pv, po) = inherited.get(layer.index).copied().unwrap_or((true, 1.0));
        let opacity = (r.opacity as f32 / 255.0) * (r.fill_opacity() as f32 / 255.0) * po;
        out.push(ImportedLayer { name: layer.name(), visible: pv && r.is_visible(), opacity: opacity.clamp(0.0, 1.0), image });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::shot;
    use craft_psd::GroupSpec;
    use proptest::prelude::*;

    #[test]
    fn export_has_layers_and_composite() {
        let p = shot();
        let bytes = export_frame(&p, 2).unwrap();
        let file = PsdFile::from_bytes(&bytes).unwrap();
        assert_eq!((file.header.width, file.header.height), (40, 30));
        let names: Vec<String> = file.iter_layers().map(|l| l.name()).collect();
        assert_eq!(names, ["Paper", "Ink", "Colour"]);
        let ink = file.layer(1).unwrap();
        assert_eq!(ink.record.opacity, 128);
        assert!(ink.record.is_visible());
        assert!(!file.layer(2).unwrap().record.is_visible());
        let px = ink.rgba8().unwrap();
        assert_eq!((px.left, px.top, px.width, px.height), (20, 10, 5, 5), "cropped to the drawing");
        let merged = file.composite_rgba8().unwrap();
        let expect = render_frame(&p, 2, RenderOptions::default());
        assert_eq!(merged.data, expect.to_straight());
        assert!(matches!(export_frame(&p, 6), Err(InteropError::Range(6, 7))));
    }

    #[test]
    fn many_frames_make_one_group_per_frame() {
        let p = shot();
        let bytes = export_frames(&p, 0, 6, false).unwrap();
        assert_eq!(&bytes[4..6], &[0, 1], "a PSD");
        let file = PsdFile::from_bytes(&bytes).unwrap();
        let folders: Vec<_> = file.layers().iter().filter(|r| matches!(r.section_type(), SectionType::OpenFolder | SectionType::ClosedFolder)).collect();
        assert_eq!(folders.len(), 6, "one group per frame");
        let visible: Vec<bool> = folders.iter().map(|r| r.is_visible()).collect();
        assert_eq!(visible, [true, false, false, false, false, false], "only the first frame shows");
        // the merged composite is the first frame
        assert_eq!(file.composite_rgba8().unwrap().data, render_frame(&p, 0, RenderOptions::default()).to_straight());
        // the layers import frame by frame: Paper, then per frame the layers that have pixels
        let layers = import(&bytes, 1000).unwrap();
        assert_eq!(layers.len(), 1 + 5 + 6, "the blank Ink cell of frame 6 is skipped");
        let names: Vec<&str> = layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(&names[..4], ["Paper", "Ink", "Colour", "Ink"]);
        assert!(layers[1].visible && !layers[3].visible, "later frames are hidden through their group");
        // frame 3 holds drawing B
        assert_eq!(layers[5].image, p.drawing(p.drawing_at(0, 2).unwrap()).unwrap().to_image());
        // a sub-range keeps its own numbering
        let some = export_frames(&p, 2, 4, false).unwrap();
        let file = PsdFile::from_bytes(&some).unwrap();
        assert_eq!(file.layers().iter().filter(|r| matches!(r.section_type(), SectionType::OpenFolder | SectionType::ClosedFolder)).count(), 2);
        let first = file.layers().iter().find(|r| matches!(r.section_type(), SectionType::OpenFolder | SectionType::ClosedFolder)).unwrap();
        assert_eq!(first.name(), "Frame 0003");
    }

    #[test]
    fn a_psb_and_the_limits() {
        let p = shot();
        let bytes = export_frames(&p, 0, 2, true).unwrap();
        assert_eq!(&bytes[4..6], &[0, 2], "a PSB");
        assert_eq!(import(&bytes, 1000).unwrap().len(), 1 + 4);
        assert!(matches!(export_frames(&p, 3, 3, false), Err(InteropError::Range(3, 3))));
        assert!(matches!(export_frames(&p, 0, 7, false), Err(InteropError::Range(0, 7))));
        let mut long = Project::new("long", 8, 8, 12, MAX_FRAMES_PER_FILE + 1).unwrap();
        long.add_layer("L", 1).unwrap();
        assert!(matches!(export_frames(&long, 0, MAX_FRAMES_PER_FILE + 1, false), Err(InteropError::Limit(_))));
    }

    #[test]
    fn round_trip() {
        let p = shot();
        let layers = import(&export_frame(&p, 0).unwrap(), 1000).unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[1].name, "Ink");
        assert!((layers[1].opacity - 128.0 / 255.0).abs() < 1e-4);
        assert!(!layers[2].visible);
        assert_eq!(layers[1].image, p.drawing(p.drawing_at(0, 0).unwrap()).unwrap().to_image());
        assert_eq!(layers[2].image.pixel(1, 1), [0, 0, 255, 255]);
        assert!(matches!(import(&export_frame(&p, 0).unwrap(), 39), Err(InteropError::Limit(_))));
    }

    #[test]
    fn empty_layer_exports() {
        let p = shot();
        let layers = import(&export_frame(&p, 5).unwrap(), 1000).unwrap();
        // the blank Ink layer has no pixels and is skipped on import
        assert_eq!(layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Paper", "Colour"]);
    }

    #[test]
    fn groups_and_offsets() {
        let mut b = PsdBuilder::new(4, 4);
        b.push_layer(LayerSpec::new("bottom", 2, 2, 4, 4, PixelData::Rgba8(vec![255; 64])));
        let mut g = GroupSpec::new("folder");
        g.visible = false;
        g.opacity = 51;
        b.begin_group(g);
        b.push_layer(LayerSpec::new("inside", -1, -1, 2, 2, PixelData::Rgba8([200, 0, 0, 255].repeat(4))));
        b.end_group().unwrap();
        b.composite(PixelData::Rgba8(vec![0; 64]));
        let layers = import(&b.to_bytes().unwrap(), 100).unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].image.pixel(1, 1), [0; 4]);
        assert_eq!(layers[0].image.pixel(3, 3), [255; 4]);
        assert!(layers[0].visible);
        assert_eq!(layers[1].name, "inside");
        assert!(!layers[1].visible);
        assert!((layers[1].opacity - 0.2).abs() < 1e-3);
        assert_eq!(layers[1].image.pixel(0, 0), [200, 0, 0, 255]);
        assert_eq!(layers[1].image.pixel(1, 0), [0; 4]);
    }

    #[test]
    fn truncated_files_fail() {
        let bytes = export_frame(&shot(), 0).unwrap();
        for cut in [0, 4, 26, 40, bytes.len() / 3, bytes.len() / 2, bytes.len() - 1] {
            let _ = import(&bytes[..cut], 1000); // must not panic
        }
        assert!(import(&bytes[..10], 1000).is_err());
        assert!(import(b"8BPS garbage", 1000).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn corrupt_bytes_never_panic(at in 0usize..4096, val in any::<u8>(), cut in 0usize..8192) {
            let mut bytes = export_frame(&shot(), 1).unwrap();
            let i = at % bytes.len();
            bytes[i] = val;
            bytes.truncate(cut.max(1).min(bytes.len()));
            let _ = import(&bytes, 64);
        }
    }
}
