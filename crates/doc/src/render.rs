//! CPU compositing of a frame: exports, thumbnails, the fill reference and tests. The app draws
//! the canvas on the GPU from cached tile textures instead; both follow the same stacking order:
//! paper, light table pins, onion skin, then the layers bottom to top. A layer with effects is
//! drawn on its own (through its transform), run through its effect stack (which may read the
//! composite under it), and then composited with its opacity and blend mode.

use tooncraft_raster::select::Affine;
use tooncraft_raster::{BlendMode, RgbaImage, Surface, premultiply};

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
    /// Only this layer (fill reference "current layer", per-layer exports).
    pub only_layer: Option<usize>,
    /// Run the layers' effect stacks (off: only the drawn pixels, as the fill bucket needs).
    pub effects: bool,
    /// A map applied to the whole canvas before compositing (canvas pixels → output pixels);
    /// the fill uses it to see the picture in a pegged layer's own space.
    pub space: Option<Affine>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { paper: true, onion_layer: None, light_table: false, only_layer: None, space: None, effects: true }
    }
}

/// Composite the whole canvas at `frame`.
pub fn render_frame(p: &Project, frame: u32, o: RenderOptions) -> RgbaImage {
    render_region(p, frame, (0, 0, p.width(), p.height()), o)
}

fn background(p: &Project, w: u32, h: u32, paper: bool) -> RgbaImage {
    if paper {
        let c = p.paper.color;
        RgbaImage::filled(w, h, premultiply([c[0], c[1], c[2], 255]))
    } else {
        RgbaImage::transparent(w, h)
    }
}

fn draw(s: &Surface, img: &mut RgbaImage, origin: (u32, u32), t: Option<&Affine>, opacity: f32, tint: Option<[u8; 3]>) {
    match t {
        None => s.composite_region(img, origin.0, origin.1, opacity, tint),
        Some(t) => s.composite_affine(img, origin, t, opacity, tint),
    }
}

/// `t`, or nothing when it is the identity (the fast, exact copy).
fn non_identity(t: Affine) -> Option<Affine> {
    if t.is_identity(1e-6) { None } else { Some(t) }
}

/// Composite the canvas rectangle `(x, y, w, h)` (clipped to the canvas) at `frame`.
pub fn render_region(p: &Project, frame: u32, rect: (u32, u32, u32, u32), o: RenderOptions) -> RgbaImage {
    let x = rect.0.min(p.width());
    let y = rect.1.min(p.height());
    let w = rect.2.min(p.width() - x);
    let h = rect.3.min(p.height() - y);
    // Effects reach across the canvas (blurs, shadows, zoom blur): render it all, then cut out.
    if o.effects && (x, y, w, h) != (0, 0, p.width(), p.height()) && uses_effects(p, frame, o) {
        return render_region(p, frame, (0, 0, p.width(), p.height()), o).crop(x, y, w, h);
    }
    let mut img = background(p, w, h, o.paper);
    let space = o.space.unwrap_or(Affine::IDENTITY);
    let map = |li: usize, f: u32| non_identity(p.layer_transform(li, f).then(&space));
    if o.light_table && p.light_table.enabled {
        for pin in &p.light_table.pins {
            let Some(li) = p.layer_index(pin.layer) else { continue };
            if let Some(s) = p.drawing_at(li, pin.frame).and_then(|d| p.drawing(d)) {
                draw(s, &mut img, (x, y), map(li, pin.frame).as_ref(), p.light_table.opacity, Some(p.light_table.tint));
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
            let t = map(li, frame);
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
                draw(s, &mut img, (x, y), t.as_ref(), p.onion.step_opacity(off.unsigned_abs()), tint);
            }
        }
    }
    layers(p, frame, &mut img, (x, y), o, |li| Some(map(li, frame)));
    img
}

