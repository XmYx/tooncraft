//! Menu items built from the command registry ([`crate::commands`]): an icon, the label and the
//! key, disabled with the reason in the tooltip when the command cannot run, and a tick for on/off
//! commands. [`context`] builds a whole right-click menu from a list of command ids.

use egui::{Atom, Ui, vec2};

use crate::app::ToonApp;
use crate::commands;

/// In a [`context`] list: a separator line.
pub const SEP: &str = "|";

/// One menu item for the command `id`. Returns whether it ran.
pub fn item(ui: &mut Ui, app: &mut ToonApp, id: &str) -> bool {
    let Some(c) = commands::get(id) else {
        log::warn!("menu: unknown command {id}");
        return false;
    };
    let label = c.menu_label(app);
    let key = c.key_text(app);
    let enabled = (c.enabled)(app);
    let checked = c.checked.map(|f| f(app));
    let icon_id = egui::IdSalt::new(("tooncraft-menu-icon", c.id));
    let side = crate::theme::metrics::ICON;
    let mut button = egui::Button::new((Atom::custom(icon_id, vec2(side, side)), label.clone())).shortcut_text(key.clone());
    if let Some(on) = checked {
        button = button.selected(on);
    }
    let shown = ui.add_enabled_ui(enabled.is_ok(), |ui| button.atom_ui(ui)).inner;
    let resp = shown.response.clone();
    if let Some(rect) = shown.rect(icon_id) {
        let pal = crate::theme::palette(!ui.visuals().dark_mode);
        let color = if enabled.is_err() {
            pal.weak.gamma_multiply(0.5)
        } else if checked == Some(true) {
            pal.accent
        } else {
            pal.weak
        };
        if let Some(icon) = c.icon {
            crate::icons::paint(ui.painter(), rect, icon, color, checked == Some(true));
        } else if checked == Some(true) {
            crate::icons::paint(ui.painter(), rect, crate::icons::Icon::MarkKey, color, true);
        }
    }
    // the accessible name is the label alone; toggles say whether they are on
    let (name, on) = (label.clone(), checked);
    let is_enabled = enabled.is_ok();
    resp.widget_info(|| match on {
        Some(on) => egui::WidgetInfo::selected(egui::Role::CheckBox, is_enabled, on, name.clone()),
        None => egui::WidgetInfo::labeled(egui::Role::Button, is_enabled, name.clone()),
    });
    let resp = match &enabled {
        Err(why) => resp.on_disabled_hover_text(why.clone()),
        Ok(()) if !key.is_empty() => resp.on_hover_text(format!("{label} ({key})")),
        Ok(()) => resp,
    };
    if resp.clicked() {
        let ctx = ui.ctx().clone();
        let ran = commands::run(app, &ctx, c.id);
        ui.close();
        return ran;
    }
    false
}

/// A menu of registry commands: `ids` in order, with [`SEP`] for a separator. Returns the id of
/// the command that ran, if any.
pub fn context(ui: &mut Ui, app: &mut ToonApp, ids: &[&str]) -> Option<&'static str> {
    let mut ran = None;
    for id in ids {
        if *id == SEP {
            ui.separator();
        } else if item(ui, app, id) {
            ran = commands::get(id).map(|c| c.id);
        }
    }
    ran
}

#[cfg(test)]
mod tests {
    use egui::Key;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{By, NodeT, Queryable};

    use crate::app::ToonApp;
    use crate::args::Cli;

    fn harness() -> Harness<'static, ToonApp> {
        let ctx = egui::Context::default();
        let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
        let mut h = Harness::builder().with_size(egui::vec2(500.0, 400.0)).with_accessibility_check(true).build_ui_state(
            |ui, app: &mut ToonApp| {
                let r = ui.button("Menu");
                egui::Popup::context_menu(&r).show(|ui| {
                    super::context(ui, app, &["view.onion", "view.mirror", super::SEP, "edit.undo", "select.flip_h", "frame.last"]);
                });
            },
            app,
        );
        h.run();
        h
    }

    #[test]
    fn a_context_menu_lists_registry_commands_with_keys_and_reasons() {
        let mut h = harness();
        h.get_by_label("Menu").click_secondary();
        h.run();
        // toggles are ticks with their state; the label is the accessible name, without the key
        let onion = h.get(By::new().role(egui::Role::CheckBox).label("Onion skin on / off"));
        let was_on = h.state().engine.project.onion.enabled;
        assert_eq!(onion.accesskit_node().toggled().map(|t| t == egui::accesskit::Toggled::True), Some(was_on));
        // unavailable commands stay in the menu, disabled
        assert!(h.get_by_label("Undo").accesskit_node().is_disabled(), "nothing to undo");
        assert!(h.get_by_label("Flip Horizontally").accesskit_node().is_disabled(), "nothing selected");
        // from the keyboard: focus an item and press Enter
        h.get(By::new().role(egui::Role::Button).label("Last frame")).focus();
        h.run();
        h.key_press(Key::Enter);
        h.run();
        let a = h.state();
        assert_eq!(a.frame, a.engine.project.length().saturating_sub(1), "Enter ran the command");
    }

    #[test]
    fn a_tick_runs_its_command() {
        let mut h = harness();
        let before = h.state().engine.project.onion.enabled;
        h.get_by_label("Menu").click_secondary();
        h.run();
        h.get(By::new().role(egui::Role::CheckBox).label("Onion skin on / off")).focus();
        h.run();
        h.key_press(Key::Space);
        h.run();
        assert_ne!(h.state().engine.project.onion.enabled, before);
    }
}
