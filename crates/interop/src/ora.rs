//! OpenRaster (`.ora`) import, through craft-ora (written from the public OpenRaster
//! specification, <https://www.openraster.org/>).
//!
//! An OpenRaster file is a ZIP with a `stack.xml` layer tree and one PNG per layer. [`import`]
//! returns the image layers bottom to top (`stack.xml` lists the top first), each PNG placed at
//! its `x`,`y` on the full canvas. Nested stacks are flattened: a stack's visibility and opacity
//! carry over to the layers inside. Layers without a picture or whose picture is missing are
//! skipped; a damaged picture is an error. Composite operations other than normal blending are
//! not carried over.

use craft_ora::{Archive, Node};
use tooncraft_raster::{MAX_SIDE, RgbaImage};

use crate::common::{Budget, ImportedLayer, decode_png, place};
use crate::{InteropError, Result};

/// Deepest stack nesting followed.
const MAX_DEPTH: usize = 64;

struct Found {
    name: String,
    visible: bool,
    opacity: f32,
    src: String,
    x: i32,
    y: i32,
}

fn opacity(o: f32) -> f32 {
    if o.is_finite() { o.clamp(0.0, 1.0) } else { 1.0 }
}

/// Image layers of `nodes`, top first, into `out`.
fn collect(nodes: &[Node], visible: bool, alpha: f32, depth: usize, out: &mut Vec<Found>) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(InteropError::Limit(format!("layer groups nested deeper than {MAX_DEPTH}")));
    }
    for n in nodes {
        match n {
            Node::Layer(l) => {
                let Some(src) = l.src.clone() else { continue };
                if out.len() >= tooncraft_doc::MAX_LAYERS {
                    return Err(InteropError::Limit(format!("more than {} layers", tooncraft_doc::MAX_LAYERS)));
                }
                out.push(Found {
                    name: l.props.name.clone().unwrap_or_else(|| format!("Layer {}", out.len() + 1)),
                    visible: visible && l.props.visible,
                    opacity: alpha * opacity(l.props.opacity),
                    src,
                    x: l.x,
                    y: l.y,
                });
            }
            Node::Stack(s) => collect(&s.children, visible && s.props.visible, alpha * opacity(s.props.opacity), depth + 1, out)?,
            // text, filters and other elements without a raster picture
            _ => {}
        }
    }
    Ok(())
}

/// Read an OpenRaster document's image layers (bottom to top) on its full canvas.
pub fn import(bytes: &[u8]) -> Result<Vec<ImportedLayer>> {
    let bad = |e: craft_ora::Error| InteropError::Decode(e.to_string());
    let mut archive = Archive::open(bytes).map_err(bad)?;
    let (w, h) = (archive.image().width, archive.image().height);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(InteropError::Limit(format!("a {w}×{h} canvas (at most {MAX_SIDE} a side)")));
    }
    let mut found = Vec::new();
    collect(&archive.image().stack.clone(), true, 1.0, 0, &mut found)?;
    let mut budget = Budget::default();
    let mut out = Vec::with_capacity(found.len());
    for f in found.into_iter().rev() {
        let Some(png) = archive.entry(&f.src, craft_ora::MAX_ENTRY).map_err(bad)? else { continue };
        let pic = decode_png(&png, MAX_SIDE)?;
        budget.take(w, h)?;
        budget.take(pic.width, pic.height)?;
        let mut image = RgbaImage::transparent(w, h);
        place(&mut image, &pic.data, pic.width, pic.height, f.x as i64, f.y as i64);
        out.push(ImportedLayer { name: f.name, visible: f.visible, opacity: f.opacity, image });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::encode_png;
    use craft_ora::{Image, Layer, Props, Stack, Writer};
    use proptest::prelude::*;

    fn sample() -> Vec<u8> {
        let mut w = Writer::new();
        w.add("data/a.png", &encode_png(&RgbaImage::filled(2, 2, [0, 0, 255, 255])).unwrap()).unwrap();
        w.add("data/b.png", &encode_png(&RgbaImage::filled(3, 1, [255, 0, 0, 255])).unwrap()).unwrap();
        let top = Node::Layer(Layer {
            props: Props { name: Some("Top".into()), visible: true, opacity: 0.5, ..Default::default() },
            x: 4,
            y: 3,
            src: Some("data/a.png".into()),
        });
        let inner = Node::Layer(Layer {
            props: Props { name: Some("Inner".into()), visible: true, opacity: 1.0, ..Default::default() },
            x: -1,
            y: 0,
            src: Some("data/b.png".into()),
        });
        let group = Node::Stack(Stack {
            props: Props { name: Some("G".into()), visible: false, opacity: 0.5, ..Default::default() },
            children: vec![inner],
            ..Default::default()
        });
        let image = Image { version: Some("0.0.6".into()), width: 5, height: 4, xres: 72.0, yres: 72.0, stack: vec![top, group] };
        w.add_stack(&image).unwrap();
        w.finish().unwrap()
    }

    #[test]
    fn reads_layers_bottom_to_top() {
        let layers = import(&sample()).unwrap();
        assert_eq!(layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Inner", "Top"]);
        let inner = &layers[0];
        assert!(!inner.visible);
        assert!((inner.opacity - 0.5).abs() < 1e-6);
        assert_eq!((inner.image.width, inner.image.height), (5, 4));
        assert_eq!(inner.image.pixel(0, 0), [255, 0, 0, 255]);
        assert_eq!(inner.image.pixel(1, 0), [255, 0, 0, 255]);
        assert_eq!(inner.image.pixel(2, 0), [0; 4]);
        let top = &layers[1];
        assert_eq!(top.image.pixel(4, 3), [0, 0, 255, 255]);
        assert_eq!(top.image.pixel(3, 3), [0; 4]);
        assert!(top.visible);
    }

    #[test]
    fn garbage_fails() {
        assert!(import(b"").is_err());
        assert!(import(b"PK\x03\x04 not really").is_err());
        let bytes = sample();
        for cut in [10, bytes.len() / 2, bytes.len() - 5] {
            assert!(import(&bytes[..cut]).is_err());
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn corrupt_bytes_never_panic(at in 0usize..4096, val in any::<u8>()) {
            let mut bytes = sample();
            let i = at % bytes.len();
            bytes[i] = val;
            let _ = import(&bytes);
        }
    }
}
