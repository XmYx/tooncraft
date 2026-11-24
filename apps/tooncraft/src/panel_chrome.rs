//! Panel tab chrome for the dock: every tab shows the panel's glyph and name, and only the glyph
//! when its stack is narrower than 220 px (the name stays the tab's accessible name and its
//! tooltip). Each stack ends in a `⋯` menu: the shown panel's own commands, then Float, Move To
//! and Close.
//!
//! craft-ui draws tab names as text only, so a tab's name is laid out with room in front of it and
//! the glyph is painted into that room after the dock is drawn, on the tab's own layer.

use std::cell::RefCell;
use std::collections::HashSet;

use craft_ui::docking::{Action, DockContent, Permissions, Placement};
use egui::{Color32, FontId, Ui, vec2};

use crate::app::ToonApp;
use crate::dock::Pane;
use crate::icons::{Icon, IconButton};

/// A stack narrower than this shows glyphs only.
pub const NARROW: f32 = 220.0;
/// Room in front of a tab's name for its glyph.
const GLYPH_ROOM: f32 = 20.0;
/// The `⋯` button's width at the end of the tab strip.
const MENU_WIDTH: f32 = 26.0;

/// Whether a stack whose tab strip is `strip_width` wide shows glyphs only.
pub fn is_narrow(strip_width: f32) -> bool {
    !strip_width.is_finite() || strip_width < NARROW
}

/// Whether `panels` would have their names cut short in a strip `strip_width` wide (an estimate:
/// about 7 px a letter). Cut names leave only "…" beside the glyph, so such a stack shows glyphs.
pub fn crowded(panels: &[Pane], strip_width: f32) -> bool {
    let room = strip_width - MENU_WIDTH - 16.0;
    let wanted: f32 = panels.iter().map(|p| p.label().chars().count() as f32 * 7.0 + GLYPH_ROOM + 16.0).sum();
    wanted > room
}

/// The dock's id (its tabs' ids derive from it).
pub fn dock_id() -> egui::Id {
    egui::Id::unique("tooncraft-dock")
}

/// The id craft-ui gives `pane`'s tab.
fn tab_id(pane: Pane) -> egui::Id {
    dock_id().with(("tab", &pane))
}

/// The dock's content: panel bodies, tab labels with room for a glyph, and the `⋯` menus.
pub struct Chrome<'a> {
    pub app: &'a mut ToonApp,
    /// Panels of stacks too narrow for names this frame.
    narrow: RefCell<HashSet<Pane>>,
    /// Actions picked in the `⋯` menus, applied with the dock's own.
    pub queued: Vec<Action<Pane>>,
}

impl<'a> Chrome<'a> {
    pub fn new(app: &'a mut ToonApp) -> Self {
        Self { app, narrow: RefCell::new(HashSet::new()), queued: Vec::new() }
    }

    /// The panels whose tabs showed glyphs only this frame.
    pub fn glyphs_only(&self) -> HashSet<Pane> {
        self.narrow.borrow().clone()
    }
}

impl DockContent<Pane> for Chrome<'_> {
    fn body(&mut self, ui: &mut Ui, pane: &Pane) {
        let app = &mut *self.app;
        match pane {
            Pane::Canvas => {
                crate::canvas_header::strip(app, ui);
                crate::canvas::show(app, ui)
            }
            Pane::Timeline => crate::timeline::show(app, ui),
            Pane::Colour => crate::panels::colour(app, ui),
            Pane::Brush => crate::panels::brush(app, ui),
            Pane::Layers => crate::panels::layers(app, ui),
            Pane::Onion => crate::panels::onion(app, ui),
            Pane::Camera => crate::panels::camera(app, ui),
            Pane::Pegs => crate::pegs::panel(app, ui),
            Pane::Library => crate::brush_library::panel(app, ui),
            Pane::Effects => crate::effects_panel::show(app, ui),
            Pane::Sound => crate::sound_panel::show(app, ui),
            Pane::LipSync => crate::lipsync_panel::show(app, ui),
            Pane::Timing => crate::timing_panel::panel(app, ui),
            Pane::History => crate::history_panel::panel(app, ui),
            Pane::Drawings => crate::drawing_library::panel(app, ui),
            Pane::Curves => crate::curves::panel(app, ui),
        }
    }

    fn logical_label(&self, pane: &Pane) -> String {
        pane.label()
    }

    fn permissions(&self, pane: &Pane) -> Permissions {
        pane.permissions()
    }

    fn paint_icon(&mut self, painter: &egui::Painter, rect: egui::Rect, pane: &Pane, color: Color32) -> bool {
        crate::icons::paint(painter, rect, crate::icons::for_pane(*pane), color, false);
        true
    }

    fn label(&mut self, _ui: &Ui, pane: &Pane, logical: &str, font: &FontId, color: Color32) -> egui::WidgetText {
        let narrow = self.narrow.borrow().contains(pane);
        let text = if narrow { " " } else { logical };
        let mut job = egui::text::LayoutJob::default();
        job.append(text, GLYPH_ROOM, egui::TextFormat::simple(font.clone(), color));
        job.into()
    }

    fn group_header_width(&self, panels: &[Pane], _active: &Pane, _floating: bool, strip_width: f32) -> f32 {
        let glyphs_only = is_narrow(strip_width) || crowded(panels, strip_width);
        let mut narrow = self.narrow.borrow_mut();
        for p in panels {
            if glyphs_only {
                narrow.insert(*p);
            } else {
                narrow.remove(p);
            }
        }
        MENU_WIDTH
    }

    fn group_header(&mut self, ui: &mut Ui, _panels: &[Pane], active: &Pane, floating: bool) {
        let pane = *active;
        let name = tlf!("{panel} panel menu", panel = pane.label());
        let resp = ui.add(IconButton::new(Icon::More, name).size(ui.max_rect().height().min(24.0)));
        egui::Popup::menu(&resp).show(|ui| stack_menu(self, ui, pane, floating));
    }

    /// The tab's right-click menu: Reset Panel (craft-ui adds Float, Move To and Close after it).
    fn panel_menu(&mut self, ui: &mut Ui, pane: &Pane) {
        crate::context_menus::panel_tab(self.app, ui, *pane);
    }
}

