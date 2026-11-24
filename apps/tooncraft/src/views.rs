//! The second view and the navigator. The Second View panel shows the same shot a second time
//! with its own pan, zoom, rotation and mirror, in one of four looks:
//! - **Shot:** every layer, onion skin and light table, like the main canvas; draw in it too.
//! - **Drawing:** the current layer's drawing alone; draw in it too.
//! - **Camera:** the frame as the camera films it (multiplane depth, pegs, effects), read only.
//! - **Side:** the layers from the side, at their depth from the drawing plane, with the camera
//!   and what it sees; drag a layer (or use the arrow keys) to change its depth.
//!
//! The Navigator panel shows the whole picture with the main view's visible part outlined; click
//! or drag in it, or use the arrow keys, to move the main view there.

use std::sync::mpsc;

use egui::{Color32, Key, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::view::View;

/// What the second view shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    Shot,
    Drawing,
    Camera,
    Side,
}

impl Look {
    pub const ALL: [Look; 4] = [Look::Shot, Look::Drawing, Look::Camera, Look::Side];

    pub fn label(self) -> &'static str {
        match self {
            Look::Shot => tl!("Shot"),
            Look::Drawing => tl!("Drawing"),
            Look::Camera => tl!("Camera"),
            Look::Side => tl!("Side"),
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Look::Shot => tl!("Every layer, as on the canvas; draw here too"),
            Look::Drawing => tl!("The current layer's drawing alone; draw here too"),
            Look::Camera => tl!("The frame as the camera films it"),
            Look::Side => tl!("The layers from the side, at their depth; drag one to move it nearer or farther"),
        }
    }

    /// The launch name (`--second-view side`).
    pub fn id(self) -> &'static str {
        match self {
            Look::Shot => "shot",
            Look::Drawing => "drawing",
            Look::Camera => "camera",
            Look::Side => "side",
        }
    }

    pub fn from_id(s: &str) -> Option<Look> {
        Look::ALL.into_iter().find(|l| l.id() == s)
    }
}

/// The camera picture last rendered, and the one being rendered.
#[derive(Default)]
struct Filmed {
    /// (project changes, frame) of `texture`.
    key: Option<(u64, u32)>,
    texture: Option<egui::TextureHandle>,
    job: Option<((u64, u32), mpsc::Receiver<tooncraft_raster::RgbaImage>)>,
}

/// A depth drag in the side view: the layer, its depth when the drag began and now.
#[derive(Clone, Copy, Debug)]
struct DepthDrag {
    layer: usize,
    from: f32,
    to: f32,
}

pub struct SecondView {
    pub look: Look,
    pub view: View,
    fitted: bool,
    /// The view's rect last frame.
    pub rect: Rect,
    /// The navigator's picture rect last frame.
    pub nav_rect: Rect,
    filmed: Filmed,
    drag: Option<DepthDrag>,
}

impl Default for SecondView {
    fn default() -> Self {
        Self {
            look: Look::default(),
            view: View::default(),
            fitted: false,
            rect: Rect::NOTHING,
            nav_rect: Rect::NOTHING,
            filmed: Filmed::default(),
            drag: None,
        }
    }
}

impl SecondView {
    /// Has the camera look's picture been rendered for the shot as it is now?
    pub fn filmed_now(&self, changes: u64, frame: u32) -> bool {
        self.filmed.key == Some((changes, frame)) && self.filmed.texture.is_some()
    }
}

pub fn view_id() -> egui::Id {
    egui::Id::unique("tooncraft-second-view")
}

pub fn camera_id() -> egui::Id {
    egui::Id::unique("tooncraft-camera-look")
}

pub fn side_id() -> egui::Id {
    egui::Id::unique("tooncraft-side-view")
}

pub fn navigator_id() -> egui::Id {
    egui::Id::unique("tooncraft-navigator")
}

