//! CPU compositing of a frame: exports, thumbnails, the fill reference and tests. The app draws
//! the canvas on the GPU from cached tile textures instead; both follow the same stacking order:
//! paper, light table pins, onion skin, then the layers bottom to top.

use tooncraft_raster::{RgbaImage, premultiply};

use crate::Project;

/// What to include when compositing a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOptions {
    /// Fill with the paper colour first (off: transparent).
    pub paper: bool,
    /// Draw the onion skin around this layer (index), if enabled in the project.
    pub onion_layer: Option<usize>,
    /// Draw the light table pins, if enabled in the project.
    pub light_table: bool,
    /// Only this layer (fill reference "current layer").
    pub only_layer: Option<usize>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { paper: true, onion_layer: None, light_table: false, only_layer: None }
    }
}

/// Composite the whole canvas at `frame`.
pub fn render_frame(p: &Project, frame: u32, o: RenderOptions) -> RgbaImage {
    render_region(p, frame, (0, 0, p.width(), p.height()), o)
}

/// Composite the canvas rectangle `(x, y, w, h)` (clipped to the canvas) at `frame`.
pub fn render_region(p: &Project, frame: u32, rect: (u32, u32, u32, u32), o: RenderOptions) -> RgbaImage {
    let x = rect.0.min(p.width());
    let y = rect.1.min(p.height());
    let w = rect.2.min(p.width() - x);
    let h = rect.3.min(p.height() - y);
    let mut img = if o.paper {
        let c = p.paper.color;
        RgbaImage::filled(w, h, premultiply([c[0], c[1], c[2], 255]))
    } else {
        RgbaImage::transparent(w, h)
    };
    if o.light_table && p.light_table.enabled {
        for pin in &p.light_table.pins {
            let Some(li) = p.layer_index(pin.layer) else { continue };
            if let Some(s) = p.drawing_at(li, pin.frame).and_then(|d| p.drawing(d)) {
                s.composite_region(&mut img, x, y, p.light_table.opacity, Some(p.light_table.tint));
            }
        }
    }
    if let Some(cur) = o.onion_layer
        && p.onion.enabled
    {
        let layers: Vec<usize> = if p.onion.all_layers { (0..p.layers.len()).collect() } else { vec![cur] };
        for li in layers {
            if !p.layers.get(li).is_some_and(|l| l.visible) {
                continue;
            }
            // furthest first, so nearer steps sit on top
            let mut n = p.onion_drawings(li, frame);
            n.sort_by_key(|(_, off)| std::cmp::Reverse(off.unsigned_abs()));
            for (d, off) in n {
                let Some(s) = p.drawing(d) else { continue };
                let tint = if !p.onion.tinted {
                    None
                } else if off < 0 {
                    Some(p.onion.before_tint)
                } else {
                    Some(p.onion.after_tint)
                };
                s.composite_region(&mut img, x, y, p.onion.step_opacity(off.unsigned_abs()), tint);
            }
        }
    }
    for (li, layer) in p.layers.iter().enumerate() {
        let (visible, opacity) = p.effective(layer);
        if o.only_layer.is_some_and(|only| only != li) || (!visible && o.only_layer.is_none()) {
            continue;
        }
        if let Some(s) = layer.drawing_at(frame).and_then(|d| p.drawing(d)) {
            if layer.blend == tooncraft_raster::BlendMode::Normal || o.only_layer.is_some() {
                s.composite_region(&mut img, x, y, opacity, None);
            } else {
                let mut top = RgbaImage::transparent(w, h);
                s.composite_region(&mut top, x, y, opacity, None);
                img.blend_image(&top, layer.blend);
            }
        }
    }
    img
}

/// The frame as the camera sees it (same size as the canvas). Without camera keys, or with the
/// camera off, this is [`render_frame`].
pub fn render_camera(p: &Project, frame: u32, o: RenderOptions) -> RgbaImage {
    let full = render_frame(p, frame, o);
    if !p.camera.enabled || p.camera.keys.is_empty() {
        return full;
    }
    let (w, h) = (p.width(), p.height());
    let map = p.camera.output_to_canvas(frame, w, h);
    let mut out = if o.paper {
        let c = p.paper.color;
        RgbaImage::filled(w, h, tooncraft_raster::premultiply([c[0], c[1], c[2], 255]))
    } else {
        RgbaImage::transparent(w, h)
    };
    for y in 0..h {
        for x in 0..w {
            let (cx, cy) = map(x as f32 + 0.5, y as f32 + 0.5);
            let s = tooncraft_raster::sample_bilinear(&full, cx, cy);
            if s[3] != 0 {
                let d = out.pixel(x, y);
                out.set_pixel(x, y, tooncraft_raster::over(d, s));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Cell;

    #[test]
    fn layers_and_onion_composite_in_order() {
        let mut p = Project::new("t", 8, 8, 24, 3).unwrap();
        let a = p.add_drawing(None).unwrap();
        let b = p.add_drawing(None).unwrap();
        p.drawing_mut(a).unwrap().tile_mut(0).unwrap().bytes_mut()[..4].copy_from_slice(&[0, 0, 0, 255]);
        p.drawing_mut(b).unwrap().tile_mut(0).unwrap().bytes_mut()[4..8].copy_from_slice(&[0, 0, 255, 255]);
        p.layers[0].cells = vec![Cell::Key(a), Cell::Key(b), Cell::Hold];
        let plain = render_frame(&p, 1, RenderOptions::default());
        assert_eq!(plain.pixel(1, 0), [0, 0, 255, 255]);
        assert_eq!(plain.pixel(0, 0), [246, 243, 236, 255]);
        let onion = render_frame(&p, 1, RenderOptions { onion_layer: Some(0), ..Default::default() });
        assert_ne!(onion.pixel(0, 0), plain.pixel(0, 0), "the previous drawing shows tinted");
        p.camera.enabled = true;
        p.camera.set(crate::CameraKey { frame: 0, x: 1.0, y: 0.0, zoom: 2.0, angle: 0.0 });
        let cam = render_camera(&p, 1, RenderOptions::default());
        let px = cam.pixel(4, 4);
        assert!(px[2] > 200 && px[0] < 150, "zoomed ×2 on the blue pixel at (1, 0): {px:?}");
        // multiply layer and a hidden folder
        p.add_layer("Shade", 1).unwrap();
        let shade = p.add_drawing(None).unwrap();
        p.drawing_mut(shade).unwrap().tile_mut(0).unwrap().bytes_mut()[8..12].copy_from_slice(&[128, 128, 128, 255]);
        p.layers[1].cells[1] = Cell::Key(shade);
        p.layers[1].blend = crate::BlendMode::Multiply;
        let m = render_frame(&p, 1, RenderOptions::default());
        assert_eq!(m.pixel(2, 0), [123, 122, 118, 255], "paper × 50 % grey");
        p.folders.push(crate::Folder { name: "F".into(), visible: false, opacity: 1.0, collapsed: false });
        p.layers[1].folder = Some("F".into());
        assert_eq!(render_frame(&p, 1, RenderOptions::default()).pixel(2, 0), [246, 243, 236, 255]);
        let clear = render_region(&p, 2, (1, 0, 4, 4), RenderOptions { paper: false, ..Default::default() });
        assert_eq!((clear.width, clear.pixel(0, 0)), (4, [0, 0, 255, 255]));
    }
}
