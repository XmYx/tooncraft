//! UI tests for the custom tool palettes and the radial menu: the Tool Palette panel runs and edits
//! a palette from the keyboard, and the radial menu opens on its key, runs a slice by pointer or
//! by arrows and Enter, and closes on Escape or a click outside. Nothing is written to the user's
//! settings (the preferences have no store in tests).

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;
use crate::tool_palettes::{Palette, Palettes};

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h
}

fn press(h: &mut Harness<'static, ToonApp>, key: Key) {
    h.key_press_modifiers(Modifiers::NONE, key);
    h.run();
}

fn focus_press(h: &mut Harness<'static, ToonApp>, role: egui::Role, label: &str, key: Key) {
    h.get(By::new().role(role).label(label)).focus();
    h.run();
    press(h, key);
}

fn open_palette_panel(h: &mut Harness<'static, ToonApp>) {
    h.state_mut().workspace.raise(Pane::Palette);
    h.run();
    h.run();
}

#[test]
fn a_palette_button_runs_its_command_like_the_menu_item() {
    let mut h = harness();
    open_palette_panel(&mut h);
    assert_ne!(h.state().tool, Tool::Eraser);
    focus_press(&mut h, egui::Role::Button, "Eraser", Key::Enter);
    assert_eq!(h.state().tool, Tool::Eraser, "the palette's Eraser button picked the eraser");
    let onion = h.state().engine.project.onion.enabled;
    let name = crate::commands::get("view.onion").map(|c| c.label()).unwrap_or_default();
    focus_press(&mut h, egui::Role::Button, &name, Key::Enter);
    assert_ne!(h.state().engine.project.onion.enabled, onion, "a toggle command toggles");
}

#[test]
fn a_palette_is_put_together_from_the_keyboard() {
    let mut h = harness();
    open_palette_panel(&mut h);
    focus_press(&mut h, egui::Role::Button, "New palette", Key::Enter);
    assert_eq!(h.state().preferences.prefs.palettes.sets.len(), 3);
    assert_eq!(h.state().preferences.prefs.palettes.shown, 2, "the new palette is shown, in the editor");
    // add two commands by searching for them
    for (q, label) in [("gradient", "Add Gradient"), ("invert", "Add Invert selection")] {
        let search = By::new().role(egui::Role::SearchInput).label("Find a command to add");
        h.get(search.clone()).focus();
        h.run();
        h.get(search).type_text(q);
        h.run();
        focus_press(&mut h, egui::Role::Button, label, Key::Enter);
    }
    let cmds = |h: &Harness<'static, ToonApp>| h.state().preferences.prefs.palettes.sets[2].commands.clone();
    assert_eq!(cmds(&h), vec!["tool.gradient".to_string(), "select.invert".to_string()]);
    // reorder and remove
    focus_press(&mut h, egui::Role::Button, "Move Invert selection earlier", Key::Enter);
    assert_eq!(cmds(&h), vec!["select.invert".to_string(), "tool.gradient".to_string()]);
    focus_press(&mut h, egui::Role::Button, "Remove Gradient", Key::Enter);
    assert_eq!(cmds(&h), vec!["select.invert".to_string()]);
    // rename, and make it the radial menu's palette
    let name = By::new().role(egui::Role::TextInput).label("Palette name");
    h.get(name.clone()).focus();
    h.run();
    // Ctrl+A in a text field selects its text (the app's Select All leaves it alone)
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run();
    h.get(name).type_text("Mine");
    h.run();
    assert_eq!(h.state().preferences.prefs.palettes.sets[2].name, "Mine");
    focus_press(&mut h, egui::Role::CheckBox, "Use in the radial menu", Key::Space);
    assert_eq!(h.state().preferences.prefs.palettes.radial, 2);
    assert_eq!(h.state().preferences.prefs.palettes.radial_items(), vec!["select.invert"]);
    // done editing: the palette's button is there and runs
    focus_press(&mut h, egui::Role::Button, "Edit palette", Key::Enter);
    assert!(h.state().selection.is_none());
    focus_press(&mut h, egui::Role::Button, "Invert selection", Key::Enter);
    assert!(h.state().selection.is_some(), "inverting nothing selects the whole drawing");
    // delete it: the radial menu falls back to the first palette
    focus_press(&mut h, egui::Role::Button, "Edit palette", Key::Enter);
    focus_press(&mut h, egui::Role::Button, "Delete Palette", Key::Enter);
    let p = &h.state().preferences.prefs.palettes;
    assert_eq!((p.sets.len(), p.radial), (2, 0));
}

