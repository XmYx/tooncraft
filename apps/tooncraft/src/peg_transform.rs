//! The Transform tool: move, scale and turn the current layer's peg on the canvas, and drag its
//! pivot. A drag shows the layer where it will go and sets one peg key at the current frame when
//! it ends (one undo step; Escape cancels). With the canvas focused the arrow keys move the peg a
//! pixel (Shift: ten), `[` and `]` turn it a degree (Shift: fifteen), Page Up and Page Down scale
//! it by 1 %. The tool options show the key's values for typing.

use egui::{Pos2, Rect, Stroke, Ui, pos2, vec2};
use tooncraft_doc::peg::{self, PegDrag};
use tooncraft_doc::{Affine, PegId, PegKey};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Handles are picked this close (screen points).
const PICK: f32 = 9.0;
/// The turn knob sits this far outside the box (screen points).
const KNOB: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    Peg(PegDrag),
    Pivot,
}

/// A drag in progress: nothing changes in the project until it ends.
#[derive(Clone, Copy, Debug)]
struct Drag {
    peg: PegId,
    frame: u32,
    grab: Grab,
    /// Canvas pixels where it began and where the pointer is now.
    from: (f32, f32),
    to: (f32, f32),
}

#[derive(Default)]
pub struct TransformUi {
    drag: Option<Drag>,
    /// The key while one of its number fields is being dragged (applied once, on release).
    typed: Option<PegKey>,
}

#[cfg(test)]
impl TransformUi {
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.typed.is_some()
    }
}

/// The peg the tool works on: the current layer's.
pub fn target(app: &ToonApp) -> Option<PegId> {
    app.engine.project.layers.get(app.layer).and_then(|l| l.peg).filter(|id| app.engine.project.peg(*id).is_some())
}

/// The key a drag would set (the peg's own key at the drag's frame).
fn preview_key(app: &ToonApp, d: &Drag) -> Option<PegKey> {
    match d.grab {
        Grab::Peg(kind) => peg::dragged_key(&app.engine.project.pegs, d.peg, d.frame, kind, d.from, d.to),
        Grab::Pivot => None,
    }
}

/// The pegs with the drag's key in place, while a drag is in progress.
fn previewed_pegs(app: &ToonApp) -> Option<Vec<tooncraft_doc::Peg>> {
    if let Some(pegs) = crate::motion_record::previewed_pegs(app) {
        return Some(pegs);
    }
    let d = app.transform_ui.drag?;
    let k = preview_key(app, &d)?;
    let mut pegs = app.engine.project.pegs.clone();
    pegs.iter_mut().find(|q| q.id == d.peg)?.set_key(k);
    Some(pegs)
}

/// The layer transforms the canvas draws with this frame: through the drag's key while one is in
/// progress (the pegs are cloned once per frame, only during a drag).
pub fn layer_transforms(app: &ToonApp) -> Vec<Affine> {
    let p = &app.engine.project;
    match previewed_pegs(app) {
        Some(pegs) => p.layers.iter().map(|l| l.peg.map(|id| peg::chain(&pegs, id, app.frame)).unwrap_or(Affine::IDENTITY)).collect(),
        None => (0..p.layers.len()).map(|i| p.layer_transform(i, app.frame)).collect(),
    }
}

/// The drawing area's corners through the peg, the pivot and the turn knob, on screen.
fn handles(app: &ToonApp, id: PegId, screen: &dyn Fn(f32, f32) -> Pos2) -> Option<([Pos2; 4], Pos2, Pos2)> {
    let p = &app.engine.project;
    let t = match previewed_pegs(app) {
        Some(pegs) => peg::chain(&pegs, id, app.frame),
        None => peg::chain(&p.pegs, id, app.frame),
    };
    let (w, h) = (p.width() as f32, p.height() as f32);
    let c = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| {
        let (x, y) = t.apply(x, y);
        screen(x, y)
    });
    let pv = p.peg(id)?.pivot;
    let pivot = match app.transform_ui.drag {
        Some(Drag { grab: Grab::Pivot, to, .. }) => screen(to.0, to.1),
        _ => {
            let (x, y) = t.apply(pv.0, pv.1);
            screen(x, y)
        }
    };
    let top = pos2((c[0].x + c[1].x) * 0.5, (c[0].y + c[1].y) * 0.5);
    let centre = pos2((c[0].x + c[2].x) * 0.5, (c[0].y + c[2].y) * 0.5);
    let out = (top - centre).normalized();
    let out = if out.x.is_finite() && out.y.is_finite() && out != egui::Vec2::ZERO { out } else { vec2(0.0, -1.0) };
    Some((c, pivot, top + out * KNOB))
}

