//! The camera's path on the canvas: a line through the camera centre at every frame, a square
//! per camera key and a cross at the current frame. With the Transform tool a key's square drags
//! to move that key, and the cross drags to key the camera at the current frame; the camera frame
//! follows while dragging, nothing changes until the drag ends (one undo step), and Escape
//! cancels.

use egui::{Color32, Pos2, Stroke, Ui, vec2};
use tooncraft_doc::{Camera, CameraKey};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Handles are picked this close (screen points).
const PICK: f32 = 8.0;
/// Most frames the path draws.
const MAX_PATH: u32 = 10_000;

/// A drag of one camera key: nothing changes in the project until it ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraDrag {
    /// The key being moved (or made, at the current frame).
    pub key: CameraKey,
    /// Canvas pixels where it began and where the pointer is now.
    pub from: (f32, f32),
    pub to: (f32, f32),
    /// The project's change count when the drag began.
    pub changes: u64,
}

impl CameraDrag {
    /// The key where the drag would put it.
    pub fn moved(&self) -> CameraKey {
        CameraKey { x: self.key.x + self.to.0 - self.from.0, y: self.key.y + self.to.1 - self.from.1, ..self.key }
    }
}

/// Whether the path shows: the camera frame is shown and the camera has keys.
pub fn visible(app: &ToonApp) -> bool {
    app.show_camera && !app.engine.project.camera.keys.is_empty() && !app.playback.playing
}

/// The camera with a drag's key in place (what the canvas shows while dragging).
pub fn previewed(app: &ToonApp) -> Option<Camera> {
    let d = app.camera_drag?;
    let p = &app.engine.project;
    let mut cam = p.camera.clone();
    cam.set(d.moved().sanitized(p.width(), p.height()));
    Some(cam)
}

/// The camera centre at each frame, `(frame, x, y)`, with a drag's key in place.
pub fn path(app: &ToonApp) -> Vec<(u32, f32, f32)> {
    let p = &app.engine.project;
    let (w, h) = (p.width(), p.height());
    let preview = previewed(app);
    let cam = preview.as_ref().unwrap_or(&p.camera);
    cam.sweep(p.length().min(MAX_PATH), w, h).into_iter().map(|k| (k.frame, k.x, k.y)).collect()
}

/// Once per UI frame: a drag ends without a trace when the tool changed or the project did
/// (an undo, an edit from elsewhere, another file), so no stale camera is shown outside undo.
pub fn settle(app: &mut ToonApp) {
    if let Some(d) = app.camera_drag
        && (app.tool != crate::app::Tool::Transform || app.engine.changes() != d.changes)
    {
        app.camera_drag = None;
    }
}

/// The handle under `at` (screen): the current frame's cross, or a key's square. Returns the key
/// a drag would move (for the cross with no key there, the camera at that frame).
fn grab(app: &ToonApp, at: Pos2, screen: &dyn Fn(f32, f32) -> Pos2) -> Option<CameraKey> {
    let p = &app.engine.project;
    let (w, h) = (p.width(), p.height());
    // the current frame first, so its key wins over a neighbour drawn under it
    let here = p.camera.at(app.frame, w, h);
    if screen(here.x, here.y).distance(at) <= PICK {
        let keyed = p.camera.keys.iter().find(|k| k.frame == app.frame).copied();
        return Some(keyed.unwrap_or(CameraKey { frame: app.frame, ..here }));
    }
    p.camera.keys.iter().map(|k| (screen(k.x, k.y).distance(at), *k)).filter(|(d, _)| *d <= PICK).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, k)| k)
}

/// Pointer input with the Transform tool. Returns whether a camera handle took it (then the peg
/// is left alone).
pub fn input(app: &mut ToonApp, ui: &Ui, resp: &egui::Response, canvas: &dyn Fn(Pos2) -> (f32, f32), screen: &dyn Fn(f32, f32) -> Pos2) -> bool {
    if app.camera_drag.is_none() {
        if !visible(app) || !resp.drag_started() {
            return false;
        }
        let Some(p) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos()) else { return false };
        let Some(key) = grab(app, p, screen) else { return false };
        let at = canvas(p);
        app.camera_drag = Some(CameraDrag { key, from: at, to: at, changes: app.engine.changes() });
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.camera_drag = None;
        app.set_status(tl!("Camera move cancelled"));
        return true;
    }
    if let Some(p) = resp.interact_pointer_pos()
        && let Some(d) = app.camera_drag.as_mut()
    {
        d.to = canvas(p);
    }
    if resp.drag_stopped() {
        if let Some(d) = app.camera_drag.take() {
            finish(app, &d);
        }
    } else if !resp.dragged() && !ui.input(|i| i.pointer.any_down()) {
        // let go where the canvas did not see it: drop the preview
        app.camera_drag = None;
    }
    true
}

/// Set the dragged key (one undo step), unless the pointer came back where it started.
fn finish(app: &mut ToonApp, d: &CameraDrag) {
    if d.from == d.to {
        return;
    }
    let k = d.moved();
    app.run(Command::SetCameraKey { frame: k.frame, x: k.x, y: k.y, zoom: k.zoom, angle: k.angle });
    app.set_status(tlf!("Camera key at frame {frame} moved", frame = k.frame + 1));
}

/// Draw the path, the key squares and the current frame's cross. `screen` maps canvas pixels to
/// the screen.
pub fn draw(app: &ToonApp, painter: &egui::Painter, screen: &dyn Fn(f32, f32) -> Pos2, col: Color32) {
    if !visible(app) {
        return;
    }
    let pts = path(app);
    if pts.len() >= 2 {
        let line: Vec<Pos2> = pts.iter().map(|(_, x, y)| screen(*x, *y)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.5, col.gamma_multiply(0.7))));
    }
    let preview = previewed(app);
    let keys = &preview.as_ref().unwrap_or(&app.engine.project.camera).keys;
    for k in keys {
        let r = egui::Rect::from_center_size(screen(k.x, k.y), vec2(9.0, 9.0));
        painter.rect_filled(r, 1.0, col);
        painter.rect_stroke(r, 1.0, Stroke::new(1.0, Color32::from_black_alpha(160)), egui::StrokeKind::Outside);
    }
    if let Some((_, x, y)) = pts.iter().find(|(f, _, _)| *f == app.frame) {
        let at = screen(*x, *y);
        painter.circle_stroke(at, 6.0, Stroke::new(1.5, col));
        painter.line_segment([at - vec2(9.0, 0.0), at + vec2(9.0, 0.0)], Stroke::new(1.5, col));
        painter.line_segment([at - vec2(0.0, 9.0), at + vec2(0.0, 9.0)], Stroke::new(1.5, col));
    }
}
