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

/// The name of the preset picked last for `kind` (shown on the dropdown).
fn picked_id(kind: BrushKind) -> Id {
    Id::unique("tooncraft-preset-picked").with(kind.id())
}

/// The preset dropdown for the brush of `kind`: it shows the current brush as a stroke and the
/// preset picked last; it opens on the library's presets, each drawn as a stroke, and Revert.
pub fn picker(app: &mut ToonApp, ui: &mut Ui, kind: BrushKind) {
    let list = presets(app, kind);
    let pal = crate::theme::palette(app.light_theme);
    let current = *app.brush_mut(kind);
    let picked = ui.ctx().data(|d| d.get_temp::<String>(picked_id(kind)));
    let name = picked.unwrap_or_else(|| tl!("Own settings").to_string());
    let font = egui::FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(name.clone(), font, pal.text);
    let stroke_w = 64.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(stroke_w + galley.size().x + 30.0, 22.0), Sense::click());
    ui.painter().rect(rect, 4.0, pal.raised, egui::Stroke::NONE, egui::StrokeKind::Inside);
    if resp.hovered() {
        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, pal.weak), egui::StrokeKind::Inside);
    }
    let img = Rect::from_min_size(rect.left_top() + vec2(4.0, 3.0), vec2(stroke_w, 16.0));
    if let Some(t) = sample(ui, &format!("current-{}", kind.id()), current) {
        // the stroke on the bar's own colour: light strokes stay readable on the dark bar
        ui.painter().rect_filled(img, 2.0, Color32::from_rgb(232, 229, 222));
        ui.painter().image(t.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    ui.painter().galley(pos2(img.right() + 6.0, rect.center().y - galley.size().y * 0.5), galley, pal.text);
    crate::tool_options::caret(ui.painter(), pos2(rect.right() - 9.0, rect.center().y), pal.weak);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
    }
    let label = tl!("Preset").to_string();
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::ComboBox, true, label.clone()));
    let resp = resp.on_hover_text(tlf!("Brush preset: {name}", name = name));
    let mut chosen: Option<(String, String)> = None;
    let mut revert = false;
    egui::Popup::menu(&resp).show(|ui| {
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for (id, name, brush) in &list {
                let (rect, resp) = ui.allocate_exact_size(vec2(SAMPLE.0 as f32 + 8.0, SAMPLE.1 as f32 + 18.0), Sense::click());
                let label = name.clone();
                resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
                if resp.hovered() || resp.has_focus() {
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
                    chosen = Some((id.clone(), name.clone()));
                    ui.close();
                }
            }
        });
        ui.separator();
        if ui.button(tl!("Revert")).on_hover_text(tl!("Put this brush's own settings back (the colour stays)")).clicked() {
            revert = true;
            ui.close();
        }
    });
    if let Some((id, name)) = chosen {
        crate::brush_library::use_brush(app, &id);
        ui.ctx().data_mut(|d| d.insert_temp(picked_id(kind), name));
    }
    if revert {
        let color = app.color;
        *app.brush_mut(kind) = Brush { color, ..Brush::preset(kind) };
        ui.ctx().data_mut(|d| d.remove::<String>(picked_id(kind)));
        app.set_status(tl!("Brush settings reverted"));
    }
}