/// Pointer input on the canvas with the Transform tool. `canvas` maps a screen point to canvas
/// pixels (the composed picture), `screen` the other way.
pub fn input(app: &mut ToonApp, ui: &Ui, resp: &egui::Response, canvas: &dyn Fn(Pos2) -> (f32, f32), screen: &dyn Fn(f32, f32) -> Pos2) {
    let Some(id) = target(app) else {
        if resp.drag_started() || resp.clicked() {
            app.set_status(tl!("This layer hangs from no peg: add one from the tool options"));
        }
        return;
    };
    if resp.drag_started()
        && let Some(p) = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos())
        && let Some((corners, pivot, knob)) = handles(app, id, screen)
    {
        let grab = if p.distance(pivot) <= PICK {
            Grab::Pivot
        } else if p.distance(knob) <= PICK {
            Grab::Peg(PegDrag::Rotate)
        } else if corners.iter().any(|c| c.distance(p) <= PICK) {
            Grab::Peg(PegDrag::Scale)
        } else {
            Grab::Peg(PegDrag::Move)
        };
        let at = canvas(p);
        app.transform_ui.drag = Some(Drag { peg: id, frame: app.frame, grab, from: at, to: at });
    }
    if let Some(d) = app.transform_ui.drag.as_mut()
        && let Some(p) = resp.interact_pointer_pos()
    {
        d.to = canvas(p);
    }
    if app.transform_ui.drag.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.transform_ui.drag = None;
        app.set_status(tl!("Transform cancelled"));
        return;
    }
    if resp.drag_stopped() {
        if let Some(d) = app.transform_ui.drag.take() {
            finish(app, &d);
        }
    } else if app.transform_ui.drag.is_some() && !resp.dragged() && !ui.input(|i| i.pointer.any_down()) {
        // let go where the canvas did not see it: drop the preview
        app.transform_ui.drag = None;
    }
}

fn finish(app: &mut ToonApp, d: &Drag) {
    if d.from == d.to || app.engine.project.peg(d.peg).is_none() {
        return;
    }
    match d.grab {
        Grab::Peg(_) => match preview_key(app, d) {
            Some(k) => {
                app.run(key_command(d.peg, k));
            }
            None => app.set_status(tl!("Drag further from the pivot to scale or turn")),
        },
        Grab::Pivot => {
            if let Some((x, y)) = peg::pivot_under(&app.engine.project.pegs, d.peg, d.frame, d.to) {
                app.run(Command::SetPegPivot { peg: d.peg, x, y });
            }
        }
    }
}

pub fn key_command(peg: PegId, k: PegKey) -> Command {
    Command::SetPegKey { peg, frame: k.frame, x: k.x, y: k.y, angle: k.angle, scale_x: k.scale_x, scale_y: k.scale_y, skew: k.skew, ease: k.ease }
}

/// Arrow keys, `[` `]` and Page Up / Down with the canvas focused. Returns whether it used a key.
pub fn keyboard(app: &mut ToonApp, ui: &Ui) -> bool {
    let Some(id) = target(app) else { return false };
    let (dx, dy, turn, scale) = ui.input_mut(|i| {
        use egui::Key::*;
        use egui::Modifiers as M;
        let mut n = |m: M, k| i.consume_key(m, k) as i32 as f32;
        let dx = 10.0 * (n(M::SHIFT, ArrowRight) - n(M::SHIFT, ArrowLeft)) + n(M::NONE, ArrowRight) - n(M::NONE, ArrowLeft);
        let dy = 10.0 * (n(M::SHIFT, ArrowDown) - n(M::SHIFT, ArrowUp)) + n(M::NONE, ArrowDown) - n(M::NONE, ArrowUp);
        let turn = 15.0 * (n(M::SHIFT, CloseBracket) - n(M::SHIFT, OpenBracket)) + n(M::NONE, CloseBracket) - n(M::NONE, OpenBracket);
        let scale = n(M::NONE, PageUp) - n(M::NONE, PageDown);
        (dx, dy, turn, scale)
    });
    if dx == 0.0 && dy == 0.0 && turn == 0.0 && scale == 0.0 {
        return false;
    }
    let mut k = match (dx, dy) {
        (0.0, 0.0) => app.engine.project.peg(id).map(|p| p.at(app.frame)),
        // a nudge moves the drawing that far on the canvas, whatever the parent pegs do
        _ => peg::dragged_key(&app.engine.project.pegs, id, app.frame, PegDrag::Move, (0.0, 0.0), (dx, dy)),
    }
    .unwrap_or(PegKey::rest(app.frame));
    k.frame = app.frame;
    k.angle += turn;
    let r = 1.0 + 0.01 * scale;
    k.scale_x *= r;
    k.scale_y *= r;
    app.run(key_command(id, k));
    true
}