#[test]
fn the_radial_menu_opens_on_its_key_and_runs_a_slice_with_arrows_and_enter() {
    let mut h = harness();
    h.state_mut().preferences.prefs.palettes = Palettes {
        sets: vec![Palette { name: "R".into(), commands: vec!["tool.pencil".into(), "tool.eraser".into(), "tool.fill".into()] }],
        shown: 0,
        radial: 0,
    };
    // the pointer over the canvas, then the key
    let at = h.state().workspace.canvas_rect.center();
    h.hover_at(at);
    h.run();
    press(&mut h, Key::V);
    h.run();
    assert!(crate::tool_palettes::radial_open(&h.ctx), "V opens the radial menu");
    let c = crate::tool_palettes::radial_centre(&h.ctx).expect("drawn");
    assert!((c - at).length() < 1.0, "round the pointer: {c:?} {at:?}");
    for name in ["Pencil", "Eraser", "Paint bucket"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
    // the keyboard starts on the first slice; two steps clockwise is the bucket
    press(&mut h, Key::ArrowRight);
    press(&mut h, Key::ArrowRight);
    press(&mut h, Key::Enter);
    h.run();
    assert_eq!(h.state().tool, Tool::Fill);
    assert!(!crate::tool_palettes::radial_open(&h.ctx), "running a slice closes the menu");
    // Escape closes it without running anything
    press(&mut h, Key::V);
    h.run();
    assert!(crate::tool_palettes::radial_open(&h.ctx));
    press(&mut h, Key::ArrowLeft);
    press(&mut h, Key::Escape);
    assert!(!crate::tool_palettes::radial_open(&h.ctx));
    assert_eq!(h.state().tool, Tool::Fill);
}

#[test]
fn the_radial_menu_runs_the_slice_clicked_and_a_click_outside_closes_it() {
    let mut h = harness();
    let at = h.state().workspace.canvas_rect.center();
    h.hover_at(at);
    h.run();
    press(&mut h, Key::V);
    h.run();
    assert!(crate::tool_palettes::radial_open(&h.ctx));
    // the default palette's third slice is the eraser
    assert_eq!(h.state().preferences.prefs.palettes.radial_items()[2], "tool.eraser");
    h.get(By::new().role(egui::Role::Button).label("Eraser")).click();
    h.run();
    h.run();
    assert_eq!(h.state().tool, Tool::Eraser);
    assert!(!crate::tool_palettes::radial_open(&h.ctx));
    // a click far outside the ring closes it, and the tool stays
    press(&mut h, Key::V);
    h.run();
    assert!(crate::tool_palettes::radial_open(&h.ctx));
    let far = egui::pos2(30.0, 980.0);
    h.hover_at(far);
    h.run();
    h.drag_at(far);
    h.run();
    h.drop_at(far);
    h.run();
    h.run();
    assert!(!crate::tool_palettes::radial_open(&h.ctx));
    assert_eq!(h.state().tool, Tool::Eraser);
}

#[test]
fn an_empty_radial_palette_says_so_instead_of_opening() {
    let mut h = harness();
    h.state_mut().preferences.prefs.palettes = Palettes { sets: vec![Palette { name: "Empty".into(), commands: Vec::new() }], shown: 0, radial: 0 };
    press(&mut h, Key::V);
    h.run();
    assert!(!crate::tool_palettes::radial_open(&h.ctx));
    assert!(h.state().status.contains("radial menu"), "{}", h.state().status);
}
