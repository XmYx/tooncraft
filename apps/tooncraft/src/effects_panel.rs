//! The Effects panel: the current layer's effect stack. Add an effect, switch it on or off, move
//! or remove it, change its settings at the current frame and key them. Every change is an
//! engine command (one undo step).

use egui::vec2;
use tooncraft_engine::Command;
use tooncraft_fx::{Effect, EffectKind, MaskShape};

use crate::app::ToonApp;

/// The effect's name in the UI.
pub fn effect_name(k: &EffectKind) -> String {
    match k {
        EffectKind::Blur { .. } => tl!("Soft blur"),
        EffectKind::DirectionalBlur { .. } => tl!("Motion blur"),
        EffectKind::RadialBlur { .. } => tl!("Zoom blur"),
        EffectKind::Glow { .. } => tl!("Glow"),
        EffectKind::DropShadow { .. } => tl!("Drop shadow"),
        EffectKind::Outline { .. } => tl!("Outline"),
        EffectKind::Thicken { .. } => tl!("Thicken lines"),
        EffectKind::Levels { .. } => tl!("Levels"),
        EffectKind::Curves { .. } => tl!("Tone curve"),
        EffectKind::HueSaturation { .. } => tl!("Hue and saturation"),
        EffectKind::Tint { .. } => tl!("Tint"),
        EffectKind::Grain { .. } => tl!("Grain"),
        EffectKind::LightWrap { .. } => tl!("Light wrap"),
        EffectKind::AlphaThreshold { .. } => tl!("Hard edges"),
        EffectKind::Mask { .. } => tl!("Soft mask"),
        EffectKind::ColourKey { .. } => tl!("Colour key"),
        EffectKind::GradientMap { .. } => tl!("Gradient map"),
        EffectKind::Posterize { .. } => tl!("Posterise"),
        EffectKind::Threshold { .. } => tl!("Black and white"),
        EffectKind::Vignette { .. } => tl!("Vignette"),
        EffectKind::ChromaticAberration { .. } => tl!("Colour fringe"),
        EffectKind::Ripple { .. } => tl!("Ripple"),
        EffectKind::Wave { .. } => tl!("Wave"),
        EffectKind::Turbulence { .. } => tl!("Turbulence"),
        EffectKind::Smooth { .. } => tl!("Smooth lines"),
    }
    .to_string()
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>, speed: f64) -> bool {
    let l = ui.label(label);
    let r = ui.add(egui::DragValue::new(v).range(range).speed(speed).max_decimals(2)).labelled_by(l.id).changed();
    ui.end_row();
    r
}

fn colour(ui: &mut egui::Ui, id: &str, label: &str, c: &mut [u8; 3]) -> bool {
    ui.label(label);
    let r = crate::color::swatch_edit(ui, id, label, c);
    ui.end_row();
    r
}

fn check(ui: &mut egui::Ui, label: &str, v: &mut bool) -> bool {
    ui.label("");
    let r = ui.checkbox(v, label).changed();
    ui.end_row();
    r
}