/// The box, its handles and the pivot over the canvas.
pub fn overlay(app: &ToonApp, painter: &egui::Painter, screen: &dyn Fn(f32, f32) -> Pos2, accent: egui::Color32, panel: egui::Color32) {
    let Some(id) = target(app) else { return };
    let Some((c, pivot, knob)) = handles(app, id, screen) else { return };
    let st = Stroke::new(1.5, accent);
    painter.add(egui::Shape::closed_line(c.to_vec(), st));
    let top = pos2((c[0].x + c[1].x) * 0.5, (c[0].y + c[1].y) * 0.5);
    painter.line_segment([top, knob], Stroke::new(1.0, accent));
    painter.circle(knob, 5.0, panel, st);
    for q in &c {
        painter.rect(Rect::from_center_size(*q, vec2(8.0, 8.0)), 1.0, panel, st, egui::StrokeKind::Middle);
    }
    painter.circle_stroke(pivot, 6.0, st);
    painter.line_segment([pivot - vec2(9.0, 0.0), pivot + vec2(9.0, 0.0)], st);
    painter.line_segment([pivot - vec2(0.0, 9.0), pivot + vec2(0.0, 9.0)], st);
}

/// Tool options: typed values for the peg's key at this frame, or a button to add a peg.
pub fn options(app: &mut ToonApp, ui: &mut Ui) {
    let Some(id) = target(app) else {
        let layer = app.engine.project.layers.get(app.layer).map(|l| l.name.clone()).unwrap_or_default();
        ui.label(tlf!("“{layer}” hangs from no peg", layer = layer));
        if ui.button(tl!("Add a Peg for This Layer")).on_hover_text(tl!("Make a new peg and hang the current layer from it")).clicked() {
            let n = app.engine.project.pegs.len() + 1;
            let layer = app.layer;
            if app.run(Command::AddPegForLayer { layer, name: tlf!("Peg {n}", n = n) }) {
                app.peg_ui.selected = app.engine.project.pegs.last().map(|p| p.id);
            }
        }
        return;
    };
    let Some(pg) = app.engine.project.peg(id).cloned() else { return };
    ui.label(tlf!("Peg “{name}”", name = pg.name));
    let mut stored = pg.at(app.frame);
    stored.frame = app.frame;
    let mut k = app.transform_ui.typed.filter(|t| t.frame == app.frame).unwrap_or(stored);
    let mut commit = false;
    let mut dragging = false;
    for (label, v, speed) in [
        (tl!("Move X"), &mut k.x, 1.0),
        (tl!("Move Y"), &mut k.y, 1.0),
        (tl!("Rotation"), &mut k.angle, 0.5),
        (tl!("Scale X"), &mut k.scale_x, 0.01),
        (tl!("Scale Y"), &mut k.scale_y, 0.01),
    ] {
        let l = ui.label(label);
        let r = ui.add(egui::DragValue::new(v).speed(speed).max_decimals(2)).labelled_by(l.id);
        // a pointer drag on a number is applied once, on release: one undo step
        commit |= (r.changed() && !r.dragged()) || r.drag_stopped();
        dragging |= r.dragged() && !r.drag_stopped();
    }
    app.transform_ui.typed = (dragging && !commit).then_some(k);
    if commit && k != stored {
        app.run(key_command(id, k));
    }
    ui.label(tl!("Drag to move, a corner to scale, the knob to turn, the cross to move the pivot"));
    let mut armed = app.recorder.armed;
    if ui
        .checkbox(&mut armed, tl!("Record Motion"))
        .on_hover_text(tl!("Drag the layer while the shot plays and every frame it passes is keyed (one undo step; Escape drops the take)"))
        .changed()
    {
        crate::motion_record::set_armed(app, armed);
    }
}