/// Is `id` one of the views a pen draws in (the canvas or the second view)?
pub fn is_drawing_view(id: egui::Id) -> bool {
    id == crate::canvas::canvas_id() || id == view_id()
}

/// Is `id` a view that takes its own keys (+, −, arrows) while it has the focus: the camera
/// look, the side look and the navigator? Plain single-key shortcuts that share those keys
/// (frame steps and holds) stay away from it.
pub fn owns_keys(id: egui::Id) -> bool {
    id == camera_id() || id == side_id() || id == navigator_id()
}

fn canvas_size(app: &ToonApp) -> egui::Vec2 {
    vec2(app.engine.project.width() as f32, app.engine.project.height() as f32)
}

/// The Second View panel.
pub fn panel(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        for look in Look::ALL {
            let on = app.view2.look == look;
            let r = ui.selectable_label(on, look.label()).on_hover_text(look.hint());
            let name = tlf!("{look} look", look = look.label());
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name.clone()));
            if r.clicked() {
                app.view2.look = look;
            }
        }
        if matches!(app.view2.look, Look::Shot | Look::Drawing | Look::Camera) {
            ui.separator();
            if ui.button(tl!("Fit")).on_hover_text(tl!("The whole picture in the second view")).clicked() {
                app.view2.fitted = false;
            }
            ui.weak(format!("{:.0} %", app.view2.view.zoom * 100.0));
        }
    });
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 8.0 || rect.height() < 8.0 {
        return;
    }
    app.view2.rect = rect;
    let size = canvas_size(app);
    if !app.view2.fitted {
        app.view2.view = View::fit(rect, size);
        app.view2.fitted = true;
    }
    match app.view2.look {
        Look::Shot | Look::Drawing => {
            // the canvas code, with this view's own pan and zoom in place of the main one's
            let only_current = app.view2.look == Look::Drawing;
            std::mem::swap(&mut app.view, &mut app.view2.view);
            crate::canvas::show_as(app, ui, view_id(), false, only_current);
            std::mem::swap(&mut app.view, &mut app.view2.view);
        }
        Look::Camera => camera(app, ui, rect),
        Look::Side => side(app, ui, rect),
    }
}

// ── the camera look ──

/// The camera's picture, rendered off the UI thread whenever the shot or the frame changes.
fn camera(app: &mut ToonApp, ui: &mut Ui, rect: Rect) {
    let resp = ui.interact(rect, camera_id(), Sense::click_and_drag());
    let size = canvas_size(app);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, tl!("Camera look")));
    if resp.clicked() {
        resp.request_focus();
    }
    pan_zoom(&mut app.view2.view, ui, rect, size, &resp);
    let key = (app.engine.changes(), app.frame);
    let f = &mut app.view2.filmed;
    // a finished render becomes the picture
    if let Some((k, rx)) = &f.job {
        match rx.try_recv() {
            Ok(img) => {
                let image = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.data);
                f.texture = Some(ui.ctx().load_texture("tooncraft-camera-look", image, egui::TextureOptions::LINEAR));
                f.key = Some(*k);
                f.job = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => f.job = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    // one render at a time; a stroke in progress waits until it is done
    if f.key != Some(key) && f.job.is_none() && !app.engine.is_stroking() {
        let project = app.engine.project.clone();
        let (tx, rx) = mpsc::channel();
        let frame = app.frame;
        let spawned = std::thread::Builder::new().name("tooncraft-camera-look".into()).spawn(move || {
            let img = tooncraft_doc::render::render_camera(&project, frame, tooncraft_doc::render::RenderOptions::default());
            let _ = tx.send(img);
        });
        match spawned {
            Ok(_) => f.job = Some((key, rx)),
            Err(e) => log::warn!("camera look: {e}"),
        }
    }
    // look again shortly for the render; no busy repaint while it runs
    if app.view2.filmed.job.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(15));
    }
    let pal = crate::theme::palette(app.light_theme);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, pal.workspace);
    let v = app.view2.view;
    let corners = [pos2(0.0, 0.0), pos2(size.x, 0.0), pos2(size.x, size.y), pos2(0.0, size.y)].map(|p| v.to_screen(rect, size, p));
    match &app.view2.filmed.texture {
        Some(t) => {
            let mut mesh = egui::Mesh::with_texture(t.id());
            let uvs = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
            for (p, uv) in corners.iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv, color: Color32::WHITE });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(egui::Shape::mesh(mesh));
        }
        None => {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, tl!("Filming…"), egui::FontId::proportional(13.0), pal.weak);
        }
    }
    painter.add(egui::Shape::closed_line(corners.to_vec(), Stroke::new(2.0, pal.accent)));
    if resp.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
}

