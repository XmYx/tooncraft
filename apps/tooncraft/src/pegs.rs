//! The Pegs panel: pegs (keyed transforms layers hang from), their keys and eases, the current
//! layer's peg and multiplane depth; the camera shake list; and the motion path drawn on the
//! canvas. Every value is a drag value that also takes typing and arrow keys.

use egui::{Color32, Pos2, Stroke, vec2};
use tooncraft_doc::{Ease, PegId, PegKey, Shake};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Panel state that is not part of the project.
pub struct PegUi {
    /// The peg the panel edits and whose motion path the canvas shows.
    pub selected: Option<PegId>,
    pub show_path: bool,
    /// Name being typed for the selected peg.
    pub renaming: Option<String>,
}

impl Default for PegUi {
    fn default() -> Self {
        Self { selected: None, show_path: true, renaming: None }
    }
}

pub fn ease_label(e: Ease) -> String {
    match e {
        Ease::Linear => tl!("Linear"),
        Ease::In => tl!("Ease in"),
        Ease::Out => tl!("Ease out"),
        Ease::InOut => tl!("Ease in and out"),
        Ease::Hold => tl!("Hold"),
        Ease::Curve { .. } => tl!("Custom curve"),
    }
    .to_string()
}

fn peg_name(app: &ToonApp, id: Option<PegId>) -> String {
    match id.and_then(|i| app.engine.project.peg(i)) {
        Some(p) => p.name.clone(),
        None => tl!("None").to_string(),
    }
}

fn key_command(peg: PegId, k: PegKey) -> Command {
    Command::SetPegKey { peg, frame: k.frame, x: k.x, y: k.y, angle: k.angle, scale_x: k.scale_x, scale_y: k.scale_y, skew: k.skew, ease: k.ease }
}

