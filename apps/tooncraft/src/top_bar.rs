//! The top bar (32 px): the app mark and the menus on the left; on the right, the workspace
//! segments (Draw · Time · Ink & Paint · Sheet), command search and undo/redo. Every control has
//! a name for assistive technology and a tooltip with its key; the same commands stay in the
//! menus and on their keys.
//!
//! The current frame, the in and out points and the frame rate are typed in the Timeline header;
//! the shot length is in its ⋯ menu.

use egui::{Color32, Sense, Stroke, vec2};

use crate::app::ToonApp;
use crate::workspaces::Kind;

/// The bar's height.
pub const HEIGHT: f32 = 32.0;

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let full = ui.max_rect();
    // the right-hand group first, from the right edge; the menus take what is left
    let right = ui
        .scope_builder(egui::UiBuilder::new().max_rect(full).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            undo_redo(app, ui);
            search(app, ui);
            segments(app, ui);
            ui.separator();
            ui.min_rect()
        })
        .inner;
    let left = egui::Rect::from_min_max(full.min, egui::pos2((right.left() - 4.0).max(full.left()), full.bottom()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        mark(app, ui);
        app.menu_bar(ui);
    });
}

/// The app mark: a small accent diamond and the name.
fn mark(app: &ToonApp, ui: &mut egui::Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    let c = r.center();
    let d = 5.5;
    ui.painter().add(egui::Shape::convex_polygon(vec![c + vec2(0.0, -d), c + vec2(d, 0.0), c + vec2(0.0, d), c + vec2(-d, 0.0)], pal.accent, Stroke::NONE));
    ui.label(egui::RichText::new("ToonCraft").strong());
}

/// The workspace segments. Each is a radio button for assistive technology; Tab reaches them
/// and Space or Enter picks one. Right-click a segment for Reset and Save As.
fn segments(app: &mut ToonApp, ui: &mut egui::Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let font = egui::TextStyle::Button.resolve(ui.style());
    let galleys: Vec<_> = Kind::ALL.iter().map(|k| ui.painter().layout_no_wrap(k.label(), font.clone(), pal.text)).collect();
    let pad = 10.0;
    let width: f32 = galleys.iter().map(|g| g.size().x + 2.0 * pad).sum::<f32>() + 4.0;
    let (track, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    ui.painter().rect_filled(track, 4.0, pal.bg);
    let mut x = track.left() + 2.0;
    let mut picked = None;
    for (kind, galley) in Kind::ALL.into_iter().zip(galleys) {
        let w = galley.size().x + 2.0 * pad;
        let r = egui::Rect::from_min_size(egui::pos2(x, track.top() + 2.0), vec2(w, track.height() - 4.0));
        x += w;
        let id = egui::Id::unique("tooncraft-workspace-segment").with(kind.name());
        let resp = ui.interact(r, id, Sense::click());
        let on = app.workspaces.active == Some(kind);
        let fill = if on {
            pal.raised
        } else if resp.hovered() {
            pal.raised.gamma_multiply(0.5)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(r, 3.0, fill);
        if on {
            // the accent underline marks the active workspace
            let y = r.bottom() - 1.0;
            ui.painter().line_segment([egui::pos2(r.left() + 6.0, y), egui::pos2(r.right() - 6.0, y)], Stroke::new(2.0, pal.accent));
        }
        let colour = if on { pal.text } else { pal.weak };
        ui.painter().galley(r.center() - galley.size() * 0.5, galley, colour);
        if resp.has_focus() {
            ui.painter().rect_stroke(r, 3.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
        }
        let name = tlf!("{workspace} workspace", workspace = kind.label());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name.clone()));
        let tip = if app.workspaces.is_edited(kind) { tlf!("{hint} (changed; right-click to reset)", hint = kind.hint()) } else { kind.hint() };
        let resp = resp.on_hover_text(tip);
        if resp.clicked() {
            picked = Some(kind);
        }
        resp.context_menu(|ui| {
            if ui.button(tl!("Reset Workspace")).on_hover_text(tl!("Back to its first layout")).clicked() {
                if app.workspaces.active != Some(kind) {
                    crate::workspaces::switch(app, kind);
                }
                crate::workspaces::reset(app);
                ui.close();
            }
            ui.separator();
            ui.horizontal(|ui| {
                let l = ui.label(tl!("Save Workspace As…"));
                let field = ui.add(egui::TextEdit::singleline(&mut app.workspaces.save_as).hint_text(tl!("name")).desired_width(110.0)).labelled_by(l.id);
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button(tl!("Save")).clicked() || enter) && crate::workspaces::save_as(app, &app.workspaces.save_as.clone()) {
                    ui.close();
                }
            });
        });
    }
    if let Some(kind) = picked
        && app.workspaces.active != Some(kind)
    {
        crate::workspaces::switch(app, kind);
    }
}

/// The command search field: a click (or its key) opens Find Command.
fn search(app: &mut ToonApp, ui: &mut egui::Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let key = app.keys.text("app.find_command");
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text = ui.painter().layout_no_wrap(tl!("Search commands").to_string(), font.clone(), pal.weak);
    let keyg = ui.painter().layout_no_wrap(key.clone(), egui::FontId::new((font.size * 0.85).max(11.0), font.family.clone()), pal.weak);
    let (r, resp) = ui.allocate_exact_size(vec2(text.size().x + keyg.size().x + 48.0, 24.0), Sense::click());
    ui.painter().rect(r, 4.0, pal.bg, Stroke::new(1.0, if resp.hovered() { pal.weak } else { pal.raised }), egui::StrokeKind::Inside);
    let glyph = egui::Rect::from_center_size(r.left_center() + vec2(12.0, 0.0), vec2(crate::theme::metrics::ICON, crate::theme::metrics::ICON));
    crate::icons::paint(ui.painter(), glyph, crate::icons::Icon::Search, pal.weak, false);
    ui.painter().galley(egui::pos2(r.left() + 24.0, r.center().y - text.size().y * 0.5), text, pal.weak);
    let kr = egui::Rect::from_min_size(egui::pos2(r.right() - keyg.size().x - 10.0, r.center().y - keyg.size().y * 0.5), keyg.size());
    ui.painter().rect_stroke(kr.expand2(vec2(3.0, 1.0)), 3.0, Stroke::new(1.0, pal.raised), egui::StrokeKind::Inside);
    ui.painter().galley(kr.min, keyg, pal.weak);
    if resp.has_focus() {
        ui.painter().rect_stroke(r, 4.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
    }
    let name = tl!("Search commands").to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
    if resp.on_hover_text(tlf!("Find a command by its name or key ({key})", key = key)).clicked() {
        app.find.open = true;
    }
}

/// Undo and redo (laid out right to left: redo first).
fn undo_redo(app: &mut ToonApp, ui: &mut egui::Ui) {
    use crate::icons::{Icon, IconButton};
    let side = 24.0;
    let redo = IconButton::new(Icon::Redo, tl!("Redo step")).shortcut("Ctrl+Shift+Z").size(side);
    let why = (!app.engine.can_redo()).then(|| tl!("Nothing to redo").to_string());
    if ui.add(redo.disabled(why)).clicked() {
        app.redo();
    }
    let undo = IconButton::new(Icon::Undo, tl!("Undo step")).shortcut("Ctrl+Z").size(side);
    let why = (!app.engine.can_undo()).then(|| tl!("Nothing to undo").to_string());
    if ui.add(undo.disabled(why)).clicked() {
        app.undo();
    }
}