/// Wheel zoom about the pointer, middle or Space drag to pan, and the keys +, − and arrows
/// while the view has the focus.
fn pan_zoom(v: &mut View, ui: &Ui, rect: Rect, size: egui::Vec2, resp: &egui::Response) {
    let (hover, scroll, space) = ui.input(|i| (i.pointer.hover_pos(), i.smooth_scroll_delta, i.key_down(Key::Space)));
    if let Some(h) = hover.filter(|h| rect.contains(*h))
        && scroll.y != 0.0
    {
        v.zoom_at(rect, size, h, (scroll.y * 0.0018).exp());
    }
    if resp.dragged_by(egui::PointerButton::Middle) || (space && resp.dragged_by(egui::PointerButton::Primary)) {
        v.pan += resp.drag_delta();
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        let (zin, zout, d) = ui.input(|i| {
            let mut d = egui::Vec2::ZERO;
            for (k, s) in
                [(Key::ArrowLeft, vec2(40.0, 0.0)), (Key::ArrowRight, vec2(-40.0, 0.0)), (Key::ArrowUp, vec2(0.0, 40.0)), (Key::ArrowDown, vec2(0.0, -40.0))]
            {
                if i.key_pressed(k) {
                    d += s;
                }
            }
            (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals), i.key_pressed(Key::Minus), d)
        });
        v.pan += d;
        if zin {
            v.zoom_at(rect, size, rect.center(), 1.25);
        }
        if zout {
            v.zoom_at(rect, size, rect.center(), 0.8);
        }
    }
}

// ── the side look ──

/// Where the camera sits, in depth units (−1 at rest; it moves towards the plane as it zooms in).
fn camera_depth(app: &ToonApp) -> f32 {
    let p = &app.engine.project;
    let k = p.camera.shot(app.frame, p.width(), p.height());
    let t = 1.0 - 1.0 / k.zoom.max(1e-3);
    t - 1.0
}

/// The depth range the side view spans: the camera, every layer, and the drawing plane.
fn depth_range(app: &ToonApp) -> (f32, f32) {
    let mut lo = camera_depth(app).min(0.0);
    let mut hi = 1.0f32;
    for l in &app.engine.project.layers {
        lo = lo.min(l.depth);
        hi = hi.max(l.depth);
    }
    let pad = ((hi - lo) * 0.08).max(0.1);
    (lo - pad, hi + pad)
}

/// Side view geometry: plane x and depth to screen, and back.
struct Side {
    rect: Rect,
    lo: f32,
    hi: f32,
    w: f32,
}

impl Side {
    fn new(app: &ToonApp, rect: Rect) -> Self {
        let (lo, hi) = depth_range(app);
        Side { rect: rect.shrink2(vec2(24.0, 18.0)), lo, hi, w: app.engine.project.width() as f32 }
    }

    /// The widest a layer at the farthest depth reaches, in plane pixels (a far layer is drawn
    /// larger so it keeps its size on screen).
    fn span(&self) -> f32 {
        (self.w * (1.0 + self.hi.max(0.0))).max(1.0)
    }