/// Composite the layers bottom to top; `map` gives each layer's transform (`None`: skip the
/// layer, `Some(None)`: drawn as is).
fn layers(p: &Project, frame: u32, img: &mut RgbaImage, origin: (u32, u32), o: RenderOptions, map: impl Fn(usize) -> Option<Option<Affine>>) {
    let only = o.only_layer;
    for (li, layer) in p.layers.iter().enumerate() {
        let (visible, opacity) = p.effective(layer);
        if only.is_some_and(|only| only != li) || (!visible && only.is_none()) {
            continue;
        }
        let Some(s) = layer.drawing_at(frame).and_then(|d| p.drawing(d)) else { continue };
        let Some(t) = map(li) else { continue };
        let mode = if only.is_some() { BlendMode::Normal } else { layer.blend };
        if o.effects && tooncraft_fx::is_active(&layer.effects) {
            // the whole canvas here (see the top of `render_region`)
            let mut top = RgbaImage::transparent(img.width, img.height);
            draw(s, &mut top, origin, t.as_ref(), 1.0, None);
            let below = tooncraft_fx::needs_below(&layer.effects).then_some(&*img);
            tooncraft_fx::apply_stack(&mut top, &layer.effects, frame, below);
            img.composite(&top, mode, opacity);
        } else if mode == BlendMode::Normal {
            draw(s, img, origin, t.as_ref(), opacity, None);
        } else {
            let mut top = RgbaImage::transparent(img.width, img.height);
            draw(s, &mut top, origin, t.as_ref(), opacity, None);
            img.blend_image(&top, layer.blend);
        }
    }
}

/// Will compositing `frame` with these options run any effect?
fn uses_effects(p: &Project, frame: u32, o: RenderOptions) -> bool {
    p.layers.iter().enumerate().any(|(li, l)| {
        let shown = match o.only_layer {
            Some(only) => only == li,
            None => p.effective(l).0,
        };
        shown && tooncraft_fx::is_active(&l.effects) && l.drawing_at(frame).is_some()
    })
}