/// The `⋯` menu of the stack showing `pane`.
fn stack_menu(c: &mut Chrome<'_>, ui: &mut Ui, pane: Pane, floating: bool) {
    ui.set_min_width(180.0);
    if own_commands(c.app, ui, pane) {
        ui.separator();
    }
    let p = pane.permissions();
    if p.movable && p.floatable && !floating && ui.button(tl!("Float")).on_hover_text(tl!("A window of its own")).clicked() {
        let at = ui.ctx().pointer_latest_pos().unwrap_or(egui::pos2(200.0, 120.0)) - vec2(320.0, 0.0);
        c.queued.push(Action::Float { panel: pane, rect: [at.x.max(0.0), at.y.max(0.0), 320.0, 380.0] });
        ui.close();
    }
    let targets: Vec<Pane> =
        if p.movable { c.app.workspace.layout.move_targets(&pane, |q| q.permissions()).into_iter().copied().collect() } else { Vec::new() };
    if !targets.is_empty() {
        ui.menu_button(tl!("Move To"), |ui| {
            for t in targets {
                if ui.button(tlf!("Beside {panel}", panel = t.label())).clicked() {
                    c.queued.push(Action::Move { panel: pane, anchor: t, placement: Placement::Tab { before: None } });
                    ui.close();
                }
            }
        });
    }
    if p.closable && ui.button(tl!("Close")).on_hover_text(tl!("Window puts it back")).clicked() {
        c.queued.push(Action::Close { panel: pane });
        ui.close();
    }
    ui.separator();
    crate::context_menus::panel_tab(c.app, ui, pane);
}

/// The panel's own commands at the top of its `⋯` menu; false when it has none.
fn own_commands(app: &mut ToonApp, ui: &mut Ui, pane: Pane) -> bool {
    match pane {
        Pane::Timeline => {
            ui.checkbox(&mut app.tl.vertical, tl!("Vertical Exposure Sheet")).on_hover_text(tl!("Frames down the page, layers as columns"));
        }
        Pane::Colour => {
            ui.checkbox(&mut app.colour.wheel, tl!("Hue Wheel"));
            ui.checkbox(&mut app.colour.window, tl!("Picker Window"));
        }
        Pane::Onion => {
            ui.checkbox(&mut app.engine.project.onion.enabled, tl!("Onion Skin"));
        }
        _ => return false,
    }
    true
}

/// Paint each visible tab's glyph into the room left in front of its name.
pub fn paint_tab_glyphs(app: &ToonApp, ctx: &egui::Context, glyphs_only: &HashSet<Pane>) {
    let pal = crate::theme::palette(app.light_theme);
    for pane in app.workspace.layout.panels() {
        let Some(r) = ctx.read_response(tab_id(*pane)) else { continue };
        if !r.rect.is_positive() {
            continue;
        }
        let side = crate::theme::metrics::ICON.min(r.rect.height() - 4.0);
        let glyph = egui::Rect::from_center_size(egui::pos2(r.rect.left() + 6.0 + side * 0.5, r.rect.center().y), vec2(side, side));
        if !r.rect.contains_rect(glyph) {
            continue;
        }
        let painter = ctx.layer_painter(r.layer_id).with_clip_rect(r.rect);
        crate::icons::paint(&painter, glyph, crate::icons::for_pane(*pane), pal.weak, false);
        // a tab showing its glyph alone names its panel on hover
        if glyphs_only.contains(pane) {
            let _ = r.on_hover_text(pane.label());
        }
    }
}

/// The rectangle of `pane`'s tab drawn last frame (tests).
#[cfg(test)]
pub fn tab_rect(ctx: &egui::Context, pane: Pane) -> Option<egui::Rect> {
    ctx.read_response(tab_id(pane)).map(|r| r.rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacks_under_220_px_show_glyphs_only() {
        assert!(is_narrow(219.0));
        assert!(!is_narrow(220.0));
        assert!(is_narrow(f32::NAN), "a broken width hides names rather than overflowing");
        assert!(!crowded(&[Pane::Timeline, Pane::Curves], 900.0));
        assert!(crowded(&[Pane::Layers, Pane::Onion, Pane::Camera, Pane::Pegs, Pane::Effects, Pane::Sound], 300.0));
    }
}