    fn x(&self, plane_x: f32) -> f32 {
        self.rect.center().x + (plane_x - self.w / 2.0) / self.span() * self.rect.width()
    }

    /// Depth to screen y: the camera at the bottom, far away at the top.
    fn y(&self, depth: f32) -> f32 {
        let t = (depth - self.lo) / (self.hi - self.lo).max(1e-3);
        self.rect.bottom() - t * self.rect.height()
    }

    fn depth(&self, y: f32) -> f32 {
        self.lo + (self.rect.bottom() - y) / self.rect.height().max(1.0) * (self.hi - self.lo)
    }

    /// The screen line of a layer at `depth`.
    fn line(&self, depth: f32) -> (Pos2, Pos2) {
        let half = self.w * (1.0 + depth) / 2.0;
        let y = self.y(depth);
        (pos2(self.x(self.w / 2.0 - half), y), pos2(self.x(self.w / 2.0 + half), y))
    }
}

/// The layer under the pointer in the side view (the nearest line within a few points).
fn layer_at(app: &ToonApp, s: &Side, p: Pos2) -> Option<usize> {
    app.engine
        .project
        .layers
        .iter()
        .enumerate()
        .filter_map(|(li, l)| {
            let (a, b) = s.line(l.depth);
            (p.x >= a.x - 6.0 && p.x <= b.x + 6.0).then(|| (li, (p.y - a.y).abs()))
        })
        .filter(|(_, d)| *d <= 6.0)
        // layers at the same depth share a line: the current one wins the tie
        .min_by(|a, b| a.1.total_cmp(&b.1).then((a.0 != app.layer).cmp(&(b.0 != app.layer))))
        .map(|(li, _)| li)
}

