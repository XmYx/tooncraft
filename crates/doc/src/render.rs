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
            // the layer's own onion settings, when it has them
            let lo = p.onion_for(li);
            if !lo.enabled {
                continue;
            }
            let t = map(li, frame);
            // furthest first, so nearer steps sit on top
            let mut n = p.onion_drawings(li, frame);
            n.sort_by_key(|(_, off)| std::cmp::Reverse(off.unsigned_abs()));
            for (d, off) in n {
                let Some(s) = p.drawing(d) else { continue };
                let tint = if !lo.tinted {
                    None
                } else if off < 0 {
                    Some(lo.before_tint)
                } else {
                    Some(lo.after_tint)
                };
                let outlined;
                let s = if lo.outline {
                    outlined = s.outlined();
                    &outlined
                } else {
                    s
                };
                draw(s, &mut img, (x, y), t.as_ref(), lo.offset_opacity(off), tint);
            }
        }
    }
    layers(p, frame, &mut img, (x, y), o, |li| Some(map(li, frame)));
    img
}

/// Composite the layers bottom to top; `map` gives each layer's transform (`None`: skip the
/// layer, `Some(None)`: drawn as is). Layers a matte clips are composited on their own and cut
/// by the matte; the matte itself is not drawn. A layer with clipping layers above it is
/// composited with them as a group (see [`crate::Layer::clip`]).
fn layers(p: &Project, frame: u32, img: &mut RgbaImage, origin: (u32, u32), o: RenderOptions, map: impl Fn(usize) -> Option<Option<Affine>>) {
    let only = o.only_layer;
    let owners = p.matte_owners();
    let (w, h) = (img.width, img.height);
    let mut clipped: Option<RgbaImage> = None;
    let n = p.layers.len();
    let mut li = 0;
    while li < n {
        let Some(layer) = p.layers.get(li) else { break };
        let (visible, opacity) = p.effective_at(layer, frame);
        if let Some(only) = only {
            if only == li {
                let mut out = RgbaImage::transparent(w, h);
                draw_layer(p, li, frame, &mut out, origin, o, BlendMode::Normal, opacity, &map);
                if let Some(m) = owners.get(li).copied().flatten() {
                    apply_matte(p, m, frame, &mut out, origin, o, &map);
                }
                img.over_image(&out);
            }
            li += 1;
            continue;
        }
        if layer.matte.is_some() {
            if let Some(mut c) = clipped.take() {
                apply_matte(p, li, frame, &mut c, origin, o, &map);
                img.over_image(&c);
            }
            li += 1;
            continue;
        }
        // the clipping layers resting on this one
        let mut end = li + 1;
        while p.layers.get(end).is_some_and(|l| l.clip && l.matte.is_none()) {
            end += 1;
        }
        if visible {
            let target = if owners.get(li).copied().flatten().is_some() { clipped.get_or_insert_with(|| RgbaImage::transparent(w, h)) } else { &mut *img };
            if end == li + 1 {
                draw_layer(p, li, frame, target, origin, o, layer.blend, opacity, &map);
            } else {
                // a clipping group: each clipping layer shows only where the base has paint, and
                // the base's opacity and blend mode carry the group
                let mut group = RgbaImage::transparent(w, h);
                draw_layer(p, li, frame, &mut group, origin, o, BlendMode::Normal, 1.0, &map);
                for ci in li + 1..end {
                    let Some(c) = p.layers.get(ci) else { continue };
                    let (cv, co) = p.effective_at(c, frame);
                    if !cv {
                        continue;
                    }
                    let mut top = RgbaImage::transparent(w, h);
                    draw_layer(p, ci, frame, &mut top, origin, o, BlendMode::Normal, co, &map);
                    group.atop_image(&top, c.blend);
                }
                target.composite(&group, layer.blend, opacity);
            }
        }
        li = end;
    }
    if let Some(c) = clipped {
        img.over_image(&c);
    }
}