/// Edit the parameters in place; true when one changed.
fn params(ui: &mut egui::Ui, id: usize, k: &mut EffectKind) -> bool {
    let mut ch = false;
    let cid = format!("fx-colour-{id}");
    egui::Grid::new(("fx-params", id)).num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| match k {
        EffectKind::Blur { radius } => ch |= num(ui, tl!("Radius"), radius, 0.0..=200.0, 0.1),
        EffectKind::DirectionalBlur { length, angle } => {
            ch |= num(ui, tl!("Length"), length, 0.0..=500.0, 0.5);
            ch |= num(ui, tl!("Angle"), angle, -360.0..=360.0, 1.0);
        }
        EffectKind::RadialBlur { amount, centre_x, centre_y } => {
            ch |= num(ui, tl!("Amount"), amount, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Centre X"), centre_x, -1.0..=2.0, 0.005);
            ch |= num(ui, tl!("Centre Y"), centre_y, -1.0..=2.0, 0.005);
        }
        EffectKind::Glow { radius, intensity, colour: c } => {
            ch |= num(ui, tl!("Radius"), radius, 0.0..=200.0, 0.1);
            ch |= num(ui, tl!("Intensity"), intensity, 0.0..=5.0, 0.01);
            ch |= colour(ui, &cid, tl!("Glow colour"), c);
        }
        EffectKind::DropShadow { dx, dy, blur, colour: c, opacity } => {
            ch |= num(ui, tl!("Offset X"), dx, -4096.0..=4096.0, 0.5);
            ch |= num(ui, tl!("Offset Y"), dy, -4096.0..=4096.0, 0.5);
            ch |= num(ui, tl!("Softness"), blur, 0.0..=200.0, 0.1);
            ch |= num(ui, tl!("Opacity"), opacity, 0.0..=1.0, 0.01);
            ch |= colour(ui, &cid, tl!("Shadow colour"), c);
        }
        EffectKind::Outline { width, colour: c } => {
            ch |= num(ui, tl!("Width"), width, 0.0..=100.0, 0.1);
            ch |= colour(ui, &cid, tl!("Outline colour"), c);
        }
        EffectKind::Thicken { amount } => ch |= num(ui, tl!("Pixels"), amount, -50.0..=50.0, 0.05),
        EffectKind::Levels { in_black, in_white, gamma, out_black, out_white } => {
            ch |= num(ui, tl!("Input black"), in_black, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Input white"), in_white, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Gamma"), gamma, 0.1..=10.0, 0.01);
            ch |= num(ui, tl!("Output black"), out_black, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Output white"), out_white, 0.0..=1.0, 0.005);
        }
        EffectKind::Curves { points } => {
            for (i, p) in points.iter_mut().enumerate() {
                ch |= num(ui, &tlf!("Point {n} in", n = i + 1), &mut p[0], 0.0..=1.0, 0.005);
                ch |= num(ui, &tlf!("Point {n} out", n = i + 1), &mut p[1], 0.0..=1.0, 0.005);
            }
        }
        EffectKind::HueSaturation { hue, saturation, lightness } => {
            ch |= num(ui, tl!("Hue"), hue, -180.0..=180.0, 0.5);
            ch |= num(ui, tl!("Saturation"), saturation, -1.0..=1.0, 0.005);
            ch |= num(ui, tl!("Lightness"), lightness, -1.0..=1.0, 0.005);
        }
        EffectKind::Tint { colour: c, amount } => {
            ch |= num(ui, tl!("Amount"), amount, 0.0..=1.0, 0.005);
            ch |= colour(ui, &cid, tl!("Tint colour"), c);
        }
        EffectKind::Grain { amount, size, animated, .. } => {
            ch |= num(ui, tl!("Amount"), amount, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Grain size"), size, 1.0..=16.0, 0.05);
            ch |= check(ui, tl!("New grain every frame"), animated);
        }
        EffectKind::LightWrap { radius, intensity } => {
            ch |= num(ui, tl!("Radius"), radius, 0.0..=200.0, 0.1);
            ch |= num(ui, tl!("Intensity"), intensity, 0.0..=2.0, 0.01);
        }
        EffectKind::AlphaThreshold { threshold } => ch |= num(ui, tl!("Threshold"), threshold, 0.0..=1.0, 0.005),
        EffectKind::Mask { shape, x, y, width, height, feather, invert } => {
            let mut ellipse = *shape == MaskShape::Ellipse;
            if check(ui, tl!("Ellipse"), &mut ellipse) {
                *shape = if ellipse { MaskShape::Ellipse } else { MaskShape::Rectangle };
                ch = true;
            }
            ch |= num(ui, tl!("Centre X"), x, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Centre Y"), y, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Width"), width, 0.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Height"), height, 0.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Feather"), feather, 0.0..=4096.0, 0.5);
            ch |= check(ui, tl!("Keep the outside"), invert);
        }
        EffectKind::ColourKey { colour: c, tolerance, softness } => {
            ch |= colour(ui, &cid, tl!("Key colour"), c);
            ch |= num(ui, tl!("Tolerance"), tolerance, 0.0..=1.0, 0.005);
            ch |= num(ui, tl!("Softness"), softness, 0.0..=1.0, 0.005);
        }
        EffectKind::GradientMap { dark, light, amount } => {
            ch |= colour(ui, &format!("{cid}-dark"), tl!("Dark colour"), dark);
            ch |= colour(ui, &format!("{cid}-light"), tl!("Light colour"), light);
            ch |= num(ui, tl!("Amount"), amount, 0.0..=1.0, 0.005);
        }
        EffectKind::Posterize { levels } => {
            let l = ui.label(tl!("Levels"));
            ch |= ui.add(egui::DragValue::new(levels).range(2..=64)).labelled_by(l.id).changed();
            ui.end_row();
        }
        EffectKind::Threshold { level } => ch |= num(ui, tl!("Threshold"), level, 0.0..=1.0, 0.005),
        EffectKind::Vignette { x, y, radius, softness, amount } => {
            ch |= num(ui, tl!("Centre X"), x, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Centre Y"), y, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Radius"), radius, 0.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Softness"), softness, 1.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Amount"), amount, 0.0..=1.0, 0.005);
        }
        EffectKind::ChromaticAberration { amount } => ch |= num(ui, tl!("Pixels"), amount, 0.0..=50.0, 0.05),
        EffectKind::Ripple { x, y, amplitude, wavelength, speed } => {
            ch |= num(ui, tl!("Centre X"), x, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Centre Y"), y, -65536.0..=65536.0, 1.0);
            ch |= num(ui, tl!("Height"), amplitude, 0.0..=100.0, 0.1);
            ch |= num(ui, tl!("Wavelength"), wavelength, 2.0..=2000.0, 0.5);
            ch |= num(ui, tl!("Speed"), speed, -10.0..=10.0, 0.005);
        }
        EffectKind::Wave { amplitude, wavelength, angle, speed } => {
            ch |= num(ui, tl!("Height"), amplitude, 0.0..=100.0, 0.1);
            ch |= num(ui, tl!("Wavelength"), wavelength, 2.0..=2000.0, 0.5);
            ch |= num(ui, tl!("Angle"), angle, -3600.0..=3600.0, 0.5);
            ch |= num(ui, tl!("Speed"), speed, -10.0..=10.0, 0.005);
        }
        EffectKind::Turbulence { amount, scale, animated, .. } => {
            ch |= num(ui, tl!("Pixels"), amount, 0.0..=100.0, 0.1);
            ch |= num(ui, tl!("Size"), scale, 2.0..=1000.0, 0.5);
            ch |= check(ui, tl!("Change every frame"), animated);
        }
        EffectKind::Smooth { radius } => ch |= num(ui, tl!("Radius"), radius, 0.0..=20.0, 0.05),
    });
    ch
}

