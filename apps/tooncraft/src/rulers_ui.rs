//! Stroke rulers in the app: the Rulers toggle above the canvas and its menu (add a straight,
//! curve, ellipse, concentric or perspective ruler, grid snapping, reach, typed handle positions,
//! remove), the rulers drawn on the canvas, and their handles dragged with the pointer. The
//! snapping itself is `tooncraft_doc::rulers`, applied by the engine to every stroke.
//!
//! Keyboard path: the toggle and every menu item are buttons, and each handle has typed X and Y
//! fields in the menu (Shift+F10 or the Menu key on the focused toggle opens it).

use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use tooncraft_doc::rulers::{Ruler, Rulers};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// The rulers' outlines for one `Rulers` value and canvas size (rebuilding them every frame
/// costs up to a few hundred thousand points with many concentric rulers).
#[derive(Default)]
pub struct Outlines {
    key: Option<(Rulers, (u32, u32))>,
    lines: Vec<Vec<(f32, f32)>>,
}

impl Outlines {
    fn get(&mut self, rulers: &Rulers, size: (u32, u32)) -> &[Vec<(f32, f32)>] {
        if self.key.as_ref().is_none_or(|(r, s)| r != rulers || *s != size) {
            self.lines = rulers.list.iter().flat_map(|r| r.outline(size.0, size.1)).collect();
            self.key = Some((rulers.clone(), size));
        }
        &self.lines
    }
}

/// Screen pixels from a handle that still grab it.
const GRAB: f32 = 8.0;

fn kind_name(r: &Ruler) -> String {
    match r {
        Ruler::Line { .. } => tl!("Straight ruler"),
        Ruler::Curve { .. } => tl!("Curve ruler"),
        Ruler::Ellipse { .. } => tl!("Ellipse ruler"),
        Ruler::Concentric { .. } => tl!("Concentric ruler"),
        Ruler::Perspective { points } => match points.len() {
            1 => tl!("Perspective, one point"),
            2 => tl!("Perspective, two points"),
            _ => tl!("Perspective, three points"),
        },
    }
    .to_string()
}

/// A new ruler of each kind, placed on a `w`×`h` canvas.
pub fn starters(w: u32, h: u32) -> Vec<(String, Ruler)> {
    let (w, h) = (w as f32, h as f32);
    vec![
        (tl!("Add Straight Ruler").to_string(), Ruler::Line { a: (w * 0.25, h * 0.5), b: (w * 0.75, h * 0.5) }),
        (tl!("Add Curve Ruler").to_string(), Ruler::Curve { points: vec![(w * 0.2, h * 0.6), (w * 0.5, h * 0.35), (w * 0.8, h * 0.6)] }),
        (tl!("Add Ellipse Ruler").to_string(), Ruler::Ellipse { center: (w * 0.5, h * 0.5), rx: w * 0.25, ry: h * 0.25, angle: 0.0 }),
        (tl!("Add Concentric Ruler").to_string(), Ruler::Concentric { center: (w * 0.5, h * 0.5), ratio: 0.5, angle: 0.0 }),
        (tl!("Add One-Point Perspective").to_string(), Ruler::Perspective { points: vec![(w * 0.5, h * 0.4)] }),
        (tl!("Add Two-Point Perspective").to_string(), Ruler::Perspective { points: vec![(0.0, h * 0.4), (w, h * 0.4)] }),
        (tl!("Add Three-Point Perspective").to_string(), Ruler::Perspective { points: vec![(0.0, h * 0.4), (w, h * 0.4), (w * 0.5, h * 3.0)] }),
    ]
}

/// `--rulers`: a straight ruler and two-point perspective, on (for screenshots).
pub fn demo(app: &mut ToonApp) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let pick = |i: usize| starters(w, h).into_iter().nth(i).map(|(_, r)| r);
    let list = [pick(0), pick(5)].into_iter().flatten().collect();
    app.run(Command::SetRulers { rulers: Rulers { on: true, list, ..Rulers::default() } });
}

fn set(app: &mut ToonApp, rulers: Rulers) {
    if rulers != app.engine.project.rulers {
        app.run(Command::SetRulers { rulers });
    }
}

/// The Rulers toggle in the strip above the canvas; right-click (or Shift+F10) for its menu.
pub fn toggle(app: &mut ToonApp, ui: &mut egui::Ui) {
    let mut on = app.engine.project.rulers.on;
    let resp = ui
        .add(crate::icons::IconButton::new(crate::icons::Icon::Rulers, tl!("Rulers")).active(on))
        .on_hover_text(tl!("Strokes started near a ruler follow it; right-click to add rulers and choose grid snapping"));
    if resp.clicked() {
        on = !on;
        let mut r = app.engine.project.rulers.clone();
        r.on = on;
        if on && r.list.is_empty() && !r.grid {
            r.grid = true;
            app.set_status(tl!("Rulers on: strokes started near a grid line follow it; right-click Rulers to add a ruler"));
        }
        set(app, r);
    }
    let now = resp.has_focus() && crate::timeline::menu_key(ui);
    crate::timeline::keyboard_menu(ui, &resp, now, |ui| menu(app, ui));
    resp.context_menu(|ui| menu(app, ui));
}

