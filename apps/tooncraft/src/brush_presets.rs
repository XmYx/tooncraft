//! The preset picker in the tool options bar: the library's brushes of the current kind, each
//! drawn as a sample stroke so the choice is by look, not by name. "Revert" puts the kind's own
//! preset back. Picking one goes through the library (`use_brush`), so it lands in Recent.

use egui::{Color32, Id, Rect, Sense, Ui, WidgetInfo, pos2, vec2};
use tooncraft_brush::{Brush, BrushKind, Images};
use tooncraft_engine::library::Source;

use crate::app::ToonApp;

/// Most presets listed in the picker (the library panel lists them all).
const MAX_SHOWN: usize = 40;
const SAMPLE: (u32, u32) = (132, 26);

/// A sample stroke of `brush` in dark grey, cached per brush.
fn sample(ui: &Ui, key: &str, brush: Brush) -> Option<egui::TextureHandle> {
    let brush = Brush { color: [40, 40, 44, 255], ..brush };
    let id = Id::unique("tooncraft-preset-sample").with(key);
    if let Some((b, t)) = ui.ctx().data(|d| d.get_temp::<(Brush, egui::TextureHandle)>(id))
        && b == brush
    {
        return Some(t);
    }
    let img = tooncraft_engine::preview::render(brush, Images { tip: None, grain: None }, SAMPLE.0, SAMPLE.1)?;
    let t = ui.ctx().load_texture(
        format!("preset-{key}"),
        egui::ColorImage::from_rgba_premultiplied([SAMPLE.0 as usize, SAMPLE.1 as usize], &img.data),
        egui::TextureOptions::LINEAR,
    );
    ui.ctx().data_mut(|d| d.insert_temp(id, (brush, t.clone())));
    Some(t)
}

/// The library brushes of `kind` (built-in ones carry their settings; file ones load on pick).
fn presets(app: &ToonApp, kind: BrushKind) -> Vec<(String, String, Option<Brush>)> {
    app.library
        .library
        .entries()
        .iter()
        .filter(|e| e.kind == kind)
        .take(MAX_SHOWN)
        .map(|e| {
            let brush = match &e.source {
                Source::Builtin(b) => Some(**b),
                Source::File(_) => None,
            };
            (e.id.clone(), crate::brush_library::shown_name(e), brush)
        })
        .collect()
}

/// The "Preset" picker and "Revert" for the brush of `kind`.
pub fn picker(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let list = presets(app, kind);
    let l = ui.label(tl!("Preset"));
    let mut chosen: Option<String> = None;
    egui::ComboBox::from_id_salt(("brush-preset", kind.id()))
        .selected_text(tl!("Choose…"))
        .width(150.0)
        .show_ui(ui, |ui| {
            for (id, name, brush) in &list {
                let (rect, resp) = ui.allocate_exact_size(vec2(SAMPLE.0 as f32 + 8.0, SAMPLE.1 as f32 + 18.0), Sense::click());
                let label = name.clone();
                resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.bg_fill);
                }
                ui.painter().text(rect.left_top() + vec2(4.0, 1.0), egui::Align2::LEFT_TOP, name, egui::FontId::proportional(11.5), ui.visuals().text_color());
                let img = Rect::from_min_size(rect.left_top() + vec2(4.0, 15.0), vec2(SAMPLE.0 as f32, SAMPLE.1 as f32));
                ui.painter().rect_filled(img, 2.0, Color32::from_rgb(246, 243, 236));
                if let Some(b) = brush
                    && let Some(t) = sample(ui, id, *b)
                {
                    ui.painter().image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                }
                if resp.clicked() {
                    chosen = Some(id.clone());
                }
            }
        })
        .response
        .labelled_by(l.id);
    if let Some(id) = chosen {
        crate::brush_library::use_brush(app, &id);
    }
    if ui.button(tl!("Revert")).on_hover_text(tl!("Put this brush's own settings back (the colour stays)")).clicked() {
        let color = app.color;
        *app.brush_mut(kind) = Brush { color, ..Brush::preset(kind) };
        app.set_status(tl!("Brush settings reverted"));
    }
}