fn one_effect(app: &mut ToonApp, ui: &mut egui::Ui, li: usize, i: usize, fx: &Effect, count: usize) {
    let frame = app.frame;
    let name = effect_name(&fx.kind);
    ui.horizontal(|ui| {
        let mut on = fx.enabled;
        if ui.checkbox(&mut on, &name).changed() {
            app.run(Command::EnableEffect { layer: li, index: i, enabled: on });
        }
        if ui.add_enabled(i > 0, egui::Button::new(tlf!("Move {name} up", name = name)).small()).clicked() {
            app.run(Command::MoveEffect { layer: li, from: i, to: i - 1 });
        }
        if ui.add_enabled(i + 1 < count, egui::Button::new(tlf!("Move {name} down", name = name)).small()).clicked() {
            app.run(Command::MoveEffect { layer: li, from: i, to: i + 1 });
        }
        if ui.small_button(tlf!("Remove {name}", name = name)).clicked() {
            app.run(Command::RemoveEffect { layer: li, index: i });
        }
    });
    let mut k = fx.at(frame);
    if params(ui, i, &mut k) {
        app.run(Command::EditEffect { layer: li, index: i, frame, value: k });
    }
    // how the result lays over what the effect was given
    let (mut blend, mut mix) = (fx.blend, fx.mix);
    let mut changed = false;
    egui::Grid::new(("fx-blend", i)).num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        let l = ui.label(tl!("Result blend"));
        egui::ComboBox::from_id_salt(("fx-blend-mode", i))
            .selected_text(crate::panels::blend_name(blend))
            .show_ui(ui, |ui| {
                for b in tooncraft_doc::BlendMode::ALL {
                    changed |= ui.selectable_value(&mut blend, b, crate::panels::blend_name(b)).changed();
                }
            })
            .response
            .labelled_by(l.id);
        ui.end_row();
        changed |= num(ui, tl!("Strength"), &mut mix, 0.0..=1.0, 0.005);
    });
    if changed {
        let mut new = fx.clone();
        new.blend = blend;
        new.mix = mix;
        app.run(Command::SetEffect { layer: li, index: i, effect: new });
    }
    let key_here = fx.keys.iter().any(|key| key.frame == frame);
    ui.horizontal(|ui| {
        if ui.button(tlf!("Key {name} here", name = name)).clicked() {
            app.run(Command::SetEffectKey { layer: li, index: i, frame, value: None });
        }
        if ui.add_enabled(key_here, egui::Button::new(tlf!("Remove {name} key", name = name))).clicked() {
            app.run(Command::RemoveEffectKey { layer: li, index: i, frame });
        }
        if !fx.keys.is_empty() {
            ui.weak(tlf!("{n} keys", n = fx.keys.len()));
        }
    });
}