fn side(app: &mut ToonApp, ui: &mut Ui, rect: Rect) {
    let resp = ui.interact(rect, side_id(), Sense::click_and_drag());
    let name = app.engine.project.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
    let depth_now = app.engine.project.layers.get(app.layer).map(|l| l.depth).unwrap_or(0.0);
    let label = tlf!("Side look: {layer} at depth {depth}", layer = name, depth = format!("{depth_now:.2}"));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, label.clone()));
    let desc = tl!("Up and Down move the current layer farther or nearer; drag a layer to move it.");
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_description(desc.to_string()));
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    let s = Side::new(app, rect);
    // a drag moves a layer to another depth: one undo step when it ends; Escape cancels
    // the layer under the press, not under the pointer once it has moved far enough to drag
    if resp.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
        && let Some(li) = layer_at(app, &s, p)
    {
        let d = app.engine.project.layers[li].depth;
        app.view2.drag = Some(DepthDrag { layer: li, from: d, to: d });
        app.layer = li;
    }
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    if let Some(dr) = app.view2.drag.as_mut() {
        if escape {
            app.view2.drag = None;
        } else if resp.drag_stopped() {
            let DepthDrag { layer, from, to } = *dr;
            app.view2.drag = None;
            if (to - from).abs() > 1e-3 {
                app.run(Command::SetLayerDepth { layer, depth: to });
            }
        } else if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            dr.to = s.depth(p.y).clamp(tooncraft_doc::camera::MIN_DEPTH, tooncraft_doc::camera::MAX_DEPTH);
        }
    }
    if resp.has_focus() && app.view2.drag.is_none() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: false, vertical_arrows: true, escape: false }));
        let (up, down, shift) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown), i.modifiers.shift));
        let step = if shift { 0.5 } else { 0.1 };
        let d = if up {
            step
        } else if down {
            -step
        } else {
            0.0
        };
        if d != 0.0 && app.layer < app.engine.project.layers.len() {
            let layer = app.layer;
            let depth = (depth_now + d).clamp(tooncraft_doc::camera::MIN_DEPTH, tooncraft_doc::camera::MAX_DEPTH);
            app.run(Command::SetLayerDepth { layer, depth });
        }
    }
    // ----- drawing -----
    let pal = crate::theme::palette(app.light_theme);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, pal.workspace);
    let s = Side::new(app, rect);
    // the drawing plane
    let y0 = s.y(0.0);
    painter.extend(egui::Shape::dashed_line(&[pos2(s.rect.left(), y0), pos2(s.rect.right(), y0)], Stroke::new(1.0, pal.weak), 6.0, 4.0));
    painter.text(pos2(s.rect.left(), y0 - 2.0), egui::Align2::LEFT_BOTTOM, tl!("Drawing plane"), egui::FontId::proportional(11.0), pal.weak);
    // the camera and what it sees: rays from it through the edges of its frame on the plane
    let p = &app.engine.project;
    let k = p.camera.shot(app.frame, p.width(), p.height());
    let cam = pos2(s.x(k.x), s.y(camera_depth(app)));
    let half = p.width() as f32 / 2.0 / k.zoom.max(1e-3);
    let far = s.y(s.hi);
    for edge in [k.x - half, k.x + half] {
        let dir = pos2(s.x(edge), y0) - cam;
        let t = if dir.y.abs() > 1e-3 { ((far - cam.y) / dir.y).max(1.0) } else { 1.0 };
        painter.line_segment([cam, cam + dir * t], Stroke::new(1.0, pal.accent.gamma_multiply(0.6)));
    }
    painter.add(egui::Shape::convex_polygon(vec![cam + vec2(0.0, -7.0), cam + vec2(8.0, 7.0), cam + vec2(-8.0, 7.0)], pal.accent, Stroke::NONE));
    painter.text(cam + vec2(10.0, 0.0), egui::Align2::LEFT_CENTER, tl!("Camera"), egui::FontId::proportional(11.0), pal.accent);
    // the layers: the current one in the accent, a dragged one where it is going
    // labels of layers at the same height stack upwards instead of printing over each other
    let mut label_ys: Vec<f32> = Vec::new();
    for (li, l) in p.layers.iter().enumerate() {
        let depth = match app.view2.drag {
            Some(d) if d.layer == li => d.to,
            _ => l.depth,
        };
        let (a, b) = s.line(depth);
        let col = l.color.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(pal.text);
        let col = if l.visible { col } else { col.gamma_multiply(0.4) };
        let (width, line) = if li == app.layer { (3.0, pal.accent) } else { (1.5, col) };
        painter.line_segment([a, b], Stroke::new(width, line));
        let shared = label_ys.iter().filter(|y| (**y - b.y).abs() < 12.0).count();
        label_ys.push(b.y);
        let at = b + vec2(-2.0, -3.0 - 13.0 * shared.min(32) as f32);
        painter.text(at, egui::Align2::RIGHT_BOTTOM, format!("{} · {depth:.2}", l.name), egui::FontId::proportional(11.0), col);
    }
    if resp.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
}

// ── the navigator ──

/// Move the main view so canvas point `c` is in the middle of the canvas viewport.
pub fn centre_main_view_on(app: &mut ToonApp, c: Pos2) {
    let size = canvas_size(app);
    let vp = app.workspace.canvas_rect;
    let at = app.view.to_screen(vp, size, c);
    app.view.pan += vp.center() - at;
}

/// The canvas point in the middle of the main view.
pub fn main_view_centre(app: &ToonApp) -> Pos2 {
    let vp = app.workspace.canvas_rect;
    app.view.to_canvas(vp, canvas_size(app), vp.center())
}