/// The rulers menu: add, grid, reach, every ruler with typed handles, remove.
pub fn menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    let mut r = app.engine.project.rulers.clone();
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    ui.checkbox(&mut r.on, tl!("Strokes follow rulers"));
    ui.checkbox(&mut r.grid, tl!("Snap strokes to grid lines"));
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Reach"));
        ui.add(egui::DragValue::new(&mut r.reach).range(1.0..=500.0).suffix(" px")).labelled_by(l.id);
    });
    ui.separator();
    if r.list.len() < tooncraft_doc::rulers::MAX_RULERS {
        for (name, ruler) in starters(w, h) {
            if ui.button(name).clicked() {
                r.list.push(ruler);
                r.on = true;
            }
        }
    }
    let mut remove = None;
    if !r.list.is_empty() {
        ui.separator();
    }
    for (i, ruler) in r.list.iter_mut().enumerate() {
        ui.push_id(("ruler", i), |ui| {
            ui.horizontal(|ui| {
                ui.label(kind_name(ruler));
                if ui.button(tl!("Remove")).on_hover_text(tl!("Remove this ruler")).clicked() {
                    remove = Some(i);
                }
            });
            for (k, (x, y)) in ruler.handles().into_iter().enumerate() {
                let (mut x, mut y) = (x, y);
                let changed = ui
                    .horizontal(|ui| {
                        let lx = ui.label(tlf!("Point {n} X", n = k + 1));
                        let a = ui.add(egui::DragValue::new(&mut x).speed(1.0)).labelled_by(lx.id).changed();
                        let ly = ui.label(tl!("Y"));
                        let b = ui.add(egui::DragValue::new(&mut y).speed(1.0)).labelled_by(ly.id).changed();
                        a || b
                    })
                    .inner;
                if changed {
                    ruler.move_handle(k, (x, y));
                }
            }
        });
    }
    if let Some(i) = remove {
        r.list.remove(i);
    }
    if !r.list.is_empty() && ui.button(tl!("Remove All Rulers")).clicked() {
        r.list.clear();
    }
    set(app, r.sanitized());
}

/// A press on a ruler handle moves the handle instead of drawing; Escape during the drag puts the
/// rulers back as they were at the press. Returns true while it does.
pub fn drag(app: &mut ToonApp, ui: &egui::Ui, rect: Rect) -> bool {
    if !app.engine.project.rulers.on || app.engine.is_stroking() {
        app.aids.ruler_drag = None;
        return false;
    }
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    let (pressed, down, pos, esc) =
        ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.interact_pos(), i.key_pressed(egui::Key::Escape)));
    if let Some((ri, hi, before)) = app.aids.ruler_drag.clone() {
        if esc {
            app.aids.ruler_drag = None;
            app.engine.project.rulers = before;
            app.set_status(tl!("Ruler move cancelled"));
        } else if down {
            if let Some(p) = pos {
                let c = app.view.to_canvas(rect, size, p);
                if let Some(r) = app.engine.project.rulers.list.get_mut(ri) {
                    r.move_handle(hi, (c.x, c.y));
                }
            }
        } else {
            app.aids.ruler_drag = None;
            // the live edits above are kept; this checks them and marks the shot changed
            let r = app.engine.project.rulers.clone();
            app.run(Command::SetRulers { rulers: r });
        }
        return true;
    }
    let Some(p) = pos.filter(|p| pressed && rect.contains(*p)) else { return false };
    if ui.ctx().layer_id_at(p).is_some_and(|l| l != ui.layer_id()) {
        return false;
    }
    let near = |q: (f32, f32)| app.view.to_screen(rect, size, pos2(q.0, q.1)).distance(p) <= GRAB;
    let hit = app.engine.project.rulers.list.iter().enumerate().find_map(|(ri, r)| r.handles().into_iter().position(near).map(|hi| (ri, hi, kind_name(r))));
    if let Some((ri, hi, kind)) = hit {
        app.aids.ruler_drag = Some((ri, hi, app.engine.project.rulers.clone()));
        app.set_status(tlf!("Moving the {kind}", kind = kind));
        return true;
    }
    false
}

/// The rulers and their handles on the canvas.
pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2) {
    let rulers = &app.engine.project.rulers;
    if !rulers.on {
        return;
    }
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let colour = Color32::from_rgb(240, 120, 200);
    let line = Stroke::new(1.0, colour.gamma_multiply(0.7));
    if let Ok(mut cache) = app.aids.ruler_outlines.try_borrow_mut() {
        for part in cache.get(rulers, (w, h)) {
            let pts: Vec<Pos2> = part.iter().map(|p| to_screen(p.0, p.1)).collect();
            if pts.len() >= 2 {
                painter.add(egui::Shape::line(pts, line));
            }
        }
    }
    for r in &rulers.list {
        for (x, y) in r.handles() {
            let c = to_screen(x, y);
            painter.circle_filled(c, 4.0, colour);
            painter.circle_stroke(c, 5.5, Stroke::new(1.0, Color32::from_black_alpha(160)));
        }
    }
}
