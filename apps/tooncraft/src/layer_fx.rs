//! Layer masks and adjustment layers in the app: the registry commands behind the layer menu
//! and the Effects panel's switches. Every change is an engine command (one undo step).

use tooncraft_engine::Command;

use crate::app::ToonApp;

type Enabled = Result<(), String>;

fn mask(a: &ToonApp) -> Option<tooncraft_doc::LayerMask> {
    a.engine.project.layers.get(a.layer).and_then(|l| l.mask)
}

/// Run one of the `layer.mask_*` and `layer.adjustment` commands on the current layer.
pub fn run(a: &mut ToonApp, _: &egui::Context, id: &'static str) {
    let li = a.layer;
    let Some(layer) = a.engine.project.layers.get(li) else { return };
    let (m, adjustment) = (layer.mask, layer.adjustment);
    let cmd = match (id, m) {
        ("layer.adjustment", _) => Command::SetLayerAdjustment { layer: li, adjustment: !adjustment },
        ("layer.mask_add", None) => Command::AddLayerMask { layer: li, invert: false },
        ("layer.mask_edit", Some(m)) => Command::SetLayerMask { layer: li, enabled: Some(true), invert: None, editing: Some(!m.editing) },
        ("layer.mask_enabled", Some(m)) => Command::SetLayerMask { layer: li, enabled: Some(!m.enabled), invert: None, editing: None },
        ("layer.mask_invert", Some(m)) => Command::SetLayerMask { layer: li, enabled: None, invert: Some(!m.invert), editing: None },
        ("layer.mask_remove", Some(_)) => Command::RemoveLayerMask { layer: li },
        _ => return,
    };
    if a.run(cmd) {
        match mask(a) {
            Some(m) if m.editing => a.set_status(tl!("Painting the layer mask")),
            _ => {}
        }
    }
}

pub fn has_mask(a: &ToonApp) -> Enabled {
    if mask(a).is_some() { Ok(()) } else { Err(tl!("The layer has no mask").to_string()) }
}
pub fn no_mask(a: &ToonApp) -> Enabled {
    if mask(a).is_none() { Ok(()) } else { Err(tl!("The layer already has a mask").to_string()) }
}
pub fn is_adjustment(a: &ToonApp) -> bool {
    a.engine.project.layers.get(a.layer).is_some_and(|l| l.adjustment)
}
pub fn is_editing(a: &ToonApp) -> bool {
    mask(a).is_some_and(|m| m.editing)
}
pub fn is_enabled(a: &ToonApp) -> bool {
    mask(a).is_some_and(|m| m.enabled)
}
pub fn is_inverted(a: &ToonApp) -> bool {
    mask(a).is_some_and(|m| m.invert)
}

/// The layer section's switches in the Effects panel: adjustment layer and the mask.
pub fn section(app: &mut ToonApp, ui: &mut egui::Ui) {
    let Some(layer) = app.engine.project.layers.get(app.layer) else { return };
    let (mut adj, m) = (layer.adjustment, layer.mask);
    let ctx = ui.ctx().clone();
    if ui
        .checkbox(&mut adj, tl!("Adjustment layer"))
        .on_hover_text(tl!("This layer's effects change everything below it, where its drawing has paint (everywhere when it has none)"))
        .changed()
    {
        run(app, &ctx, "layer.adjustment");
    }
    ui.horizontal_wrapped(|ui| match m {
        None => {
            if ui.button(tl!("Add Layer Mask")).on_hover_text(tl!("Paint into the mask to hide parts of this layer")).clicked() {
                run(app, &ctx, "layer.mask_add");
            }
        }
        Some(m) => {
            let (mut edit, mut on, mut inv) = (m.editing, m.enabled, m.invert);
            if ui.toggle_value(&mut edit, tl!("Paint on Layer Mask")).changed() {
                run(app, &ctx, "layer.mask_edit");
            }
            if ui.checkbox(&mut on, tl!("Use Layer Mask")).changed() {
                run(app, &ctx, "layer.mask_enabled");
            }
            if ui.checkbox(&mut inv, tl!("Invert Layer Mask")).changed() {
                run(app, &ctx, "layer.mask_invert");
            }
            if ui.button(tl!("Remove Layer Mask")).clicked() {
                run(app, &ctx, "layer.mask_remove");
            }
        }
    });
    if m.is_some_and(|m| m.editing) {
        ui.weak(tl!("Painting the layer mask: paint hides, the eraser brings the layer back"));
    }
}

/// Launch states for screenshots: `adjustment` puts a desaturating adjustment layer on top that
/// acts where a band is painted across the upper part; `layer_mask` gives the current layer a mask
/// with a band painted through its upper part. True when `id` is one of them.
pub fn launch(app: &mut ToonApp, id: &str) -> bool {
    let (w, h) = (app.engine.project.width() as f32, app.engine.project.height() as f32);
    let band = |app: &mut ToonApp, layer: usize| {
        let brush = tooncraft_brush::Brush { size: h * 0.3, color: [0, 0, 0, 255], ..Default::default() };
        let samples = (0..=40)
            .map(|i| tooncraft_brush::Sample {
                x: w * (0.1 + 0.8 * i as f32 / 40.0),
                y: h * 0.28,
                pressure: 1.0,
                tilt_x: 0.0,
                tilt_y: 0.0,
                time: i as f64 * 0.01,
                rotation: 0.0,
            })
            .collect();
        app.run(Command::Stroke { layer, frame: app.frame, brush, samples });
    };
    match id {
        "adjustment" => {
            let at = app.engine.project.layers.len();
            app.run(Command::AddLayer { name: tl!("Adjustment").to_string(), index: Some(at) });
            let li = app.engine.project.layers.len().saturating_sub(1);
            app.layer = li;
            let fx = tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::HueSaturation { hue: 0.0, saturation: -1.0, lightness: 0.0 });
            app.run(Command::AddEffect { layer: li, effect: fx, index: None });
            app.run(Command::SetLayerAdjustment { layer: li, adjustment: true });
            band(app, li);
            true
        }
        "layer_mask" => {
            let li = app.layer;
            app.run(Command::AddLayerMask { layer: li, invert: false });
            band(app, li);
            true
        }
        _ => false,
    }
}