/// The frame as the camera sees it (same size as the canvas). Without camera keys or shakes, or
/// with the camera off, this is [`render_frame`]. Each layer is filmed at its own depth
/// (multiplane) through its peg chain; onion skin and light table are not filmed.
pub fn render_camera(p: &Project, frame: u32, o: RenderOptions) -> RgbaImage {
    if !p.camera.active() {
        return render_frame(p, frame, o);
    }
    let (w, h) = (p.width(), p.height());
    let mut out = background(p, w, h, o.paper);
    layers(p, frame, &mut out, (0, 0), o, |li| {
        let depth = p.layers.get(li).map(|l| l.depth).unwrap_or(0.0);
        let cam = p.camera.layer_map(frame, w, h, depth)?;
        Some(Some(p.layer_transform(li, frame).then(&cam)))
    });
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
        p.camera.keys.clear();
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

    fn dot_project() -> (Project, crate::DrawingId) {
        let mut p = Project::new("t", 128, 64, 24, 4).unwrap();
        let d = p.add_drawing(None).unwrap();
        let s = p.drawing_mut(d).unwrap();
        for y in 10..14u32 {
            for x in 10..14u32 {
                let o = ((y * 64 + x) * 4) as usize;
                s.tile_mut(0).unwrap().bytes_mut()[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        p.layers[0].cells = vec![Cell::Key(d), Cell::Hold, Cell::Hold, Cell::Hold];
        (p, d)
    }

    #[test]
    fn pegged_layers_render_through_their_keys() {
        let (mut p, _) = dot_project();
        let peg = p.add_peg("Body", None).unwrap();
        p.layers[0].peg = Some(peg);
        let k = |frame, x| crate::PegKey { x, ease: crate::Ease::Linear, ..crate::PegKey::rest(frame) };
        p.peg_mut(peg).unwrap().set_key(k(0, 0.0));
        p.peg_mut(peg).unwrap().set_key(k(2, 80.0));
        let ink = |img: &RgbaImage, x, y| img.pixel(x, y)[0] < 50;
        assert!(ink(&render_frame(&p, 0, RenderOptions::default()), 12, 12));
        let mid = render_frame(&p, 1, RenderOptions::default());
        assert!(ink(&mid, 52, 12) && !ink(&mid, 12, 12), "half way at frame 2");
        assert!(ink(&render_frame(&p, 3, RenderOptions::default()), 92, 12), "holds the last key");
        // the fill sees the layer in its own space through `space`
        let local = p.layer_transform(0, 3).inverse();
        let img = render_frame(&p, 3, RenderOptions { space: local, ..Default::default() });
        assert!(ink(&img, 12, 12));
        // a child peg rides on its parent
        let arm = p.add_peg("Arm", Some(peg)).unwrap();
        p.layers[0].peg = Some(arm);
        p.peg_mut(arm).unwrap().set_key(crate::PegKey { y: 30.0, ..crate::PegKey::rest(0) });
        assert!(ink(&render_frame(&p, 3, RenderOptions::default()), 92, 42));
        let path = p.motion_path(arm, 0, 3);
        assert_eq!(path.len(), 4);
        assert!((path[3].0 - path[0].0 - 80.0).abs() < 1e-3);
        // deleting the parent re-hangs the child at the top
        p.delete_peg(peg).unwrap();
        assert_eq!(p.peg(arm).unwrap().parent, None);
        assert!(ink(&render_frame(&p, 3, RenderOptions::default()), 12, 42));
    }

    #[test]
    fn camera_films_each_layer_at_its_depth() {
        let (mut p, d) = dot_project();
        p.add_layer("Far", 0).unwrap();
        p.layers[0].cells = vec![Cell::Key(d), Cell::Hold, Cell::Hold, Cell::Hold];
        p.layers[0].depth = 1.0;
        p.layers[0].opacity = 0.5;
        p.camera.enabled = true;
        p.camera.set(crate::CameraKey { frame: 0, x: 64.0, y: 32.0, zoom: 1.0, angle: 0.0 });
        p.camera.set(crate::CameraKey { frame: 3, x: 54.0, y: 32.0, zoom: 1.0, angle: 0.0 });
        let rest = render_camera(&p, 0, RenderOptions::default());
        assert!(rest.pixel(12, 12)[0] < 50, "both planes on top of each other at rest");
        let moved = render_camera(&p, 3, RenderOptions::default());
        assert!(moved.pixel(22, 12)[0] < 50, "the drawing plane slides 10 px");
        let far = moved.pixel(16, 12)[0];
        assert!(far > 50 && far < 200, "the far plane slides 5 px: {far}");
    }

    #[test]
    fn effects_render_everywhere_and_regions_match_the_frame() {
        let mut p = Project::new("t", 96, 64, 24, 2).unwrap();
        let d = p.add_drawing(None).unwrap();
        for i in 0..16u32 {
            let t = p.drawing_mut(d).unwrap().tile_mut(0).unwrap();
            let k = ((20 + i) * 64 + 20) as usize * 4;
            t.bytes_mut()[k..k + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        p.layers[0].cells = vec![Cell::Key(d), Cell::Hold];
        let plain = render_frame(&p, 0, RenderOptions::default());
        p.layers[0].effects = vec![
            tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::DropShadow { dx: 30.0, dy: 0.0, blur: 2.0, colour: [255, 0, 0], opacity: 1.0 }),
            tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::LightWrap { radius: 3.0, intensity: 1.0 }),
        ];
        let full = render_frame(&p, 0, RenderOptions::default());
        assert_ne!(full, plain);
        assert!(full.pixel(50, 28)[0] > full.pixel(50, 28)[1], "the red shadow shows 30 px to the right");
        // a region (eyedropper, tiles) is the same pixels as the whole frame
        let part = render_region(&p, 0, (40, 10, 30, 40), RenderOptions::default());
        assert_eq!(part, full.crop(40, 10, 30, 40));
        // the fill bucket sees the drawing, not its effects
        let raw = render_frame(&p, 0, RenderOptions { effects: false, ..Default::default() });
        assert_eq!(raw, plain);
        // per-layer exports carry the layer's effects; a disabled effect does nothing
        let only = render_frame(&p, 0, RenderOptions { paper: false, only_layer: Some(0), ..Default::default() });
        assert!(only.pixel(50, 28)[3] > 0);
        for e in &mut p.layers[0].effects {
            e.enabled = false;
        }
        assert_eq!(render_frame(&p, 0, RenderOptions::default()), plain);
    }
}