pub fn panel(app: &mut ToonApp, ui: &mut egui::Ui) {
    // keep the selection valid
    if app.peg_ui.selected.is_some_and(|id| app.engine.project.peg(id).is_none()) {
        app.peg_ui.selected = None;
    }
    if app.peg_ui.selected.is_none() {
        app.peg_ui.selected = app.engine.project.pegs.first().map(|p| p.id);
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Add Peg")).on_hover_text(tl!("A new peg; with a peg selected, the new one hangs from it")).clicked() {
            let n = app.engine.project.pegs.len() + 1;
            let parent = app.peg_ui.selected;
            if app.run(Command::AddPeg { name: tlf!("Peg {n}", n = n), parent }) {
                app.peg_ui.selected = app.engine.project.pegs.last().map(|p| p.id);
            }
        }
        if ui.add_enabled(app.peg_ui.selected.is_some(), egui::Button::new(tl!("Delete Peg"))).clicked()
            && let Some(peg) = app.peg_ui.selected
        {
            app.run(Command::DeletePeg { peg });
            app.peg_ui.selected = None;
        }
        ui.checkbox(&mut app.peg_ui.show_path, tl!("Motion path"));
    });
    ui.separator();
    layer_peg(app, ui);
    ui.separator();
    let pegs: Vec<(PegId, String, usize)> = app
        .engine
        .project
        .pegs
        .iter()
        .map(|p| {
            let mut depth = 0;
            let mut up = p.parent;
            while let Some(u) = up {
                depth += 1;
                if depth > tooncraft_doc::peg::MAX_PEG_DEPTH {
                    break;
                }
                up = app.engine.project.peg(u).and_then(|q| q.parent);
            }
            (p.id, p.name.clone(), depth)
        })
        .collect();
    if pegs.is_empty() {
        ui.label(tl!("No pegs yet. Add one, then hang the current layer from it."));
        return;
    }
    egui::ScrollArea::vertical().id_salt("peg-list").max_height(110.0).show(ui, |ui| {
        for (id, name, depth) in &pegs {
            ui.horizontal(|ui| {
                ui.add_space(*depth as f32 * 12.0);
                if ui.selectable_label(app.peg_ui.selected == Some(*id), name).clicked() {
                    app.peg_ui.selected = Some(*id);
                    app.peg_ui.renaming = None;
                }
            });
        }
    });
    let Some(id) = app.peg_ui.selected else { return };
    let Some(peg) = app.engine.project.peg(id).cloned() else { return };
    ui.separator();
    egui::Grid::new("peg-props").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        let l = ui.label(tl!("Name"));
        let mut name = app.peg_ui.renaming.clone().unwrap_or_else(|| peg.name.clone());
        let resp = ui.add(egui::TextEdit::singleline(&mut name).desired_width(120.0)).labelled_by(l.id);
        if resp.changed() {
            app.peg_ui.renaming = Some(name.clone());
        }
        if resp.lost_focus() && app.peg_ui.renaming.take().is_some_and(|n| n != peg.name) {
            app.run(Command::RenamePeg { peg: id, name });
        }
        ui.end_row();
        let l = ui.label(tl!("Hangs from"));
        let mut parent = peg.parent;
        egui::ComboBox::from_id_salt("peg-parent")
            .selected_text(peg_name(app, parent))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut parent, None, tl!("None"));
                for (pid, pname, _) in &pegs {
                    if *pid != id && !tooncraft_doc::peg::would_loop(&app.engine.project.pegs, id, *pid) {
                        ui.selectable_value(&mut parent, Some(*pid), pname);
                    }
                }
            })
            .response
            .labelled_by(l.id);
        if parent != peg.parent {
            app.run(Command::SetPegParent { peg: id, parent });
        }
        ui.end_row();
        let (mut px, mut py) = peg.pivot;
        let l = ui.label(tl!("Pivot X"));
        let cx = ui.add(egui::DragValue::new(&mut px).speed(1.0).max_decimals(1)).labelled_by(l.id).changed();
        ui.end_row();
        let l = ui.label(tl!("Pivot Y"));
        let cy = ui.add(egui::DragValue::new(&mut py).speed(1.0).max_decimals(1)).labelled_by(l.id).changed();
        ui.end_row();
        if cx || cy {
            app.run(Command::SetPegPivot { peg: id, x: px, y: py });
        }
    });
    ui.separator();
    let frame = app.frame;
    let here = peg.has_key(frame);
    ui.label(if here { tlf!("Key at frame {n}", n = frame + 1) } else { tlf!("Frame {n} (between keys)", n = frame + 1) });
    let mut k = peg.at(frame);
    k.frame = frame;
    let mut changed = false;
    egui::Grid::new("peg-key").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        for (label, v, speed, suffix) in [
            (tl!("Move X"), &mut k.x, 1.0, ""),
            (tl!("Move Y"), &mut k.y, 1.0, ""),
            (tl!("Rotation"), &mut k.angle, 0.5, "°"),
            (tl!("Scale X"), &mut k.scale_x, 0.01, ""),
            (tl!("Scale Y"), &mut k.scale_y, 0.01, ""),
            (tl!("Skew"), &mut k.skew, 0.5, "°"),
        ] {
            let l = ui.label(label);
            changed |= ui.add(egui::DragValue::new(v).speed(speed).suffix(suffix).max_decimals(2)).labelled_by(l.id).changed();
            ui.end_row();
        }
        let l = ui.label(tl!("Ease to next key"));
        let mut ease = k.ease;
        egui::ComboBox::from_id_salt("peg-ease")
            .selected_text(ease_label(ease))
            .show_ui(ui, |ui| {
                for e in Ease::PRESETS {
                    ui.selectable_value(&mut ease, e, ease_label(e));
                }
                let curve = match ease {
                    Ease::Curve { .. } => ease,
                    _ => Ease::Curve { x1: 0.3, y1: 0.0, x2: 0.3, y2: 1.0 },
                };
                ui.selectable_value(&mut ease, curve, ease_label(curve));
            })
            .response
            .labelled_by(l.id);
        if ease != k.ease {
            k.ease = ease;
            changed = true;
        }
        ui.end_row();
        if let Ease::Curve { mut x1, mut y1, mut x2, mut y2 } = k.ease {
            let mut c = false;
            for (label, v) in
                [(tl!("Handle 1 time"), &mut x1), (tl!("Handle 1 value"), &mut y1), (tl!("Handle 2 time"), &mut x2), (tl!("Handle 2 value"), &mut y2)]
            {
                let l = ui.label(label);
                c |= ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(2)).labelled_by(l.id).changed();
                ui.end_row();
            }
            if c {
                k.ease = Ease::Curve { x1, y1, x2, y2 };
                changed = true;
            }
        }
    });
    if let Some((x1, y1, x2, y2)) = k.ease.handles() {
        ease_graph(ui, (x1, y1, x2, y2));
    }
    if changed {
        app.run(key_command(id, k));
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Set Key")).clicked() {
            app.run(key_command(id, k));
        }
        if ui.add_enabled(here, egui::Button::new(tl!("Remove Key"))).clicked() {
            app.run(Command::RemovePegKey { peg: id, frame });
        }
    });
    let keys: Vec<u32> = peg.keys.iter().map(|k| k.frame).collect();
    egui::ScrollArea::vertical().id_salt("peg-keys").show(ui, |ui| {
        for f in keys {
            if ui.selectable_label(f == frame, tlf!("Key at frame {n}", n = f + 1)).clicked() {
                app.frame = f;
            }
        }
    });
}