/// Composite layer `li`'s drawing at `frame` into `dst` through its transform and effects.
fn draw_layer(
    p: &Project,
    li: usize,
    frame: u32,
    dst: &mut RgbaImage,
    origin: (u32, u32),
    o: RenderOptions,
    mode: BlendMode,
    opacity: f32,
    map: &impl Fn(usize) -> Option<Option<Affine>>,
) {
    let Some(layer) = p.layers.get(li) else { return };
    let drawing = layer.drawing_at(frame).and_then(|d| p.drawing(d));
    let Some(t) = map(li) else { return };
    let fx = o.effects && tooncraft_fx::is_active(&layer.effects);
    // a lens flare lights the picture even where its layer has no drawing
    if drawing.is_none() && !(fx && tooncraft_fx::generates(&layer.effects)) {
        return;
    }
    if fx {
        // the whole canvas here (see the top of `render_region`); the effects run on the drawn
        // part plus the stack's reach, which gives the same pixels for far less work
        let (w, h) = (dst.width, dst.height);
        let mut top = RgbaImage::transparent(w, h);
        if let Some(s) = drawing {
            draw(s, &mut top, origin, t.as_ref(), 1.0, None);
        }
        let (cx, cy, cx1, cy1) = match (tooncraft_fx::stack_reach(&layer.effects, frame), top.opaque_bounds()) {
            (Some(r), Some((x0, y0, x1, y1))) => (x0.saturating_sub(r), y0.saturating_sub(r), x1.saturating_add(r).min(w), y1.saturating_add(r).min(h)),
            (Some(_), None) => return,
            (None, _) => (0, 0, w, h),
        };
        let mut part = if (cx, cy, cx1, cy1) == (0, 0, w, h) { top } else { top.crop(cx, cy, cx1 - cx, cy1 - cy) };
        let below = tooncraft_fx::needs_below(&layer.effects).then(|| dst.crop(cx, cy, cx1 - cx, cy1 - cy));
        let sources = source_images(p, li, frame, origin, (w, h), (cx, cy, cx1 - cx, cy1 - cy), map);
        let inputs = tooncraft_fx::Inputs { below: below.as_ref(), layers: &sources };
        tooncraft_fx::apply_stack_with(&mut part, (origin.0 + cx, origin.1 + cy), &layer.effects, frame, inputs);
        dst.composite_at(&part, cx, cy, mode, opacity);
    } else if let Some(s) = drawing
        && mode == BlendMode::Normal
    {
        draw(s, dst, origin, t.as_ref(), opacity, None);
    } else if let Some(s) = drawing {
        let mut top = RgbaImage::transparent(dst.width, dst.height);
        draw(s, &mut top, origin, t.as_ref(), opacity, None);
        dst.blend_image(&top, mode);
    }
}

/// The drawings of the layers layer `li`'s effects read (displacement maps), each through its own
/// transform at `frame` and without its effects, cut to `part` (x, y, w, h) of a `size` canvas.
/// A layer that is gone, is `li` itself or shows nothing gives no image.
fn source_images(
    p: &Project,
    li: usize,
    frame: u32,
    origin: (u32, u32),
    size: (u32, u32),
    part: (u32, u32, u32, u32),
    map: &impl Fn(usize) -> Option<Option<Affine>>,
) -> Vec<(u64, RgbaImage)> {
    let Some(layer) = p.layers.get(li) else { return Vec::new() };
    tooncraft_fx::source_layers(&layer.effects)
        .into_iter()
        .filter_map(|id| {
            let si = p.layer_index(crate::LayerId(id)).filter(|si| *si != li)?;
            let s = p.layers.get(si)?.drawing_at(frame).and_then(|d| p.drawing(d))?;
            let t = map(si)?;
            let mut img = RgbaImage::transparent(size.0, size.1);
            draw(s, &mut img, origin, t.as_ref(), 1.0, None);
            Some((id, if part == (0, 0, size.0, size.1) { img } else { img.crop(part.0, part.1, part.2, part.3) }))
        })
        .collect()
}

