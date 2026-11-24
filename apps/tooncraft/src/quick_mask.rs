//! Quick mask: edit the selection by painting. Turning it on makes the selection a mask the
//! size of the canvas; brush strokes then paint into the mask (the eraser takes away) instead
//! of the drawing, and a red tint covers what is not selected. Turning it off traces the mask
//! back into the selection's outlines.

use egui::{Color32, Mesh, Pos2, pos2};
use tooncraft_raster::fill::Mask;

use crate::app::ToonApp;

/// Largest canvas (pixels) quick mask works on.
pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;
/// Longest side of the tint shown over the canvas (the mask is scaled down for it).
const PREVIEW_SIDE: u32 = 2048;

#[derive(Clone, Default)]
pub struct QuickMask {
    width: u32,
    height: u32,
    /// One byte per canvas pixel, 255 selected.
    data: Vec<u8>,
    /// The last point of the stroke being painted.
    last: Option<(f32, f32)>,
    tint: Option<egui::TextureHandle>,
    dirty: bool,
}

// by hand: the texture handle has no Debug
impl std::fmt::Debug for QuickMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuickMask").field("width", &self.width).field("height", &self.height).finish()
    }
}

impl QuickMask {
    /// Is canvas pixel (x, y) selected?
    pub fn get(&self, x: u32, y: u32) -> bool {
        x < self.width && y < self.height && self.data.get((y * self.width + x) as usize).is_some_and(|v| *v != 0)
    }

    /// Paint (or with `on` false, clear) a disc of `radius` at (cx, cy).
    fn disc(&mut self, cx: f32, cy: f32, radius: f32, on: bool) {
        let r = radius.clamp(0.5, 2000.0);
        let (x0, y0) = (((cx - r).floor().max(0.0)) as u32, ((cy - r).floor().max(0.0)) as u32);
        let (x1, y1) = (((cx + r).ceil().max(0.0) as u32).min(self.width), ((cy + r).ceil().max(0.0) as u32).min(self.height));
        for y in y0..y1 {
            for x in x0..x1 {
                if (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) <= r
                    && let Some(v) = self.data.get_mut((y * self.width + x) as usize)
                {
                    *v = if on { 255 } else { 0 };
                }
            }
        }
        self.dirty = true;
    }

    /// Paint a line of discs from the last point to (x, y).
    pub fn paint_to(&mut self, x: f32, y: f32, radius: f32, on: bool) {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        let from = self.last.unwrap_or((x, y));
        let step = (radius * 0.3).max(0.5);
        let len = (x - from.0).hypot(y - from.1);
        let n = ((len / step).ceil() as u32).clamp(1, 10_000);
        for k in 1..=n {
            let t = k as f32 / n as f32;
            self.disc(from.0 + (x - from.0) * t, from.1 + (y - from.1) * t, radius, on);
        }
        if self.last.is_none() {
            self.disc(x, y, radius, on);
        }
        self.last = Some((x, y));
    }

    pub fn end_stroke(&mut self) {
        self.last = None;
    }

    fn as_mask(&self) -> Mask {
        Mask { x: 0, y: 0, width: self.width, height: self.height, data: self.data.clone() }
    }
}

pub fn is_on(app: &ToonApp) -> bool {
    app.sel.quick.is_some()
}

/// Turn quick mask on (from the selection) or off (back to a selection).
pub fn toggle(app: &mut ToonApp) {
    if app.sel.quick.is_some() {
        leave(app);
    } else {
        enter(app);
    }
}

fn enter(app: &mut ToonApp) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    if w as u64 * h as u64 > MAX_PIXELS {
        app.set_status(tl!("The canvas is too large for quick mask"));
        return;
    }
    let mut data = vec![0u8; w as usize * h as usize];
    if let Some(m) = crate::selection_tools::mask(app) {
        for y in 0..m.height {
            for x in 0..m.width {
                let (cx, cy) = (m.x + x, m.y + y);
                if cx < w
                    && cy < h
                    && m.get(cx, cy)
                    && let Some(v) = data.get_mut((cy * w + cx) as usize)
                {
                    *v = 255;
                }
            }
        }
    }
    app.sel.quick = Some(QuickMask { width: w, height: h, data, last: None, tint: None, dirty: true });
    app.set_status(tl!("Quick mask: paint to select, erase to deselect; the red is not selected"));
}

fn leave(app: &mut ToonApp) {
    let Some(q) = app.sel.quick.take() else { return };
    let m = q.as_mask();
    if !m.data.iter().any(|v| *v != 0) {
        crate::selection_tools::set_rings(app, Vec::new());
        app.selection = None;
        app.set_status(tl!("Quick mask off: nothing selected"));
        return;
    }
    match tooncraft_raster::select::trace(&m) {
        Some(rings) => {
            crate::selection_tools::set_rings(app, rings);
            app.set_status(tl!("Quick mask off: the painted area is selected"));
        }
        None => app.set_status(tl!("The painted area is too detailed to make a selection from")),
    }
}

/// Canvas input while quick mask is on and a brush is chosen: paint into the mask.
pub fn input(app: &mut ToonApp, at: Option<(f32, f32)>, resp: &egui::Response) {
    let erase = app.tool == crate::app::Tool::Eraser;
    let radius = app.tool.brush().map(|k| app.brush_mut(k).size * 0.5).unwrap_or(8.0);
    let Some(q) = app.sel.quick.as_mut() else { return };
    if let Some((x, y)) = at
        && (resp.drag_started() || resp.dragged() || resp.clicked())
    {
        q.paint_to(x, y, radius, !erase);
    }
    if resp.drag_stopped() || resp.clicked() {
        q.end_stroke();
    }
}

/// The red tint over what is not selected.
pub fn overlay(painter: &egui::Painter, app: &mut ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2) {
    let ctx = painter.ctx().clone();
    let Some(q) = app.sel.quick.as_mut() else { return };
    if q.dirty || q.tint.is_none() {
        let s = q.width.max(q.height).div_ceil(PREVIEW_SIDE).max(1);
        let (pw, ph) = (q.width.div_ceil(s), q.height.div_ceil(s));
        let tint = Color32::from_rgba_unmultiplied(220, 30, 30, 110);
        let mut pixels = vec![Color32::TRANSPARENT; pw as usize * ph as usize];
        for y in 0..ph {
            for x in 0..pw {
                if !q.get(x * s, y * s)
                    && let Some(p) = pixels.get_mut((y * pw + x) as usize)
                {
                    *p = tint;
                }
            }
        }
        let img = egui::ColorImage { size: [pw as usize, ph as usize], source_size: egui::vec2(pw as f32, ph as f32), pixels };
        match &mut q.tint {
            Some(t) => t.set(img, egui::TextureOptions::NEAREST),
            None => q.tint = Some(ctx.load_texture("quick-mask", img, egui::TextureOptions::NEAREST)),
        }
        q.dirty = false;
    }
    let Some(t) = &q.tint else { return };
    let (w, h) = (q.width as f32, q.height as f32);
    let mut mesh = Mesh::with_texture(t.id());
    for (p, uv) in [((0.0, 0.0), (0.0, 0.0)), ((w, 0.0), (1.0, 0.0)), ((w, h), (1.0, 1.0)), ((0.0, h), (0.0, 1.0))] {
        mesh.vertices.push(egui::epaint::Vertex { pos: to_screen(p.0, p.1), uv: pos2(uv.0, uv.1), color: Color32::WHITE });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(mesh);
}