/// The current layer's peg and multiplane depth.
fn layer_peg(app: &mut ToonApp, ui: &mut egui::Ui) {
    let li = app.layer;
    let Some(layer) = app.engine.project.layers.get(li).cloned() else { return };
    egui::Grid::new("layer-peg").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
        let l = ui.label(tlf!("“{layer}” hangs from", layer = layer.name));
        let mut peg = layer.peg;
        let pegs: Vec<(PegId, String)> = app.engine.project.pegs.iter().map(|p| (p.id, p.name.clone())).collect();
        egui::ComboBox::from_id_salt("layer-peg-combo")
            .selected_text(peg_name(app, peg))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut peg, None, tl!("None"));
                for (id, name) in &pegs {
                    ui.selectable_value(&mut peg, Some(*id), name);
                }
            })
            .response
            .labelled_by(l.id);
        if peg != layer.peg {
            app.run(Command::SetLayerPeg { layer: li, peg });
        }
        ui.end_row();
        let l = ui.label(tl!("Depth"));
        let mut depth = layer.depth;
        let resp = ui
            .add(egui::DragValue::new(&mut depth).speed(0.01).range(tooncraft_doc::camera::MIN_DEPTH..=tooncraft_doc::camera::MAX_DEPTH).max_decimals(2))
            .labelled_by(l.id)
            .on_hover_text(tl!("Distance from the drawing plane for the camera: above 0 is further away, below 0 nearer"));
        if resp.changed() {
            app.run(Command::SetLayerDepth { layer: li, depth });
        }
        ui.end_row();
    });
}

/// A small picture of the ease curve.
fn ease_graph(ui: &mut egui::Ui, h: (f32, f32, f32, f32)) {
    let (rect, resp) = ui.allocate_exact_size(vec2(120.0, 60.0), egui::Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, tl!("Ease curve")));
    let pal = ui.visuals().clone();
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, pal.weak_text_color()), egui::StrokeKind::Inside);
    let e = Ease::Curve { x1: h.0, y1: h.1, x2: h.2, y2: h.3 };
    let pts: Vec<Pos2> = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.0;
            let v = e.apply(t);
            egui::pos2(rect.left() + t * rect.width(), rect.bottom() - v.clamp(-0.5, 1.5) * rect.height() * 0.8 - rect.height() * 0.1)
        })
        .collect();
    ui.painter().add(egui::Shape::line(pts, Stroke::new(1.5, pal.selection.stroke.color)));
}

/// Camera shakes, under the camera keys.
pub fn shakes(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.separator();
    ui.label(tl!("Shakes"));
    let frame = app.frame;
    let camera = crate::camera_path::edited_index(app);
    let list = crate::camera_path::edited(app).shakes.clone();
    let mut remove = None;
    for (i, s) in list.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(tlf!("Frames {a}–{b}, {px} px", a = s.from + 1, b = s.to + 1, px = s.amplitude.round()));
            if ui.button(tl!("Remove")).on_hover_text(tl!("Remove this shake")).clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(index) = remove {
        app.run(Command::RemoveCameraShake { index, camera });
    }
    if ui.button(tl!("Add Shake Here")).on_hover_text(tl!("Shake the camera for 12 frames from the current frame")).clicked() {
        let last = app.engine.project.length().saturating_sub(1);
        let shake = Shake { from: frame, to: (frame + 11).min(last), amplitude: 12.0, rotation: 1.0, frequency: 0.4, fade: 3, seed: frame };
        app.run(Command::AddCameraShake { shake, camera });
    }
}

/// The selected peg's motion path on the canvas: a line through the pivot's positions, a dot per
/// frame, a square per key and a cross at the current frame. `screen` maps canvas pixels to the
/// screen.
pub fn draw_path(app: &ToonApp, painter: &egui::Painter, screen: impl Fn(f32, f32) -> Pos2, accent: Color32) {
    if !app.peg_ui.show_path || app.playback.playing {
        return;
    }
    let Some(id) = app.peg_ui.selected else { return };
    let p = &app.engine.project;
    let Some(peg) = p.peg(id) else { return };
    let len = p.length();
    let (from, to) = (app.frame.saturating_sub(500), (app.frame + 500).min(len.saturating_sub(1)));
    let path = p.motion_path(id, from, to);
    if path.len() < 2 {
        return;
    }
    let pts: Vec<Pos2> = path.iter().map(|(x, y)| screen(*x, *y)).collect();
    painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.5, accent.gamma_multiply(0.8))));
    for (i, pt) in pts.iter().enumerate() {
        let f = from + i as u32;
        if peg.has_key(f) {
            painter.rect_filled(egui::Rect::from_center_size(*pt, vec2(7.0, 7.0)), 1.0, accent);
        } else {
            painter.circle_filled(*pt, 1.8, accent.gamma_multiply(0.7));
        }
        if f == app.frame {
            painter.line_segment([*pt - vec2(7.0, 0.0), *pt + vec2(7.0, 0.0)], Stroke::new(1.5, accent));
            painter.line_segment([*pt - vec2(0.0, 7.0), *pt + vec2(0.0, 7.0)], Stroke::new(1.5, accent));
        }
    }
}