/// The Navigator panel.
pub fn navigator(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        crate::menus::context(ui, app, &["view.fit", "view.actual_size"]);
    });
    let rect = ui.available_rect_before_wrap();
    if rect.width() < 8.0 || rect.height() < 8.0 {
        return;
    }
    app.view2.nav_rect = rect;
    let size = canvas_size(app);
    let nv = View::fit(rect, size);
    let resp = ui.interact(rect, navigator_id(), Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Canvas, true, tl!("Navigator")));
    let desc = tl!("Click or drag to move the view there; the arrow keys move it too.");
    ui.ctx().accesskit_node_builder(resp.id, |n| n.set_description(desc.to_string()));
    if resp.clicked() || resp.drag_started() {
        resp.request_focus();
    }
    if (resp.clicked() || resp.dragged())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let c = nv.to_canvas(rect, size, p);
        centre_main_view_on(app, c);
    }
    if resp.has_focus() {
        ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { tab: false, horizontal_arrows: true, vertical_arrows: true, escape: false }));
        let d = ui.input(|i| {
            let mut d = egui::Vec2::ZERO;
            for (k, s) in
                [(Key::ArrowLeft, vec2(-1.0, 0.0)), (Key::ArrowRight, vec2(1.0, 0.0)), (Key::ArrowUp, vec2(0.0, -1.0)), (Key::ArrowDown, vec2(0.0, 1.0))]
            {
                if i.key_pressed(k) {
                    d += s;
                }
            }
            d
        });
        if d != egui::Vec2::ZERO {
            // a tenth of the picture per press
            let mid = main_view_centre(app);
            centre_main_view_on(app, mid + d * size * 0.1);
        }
    }
    let pal = crate::theme::palette(app.light_theme);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, pal.workspace);
    crate::canvas::paint_picture(app, ui.ctx(), &painter, nv, rect);
    // what the main view shows
    let vp = app.workspace.canvas_rect;
    let shown: Vec<Pos2> = [vp.left_top(), vp.right_top(), vp.right_bottom(), vp.left_bottom()]
        .iter()
        .map(|p| nv.to_screen(rect, size, app.view.to_canvas(vp, size, *p)))
        .collect();
    painter.add(egui::Shape::closed_line(shown, Stroke::new(1.5, pal.accent)));
    if resp.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
    }
}

/// Where the navigator in `rect` draws canvas point `c` (for tests).
pub fn navigator_point(rect: Rect, size: egui::Vec2, c: Pos2) -> Pos2 {
    View::fit(rect, size).to_screen(rect, size, c)
}

/// The side look's screen point for `depth` at the middle of the picture, in `rect` (for tests).
pub fn side_point(app: &ToonApp, rect: Rect, depth: f32) -> Pos2 {
    let s = Side::new(app, rect);
    pos2(s.x(s.w / 2.0), s.y(depth))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_side_look_maps_depth_both_ways_and_keeps_every_layer_inside() {
        let ctx = egui::Context::default();
        let mut app = ToonApp::with_context(&ctx, crate::args::Cli::default()).without_autosave();
        app.engine.project.layers[0].depth = 3.0;
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 300.0));
        let s = Side::new(&app, rect);
        for d in [-0.9, 0.0, 0.5, 3.0] {
            let y = s.y(d);
            assert!(rect.contains(pos2(rect.center().x, y)), "{d} at {y}");
            assert!((s.depth(y) - d).abs() < 1e-3);
        }
        assert!(s.y(3.0) < s.y(0.0), "far is up");
        let (a, b) = s.line(3.0);
        assert!(a.x >= rect.left() && b.x <= rect.right(), "{a:?} {b:?}");
        assert!(camera_depth(&app) < 0.0, "the camera is in front of the plane");
        // a zero-width shot and an empty range give finite points, not NaN
        let s = Side { rect, lo: 0.0, hi: 0.0, w: 0.0 };
        assert!(s.y(0.5).is_finite() && s.x(0.0).is_finite() && s.depth(100.0).is_finite());
    }

    #[test]
    fn looks_have_their_own_launch_names() {
        for l in Look::ALL {
            assert_eq!(Look::from_id(l.id()), Some(l));
            assert!(!l.label().is_empty() && !l.hint().is_empty());
        }
        assert_eq!(Look::from_id("nope"), None);
    }
}