fn matte_name(m: Option<tooncraft_doc::Matte>) -> String {
    match m {
        None => tl!("Not a matte"),
        Some(tooncraft_doc::Matte::Inside) => tl!("Matte: keep inside"),
        Some(tooncraft_doc::Matte::Outside) => tl!("Matte: keep outside"),
    }
    .to_string()
}

/// The layer's keyed opacity and its matte switch.
fn layer_section(app: &mut ToonApp, ui: &mut egui::Ui, li: usize) {
    let frame = app.frame;
    let Some(layer) = app.engine.project.layers.get(li).cloned() else { return };
    egui::Grid::new("fx-layer").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
        let mut o = layer.opacity_at(frame);
        if num(ui, tl!("Opacity here"), &mut o, 0.0..=1.0, 0.005) {
            if layer.opacity_keys.is_empty() {
                app.run(Command::SetLayer { layer: li, visible: None, locked: None, opacity: Some(o) });
            } else {
                app.run(Command::SetOpacityKey { layer: li, frame, opacity: o });
            }
        }
        let l = ui.label(tl!("Matte"));
        let mut m = layer.matte;
        let mut changed = false;
        egui::ComboBox::from_id_salt("fx-matte")
            .selected_text(matte_name(m))
            .show_ui(ui, |ui| {
                for v in [None, Some(tooncraft_doc::Matte::Inside), Some(tooncraft_doc::Matte::Outside)] {
                    changed |= ui.selectable_value(&mut m, v, matte_name(v)).changed();
                }
            })
            .response
            .labelled_by(l.id);
        ui.end_row();
        if changed {
            app.run(Command::SetLayerMatte { layer: li, matte: m });
        }
    });
    let key_here = layer.opacity_keys.iter().any(|k| k.frame == frame);
    ui.horizontal(|ui| {
        if ui.button(tl!("Key opacity here")).clicked() {
            app.run(Command::SetOpacityKey { layer: li, frame, opacity: layer.opacity_at(frame) });
        }
        if ui.add_enabled(key_here, egui::Button::new(tl!("Remove opacity key"))).clicked() {
            app.run(Command::RemoveOpacityKey { layer: li, frame });
        }
        if !layer.opacity_keys.is_empty() {
            ui.weak(tlf!("{n} keys", n = layer.opacity_keys.len()));
        }
    });
}

fn preset_name(id: &str) -> String {
    match id {
        "film" => tl!("Film"),
        "glow" => tl!("Glow and fringe"),
        "ink" => tl!("Clean ink"),
        "dream" => tl!("Dream"),
        "comic" => tl!("Comic"),
        other => other,
    }
    .to_string()
}