/// Cut `img` by the matte layer `m`: keep what is inside its drawing (or outside it), scaled by
/// its opacity. A hidden matte cuts nothing.
fn apply_matte(p: &Project, m: usize, frame: u32, img: &mut RgbaImage, origin: (u32, u32), o: RenderOptions, map: &impl Fn(usize) -> Option<Option<Affine>>) {
    let Some(ml) = p.layers.get(m) else { return };
    let (visible, opacity) = p.effective_at(ml, frame);
    let Some(mode) = ml.matte else { return };
    if !visible {
        return;
    }
    let mut shape = RgbaImage::transparent(img.width, img.height);
    draw_layer(p, m, frame, &mut shape, origin, o, BlendMode::Normal, 1.0, map);
    let k = opacity.clamp(0.0, 1.0);
    for (d, s) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(shape.data.as_chunks::<4>().0.iter()) {
        let a = s[3] as f32 / 255.0;
        let keep = match mode {
            crate::Matte::Inside => 1.0 - k * (1.0 - a),
            crate::Matte::Outside => 1.0 - k * a,
        };
        if keep < 1.0 {
            *d = tooncraft_raster::scale_alpha(*d, (keep * 255.0).round() as u8);
        }
    }
}

/// Will compositing `frame` with these options run any effect?
fn uses_effects(p: &Project, frame: u32, o: RenderOptions) -> bool {
    let owner = o.only_layer.and_then(|only| p.matte_owners().get(only).copied().flatten());
    p.layers.iter().enumerate().any(|(li, l)| {
        let shown = match o.only_layer {
            Some(only) => only == li || owner == Some(li),
            None => p.effective(l).0,
        };
        shown && tooncraft_fx::is_active(&l.effects) && (l.drawing_at(frame).is_some() || tooncraft_fx::generates(&l.effects))
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
        p.folders.push(crate::Folder { visible: false, ..crate::Folder::new("F") });
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

    #[test]
    fn opacity_keys_and_mattes() {
        let mut p = Project::new("t", 8, 8, 24, 3).unwrap();
        let red = p.add_drawing(None).unwrap();
        for k in 0..8 {
            p.drawing_mut(red).unwrap().tile_mut(0).unwrap().bytes_mut()[k * 4..k * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        p.layers[0].cells = vec![Cell::Key(red), Cell::Hold, Cell::Hold];
        // opacity keys: 0 at frame 0, 1 at frame 2
        assert!(p.layers[0].set_opacity_key(0, 0.0) && p.layers[0].set_opacity_key(2, 1.0));
        let clear = RenderOptions { paper: false, ..Default::default() };
        assert_eq!(render_frame(&p, 0, clear).pixel(0, 0)[3], 0);
        assert_eq!(render_frame(&p, 1, clear).pixel(0, 0)[3], 128);
        assert_eq!(render_frame(&p, 2, clear).pixel(0, 0)[3], 255);
        p.layers[0].opacity_keys.clear();
        // a matte above with one pixel drawn: only that pixel of the red row stays
        p.add_layer("Matte", 1).unwrap();
        let dot = p.add_drawing(None).unwrap();
        p.drawing_mut(dot).unwrap().tile_mut(0).unwrap().bytes_mut()[12..16].copy_from_slice(&[0, 0, 0, 255]);
        p.layers[1].cells = vec![Cell::Key(dot), Cell::Hold, Cell::Hold];
        p.layers[1].matte = Some(crate::Matte::Inside);
        assert_eq!(p.matte_owners(), vec![Some(1), None]);
        let f = render_frame(&p, 0, clear);
        assert_eq!((f.pixel(3, 0), f.pixel(2, 0)[3]), ([255, 0, 0, 255], 0), "the matte is not drawn; it keeps only its shape");
        p.layers[1].matte = Some(crate::Matte::Outside);
        let f = render_frame(&p, 0, clear);
        assert_eq!((f.pixel(3, 0)[3], f.pixel(2, 0)), (0, [255, 0, 0, 255]));
        // per-layer exports of a clipped layer are clipped too; a hidden matte clips nothing
        let only = render_frame(&p, 0, RenderOptions { paper: false, only_layer: Some(0), ..Default::default() });
        assert_eq!(only.pixel(3, 0)[3], 0);
        p.layers[1].visible = false;
        assert_eq!(render_frame(&p, 0, clear).pixel(3, 0), [255, 0, 0, 255]);
        // a layer in another folder is not clipped
        p.layers[1].visible = true;
        p.layers[0].folder = Some("F".into());
        assert_eq!(p.matte_owners(), vec![None, None]);
    }

    #[test]
    fn clipping_layers_show_only_over_their_base() {
        let (mut p, d) = dot_project();
        // a full red layer clipped to the 4×4 black dot
        p.add_layer("Shade", 1).unwrap();
        let red = p.add_drawing(None).unwrap();
        {
            let s = p.drawing_mut(red).unwrap();
            let mut img = RgbaImage::filled(128, 64, [200, 0, 0, 255]);
            img.set_pixel(0, 0, [200, 0, 0, 255]);
            s.load_image(&img).unwrap();
        }
        p.layers[1].cells = vec![Cell::Key(red), Cell::Hold, Cell::Hold, Cell::Hold];
        let unclipped = render_frame(&p, 0, RenderOptions::default());
        assert_eq!(unclipped.pixel(40, 40), [200, 0, 0, 255]);
        p.layers[1].clip = true;
        let img = render_frame(&p, 0, RenderOptions::default());
        assert_eq!(img.pixel(12, 12), [200, 0, 0, 255], "red over the dot");
        assert_eq!(img.pixel(40, 40), [246, 243, 236, 255], "nothing outside the base");
        // the base's opacity carries the group; hiding the base hides the group
        p.layers[0].opacity = 0.5;
        let half = render_frame(&p, 0, RenderOptions::default()).pixel(12, 12);
        assert!(half[1] > 60 && half[0] > 200, "{half:?}");
        p.layers[0].visible = false;
        assert_eq!(render_frame(&p, 0, RenderOptions::default()).pixel(12, 12), [246, 243, 236, 255]);
        // one layer alone (the fill's "current layer") ignores clipping
        let only = render_frame(&p, 0, RenderOptions { only_layer: Some(1), paper: false, ..Default::default() });
        assert_eq!(only.pixel(40, 40), [200, 0, 0, 255]);
        let _ = d;
    }

    #[test]
    fn nested_folders_hide_fade_and_lock() {
        let (mut p, _) = dot_project();
        p.set_folders(vec![
            crate::Folder { parent: Some("Outer".into()), opacity: 0.5, ..crate::Folder::new("Inner") },
            crate::Folder { opacity: 0.5, ..crate::Folder::new("Outer") },
        ]);
        p.layers[0].folder = Some("Inner".into());
        let l = p.layers[0].clone();
        assert_eq!(p.effective(&l), (true, 0.25));
        assert_eq!(p.folder_depth("Inner"), 1);
        assert!(!p.is_locked(&l));
        p.folders[1].locked = true;
        assert!(p.is_locked(&l), "locked through the outer folder");
        p.folders[1].visible = false;
        assert!(!p.effective(&l).0);
        p.folders[1].collapsed = true;
        assert!(p.is_collapsed(&l));
        assert!(p.folder_would_loop("Outer", "Inner"));
        // loops and missing parents are repaired on load
        p.set_folders(vec![
            crate::Folder { parent: Some("B".into()), ..crate::Folder::new("A") },
            crate::Folder { parent: Some("A".into()), ..crate::Folder::new("B") },
            crate::Folder { parent: Some("Gone".into()), ..crate::Folder::new("C") },
            crate::Folder::new("A"),
        ]);
        assert_eq!(p.folders.len(), 3);
        assert!(p.folders.iter().all(|f| f.parent.as_ref().is_none_or(|q| !p.folder_would_loop(&f.name, q))));
        assert_eq!(p.folders[2].parent, None);
        assert_eq!(p.layers[0].folder, None, "its folder is gone");
        assert!(p.folder_chain(Some("A")).len() <= 2);
    }

    #[test]
    fn onion_outlines_leave_the_inside_clear() {
        let mut p = Project::new("t", 64, 64, 24, 2).unwrap();
        let a = p.add_drawing(None).unwrap();
        let b = p.add_drawing(None).unwrap();
        for y in 10..30u32 {
            for x in 10..30u32 {
                let o = ((y * 64 + x) * 4) as usize;
                p.drawing_mut(a).unwrap().tile_mut(0).unwrap().bytes_mut()[o..o + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        p.layers[0].cells = vec![Cell::Key(a), Cell::Key(b)];
        let o = RenderOptions { onion_layer: Some(0), paper: false, ..Default::default() };
        let filled = render_frame(&p, 1, o);
        assert!(filled.pixel(20, 20)[3] > 0);
        p.onion.outline = true;
        let lines = render_frame(&p, 1, o);
        assert_eq!(lines.pixel(20, 20)[3], 0, "inside is clear");
        assert!(lines.pixel(10, 20)[3] > 0, "the edge shows");
        p.onion.hide_previous = true;
        assert_eq!(render_frame(&p, 1, o).pixel(10, 20)[3], 0, "the previous step is hidden");
    }

    #[test]
    fn a_flare_lights_an_empty_layer_and_displacement_reads_another_layer() {
        let (mut p, _) = dot_project();
        let o = RenderOptions { paper: false, ..Default::default() };
        let li = p.add_layer("Light", 1).unwrap();
        let flare = tooncraft_fx::EffectKind::LensFlare {
            x: 100.0,
            y: 40.0,
            through_x: 64.0,
            through_y: 32.0,
            size: 6.0,
            intensity: 1.0,
            colour: [255, 255, 255],
            rays: 0,
            ghosts: 0,
        };
        p.layers[li].effects = vec![tooncraft_fx::Effect::new(flare)];
        assert!(p.layers[li].drawing_at(0).is_none(), "the light layer has no drawing");
        let lit = render_frame(&p, 0, o);
        assert!(lit.pixel(100, 40)[3] > 200, "the flare shows: {:?}", lit.pixel(100, 40));
        assert_eq!(lit.pixel(11, 11), [0, 0, 0, 255], "the dot below is untouched");
        p.layers[li].effects.clear();
        // a map layer whose red pushes the dot 6 px right; the map itself is hidden
        let map_layer = p.add_layer("Map", 1).unwrap();
        let m = p.add_drawing(None).unwrap();
        for y in 0..64u32 {
            for x in 0..64u32 {
                let o = ((y * 64 + x) * 4) as usize;
                p.drawing_mut(m).unwrap().tile_mut(0).unwrap().bytes_mut()[o..o + 4].copy_from_slice(&[255, 128, 0, 255]);
            }
        }
        p.layers[map_layer].cells[0] = Cell::Key(m);
        p.layers[map_layer].visible = false;
        let id = p.layers[map_layer].id.0;
        p.layers[0].effects =
            vec![tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Displace { layer: id, amount_x: 6.0, amount_y: 0.0, brightness: false })];
        let moved = render_frame(&p, 0, o);
        assert_eq!(moved.pixel(11, 11)[3], 0, "the dot left its place");
        assert_eq!(moved.pixel(17, 11), [0, 0, 0, 255], "for 6 px to the right");
        // the layer pointing at itself, or a layer that is gone, moves nothing
        p.layers[0].effects =
            vec![tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Displace { layer: 9999, amount_x: 6.0, amount_y: 0.0, brightness: false })];
        assert_eq!(render_frame(&p, 0, o).pixel(11, 11), [0, 0, 0, 255]);
    }
}