/// Write a layer's effect stack to a `.toonfx` file.
pub fn save_stack(app: &mut ToonApp, effects: &[Effect], path: &std::path::Path) {
    let r = tooncraft_fx::stack_to_json(effects).and_then(|t| craft_atomic::atomic_write(path, t.as_bytes()).map_err(|e| e.to_string()));
    match r {
        Ok(()) => app.set_status(tlf!("Saved {file}", file = path.display())),
        Err(e) => app.set_status(tlf!("Could not save the effects: {error}", error = e)),
    }
}

/// Add the effects saved in a `.toonfx` file to layer `li` (one undo step).
pub fn open_stack(app: &mut ToonApp, li: usize, path: &std::path::Path) {
    let r = std::fs::metadata(path)
        .map_err(|e| e.to_string())
        .and_then(|m| if m.len() > tooncraft_fx::MAX_STACK_FILE as u64 { Err(tl!("the file is too large").to_string()) } else { Ok(()) })
        .and_then(|_| std::fs::read_to_string(path).map_err(|e| e.to_string()))
        .and_then(|t| tooncraft_fx::stack_from_json(&t));
    match r {
        Ok(effects) => {
            let n = effects.len();
            if app.run(Command::AddEffects { layer: li, effects }) {
                app.set_status(tlf!("Added {n} effects", n = n));
            }
        }
        Err(e) => app.set_status(tlf!("Could not open the effects: {error}", error = e)),
    }
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let li = app.layer;
    let Some(layer) = app.engine.project.layers.get(li) else { return };
    let (lname, effects) = (layer.name.clone(), layer.effects.clone());
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    ui.label(tlf!("Effects on {layer}, frame {frame}", layer = lname, frame = app.frame + 1));
    layer_section(app, ui, li);
    ui.separator();
    let l = ui.label(tl!("Add effect"));
    let mut pick: Option<EffectKind> = None;
    egui::ComboBox::from_id_salt("fx-add")
        .selected_text(tl!("Choose…"))
        .show_ui(ui, |ui| {
            for k in EffectKind::catalogue(w, h) {
                if ui.selectable_label(false, effect_name(&k)).clicked() {
                    pick = Some(k);
                }
            }
        })
        .response
        .labelled_by(l.id);
    if let Some(k) = pick {
        app.run(Command::AddEffect { layer: li, effect: Effect::new(k), index: None });
    }
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("Add a look"));
        let mut chosen: Option<Vec<Effect>> = None;
        egui::ComboBox::from_id_salt("fx-preset")
            .selected_text(tl!("Choose…"))
            .show_ui(ui, |ui| {
                for (id, stack) in tooncraft_fx::presets(w, h) {
                    if ui.selectable_label(false, preset_name(id)).clicked() {
                        chosen = Some(stack);
                    }
                }
            })
            .response
            .labelled_by(l.id);
        if let Some(stack) = chosen {
            app.run(Command::AddEffects { layer: li, effects: stack });
        }
        if ui
            .add_enabled(!effects.is_empty(), egui::Button::new(tl!("Save Stack…")))
            .on_hover_text(tl!("Save this layer's effects to a file (.toonfx)"))
            .clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Effect stack"), &["toonfx"]).set_file_name(format!("{lname}.toonfx")).save_file()
        {
            save_stack(app, &effects, &f);
        }
        if ui.button(tl!("Open Stack…")).on_hover_text(tl!("Add the effects saved in a .toonfx file to this layer")).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Effect stack"), &["toonfx"]).pick_file()
        {
            open_stack(app, li, &f);
        }
    });
    ui.separator();
    if effects.is_empty() {
        ui.weak(tl!("No effects on this layer."));
        return;
    }
    let count = effects.len();
    egui::ScrollArea::vertical().id_salt("fx-stack").show(ui, |ui| {
        for (i, fx) in effects.iter().enumerate() {
            ui.push_id(("fx", i), |ui| one_effect(app, ui, li, i, fx, count));
            ui.separator();
        }
    });
}
