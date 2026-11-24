//! A kittest test for each right-click menu of design §5, driven from the keyboard: focus the
//! widget, open its menu with Shift+F10 or the Menu key, focus an item and press Enter. Disabled
//! items stay in the menu with their reason. The accessibility check is on.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};
use tooncraft_doc::palette::Palette;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness_with(pane: Option<Pane>) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1200.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    if let Some(p) = pane {
        h.state_mut().workspace.raise(p);
    }
    h.run();
    h.run();
    h
}

/// Focus the node, then open its menu with Shift+F10 (or the Menu key).
fn open_menu(h: &mut Harness<'static, ToonApp>, by: By<'_>, menu_key: bool) {
    h.get(by).focus();
    h.run();
    if menu_key {
        h.key_press(Key::Menu);
    } else {
        h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    }
    h.run();
    h.run();
}

/// Focus a menu item and press Enter.
fn choose(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

/// Focus a submenu's button and open it with the right arrow key.
fn open_sub(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
}

fn disabled(h: &Harness<'static, ToonApp>, label: &str) -> bool {
    h.get_by_label(label).accesskit_node().is_disabled()
}

fn canvas() -> By<'static> {
    By::new().role(egui::Role::Canvas).label_contains("Canvas")
}

/// A drawing on frames 1 to 3 of the bottom layer, and one on frame 4.
fn two_drawings(h: &mut Harness<'static, ToonApp>) {
    let a = h.state_mut();
    assert!(a.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(a.run(Command::SetExposure { layer: 0, frame: 0, length: 3 }));
    assert!(a.run(Command::NewDrawing { layer: 0, frame: 3 }));
    (a.layer, a.frame) = (0, 0);
    h.run();
}

#[test]
fn canvas_menu_flips_the_whole_drawing_and_says_why_cut_is_off() {
    let mut h = harness_with(None);
    two_drawings(&mut h);
    open_menu(&mut h, canvas(), false);
    assert!(disabled(&h, "Cut Pixels"), "nothing selected");
    let steps = h.state().engine.undo_len();
    open_sub(&mut h, "Flip");
    choose(&mut h, "Flip Drawing Horizontally");
    assert_eq!(h.state().engine.undo_len(), steps + 1, "one undo step");
    // the Tool submenu picks a tool
    open_menu(&mut h, canvas(), true);
    open_sub(&mut h, "Tool");
    choose(&mut h, "Paint bucket");
    assert_eq!(h.state().tool, crate::app::Tool::Fill);
}

#[test]
fn selection_menu_fills_the_selection() {
    let mut h = harness_with(None);
    two_drawings(&mut h);
    {
        let a = h.state_mut();
        crate::selection_tools::set_rings(a, vec![vec![(10.0, 10.0), (40.0, 10.0), (40.0, 40.0), (10.0, 40.0)]]);
        a.color = [20, 30, 200, 255];
    }
    h.run();
    open_menu(&mut h, canvas(), false);
    h.get_by_label("Stroke Selection");
    choose(&mut h, "Fill Selection");
    assert_eq!(h.state().engine.undo_label(), Some(tl!("Fill selection")));
    assert!(tooncraft_engine::select::colour_mask(&h.state().engine, 0, 0, [20, 30, 200], 2).is_some());
}

#[test]
fn cell_menu_holds_marks_and_pastes_reversed() {
    let mut h = harness_with(None);
    two_drawings(&mut h);
    // the grid takes the keyboard: a click makes frame 1 the current cell, then the keys open its menu
    h.get(By::new().role(egui::Role::Cell).label_contains(", frame 1:")).click();
    h.run();
    let grid_keys = |h: &mut Harness<'static, ToonApp>, menu_key: bool| {
        if menu_key {
            h.key_press(Key::Menu);
        } else {
            h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
        }
        h.run();
        h.run();
    };
    grid_keys(&mut h, false);
    open_sub(&mut h, "Hold For");
    choose(&mut h, "5 frames");
    assert_eq!(h.state().engine.project.layers[0].exposure_range(0), Some((0, 5)));
    // marks are ticks
    h.get(By::new().role(egui::Role::Cell).label_contains(", frame 1:")).click();
    h.run();
    grid_keys(&mut h, true);
    open_sub(&mut h, "Mark As");
    choose(&mut h, "Key");
    let d = h.state().engine.project.drawing_at(0, 0).expect("drawing");
    assert_eq!(h.state().engine.project.marks.get(&d), Some(&tooncraft_doc::Mark::Key));
    // nothing copied: Paste Reversed stays, disabled
    h.get(By::new().role(egui::Role::Cell).label_contains(", frame 1:")).click();
    h.run();
    grid_keys(&mut h, false);
    assert!(disabled(&h, "Paste Reversed"));
}

#[test]
fn ruler_menu_sets_and_clears_the_loop_and_the_onion_range() {
    let mut h = harness_with(None);
    h.state_mut().frame = 3;
    h.run();
    let ruler = || By::new().label("Frame ruler");
    open_menu(&mut h, ruler(), false);
    assert!(disabled(&h, "Clear In and Out"), "no loop yet");
    choose(&mut h, "Set Loop In Here");
    assert_eq!(h.state().tl.range.map(|r| r.0), Some(3));
    open_menu(&mut h, ruler(), true);
    open_sub(&mut h, "Onion Range");
    choose(&mut h, "3 Each Side");
    let o = h.state().engine.project.onion;
    assert!(o.enabled && o.before == 3 && o.after == 3);
    open_menu(&mut h, ruler(), false);
    choose(&mut h, "Clear In and Out");
    assert!(h.state().tl.range.is_none());
}

#[test]
fn layer_menu_from_the_timeline_row() {
    let mut h = harness_with(None);
    let name = h.state().engine.project.layers[0].name.clone();
    let label = format!("Layer {name}");
    let row = || By::new().role(egui::Role::Button).label(label.as_str());
    open_menu(&mut h, row(), false);
    assert!(disabled(&h, "Delete Layer"), "the only layer");
    let onion = h.get(By::new().role(egui::Role::CheckBox).label("Include in Onion Skin"));
    assert_eq!(onion.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    choose(&mut h, "Include in Onion Skin");
    assert!(!crate::menu_actions::layer_in_onion(h.state(), 0));
    open_menu(&mut h, row(), true);
    h.get(By::new().role(egui::Role::Button).label("Opacity")).focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    choose(&mut h, "50 %");
    assert!((h.state().engine.project.layers[0].opacity - 0.5).abs() < 1e-6);
    open_menu(&mut h, row(), false);
    choose(&mut h, "Add Peg");
    assert!(h.state().engine.project.layers[0].peg.is_some());
}

#[test]
fn swatch_menu_duplicates_and_the_basic_palette_says_why_not() {
    let mut h = harness_with(Some(Pane::Colour));
    two_drawings(&mut h);
    let mut p = Palette::new("Characters");
    p.push("skin", [250, 210, 190]);
    {
        let a = h.state_mut();
        assert!(a.run(Command::SetPalettes { palettes: vec![p] }));
        a.colour.current = 1;
    }
    h.run();
    let swatch = || By::new().role(egui::Role::Button).label("Swatch skin, #FAD2BE");
    h.get(swatch()).scroll_to_me();
    h.run();
    open_menu(&mut h, swatch(), false);
    choose(&mut h, "Duplicate Swatch");
    assert_eq!(h.state().engine.project.palettes[0].swatches.len(), 2);
    // the basic palette cannot be changed: its items say why
    h.state_mut().colour.current = 0;
    h.run();
    let first = h.query_all(By::new().role(egui::Role::Button).label_contains("Swatch #")).next().and_then(|n| n.accesskit_node().label());
    let first = first.expect("a basic swatch");
    h.get(By::new().role(egui::Role::Button).label(first.as_str())).scroll_to_me();
    h.run();
    open_menu(&mut h, By::new().role(egui::Role::Button).label(first.as_str()), true);
    assert!(disabled(&h, "Delete Swatch"));
    assert!(!disabled(&h, "Use Colour"));
}

#[test]
fn brush_preset_menu_keeps_built_ins_read_only() {
    let mut h = harness_with(Some(Pane::Library));
    // a temporary brush folder, never the user's
    let dir = std::env::temp_dir().join(format!("tooncraft-menu-ui-brushes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    h.state_mut().library = crate::brush_library::State::new(Some(dir.clone()));
    h.run();
    h.run();
    let row = h.query_all(By::new().role(egui::Role::Button).label_contains(", Basics")).next().and_then(|n| n.accesskit_node().label());
    let row = row.expect("a built-in brush row");
    open_menu(&mut h, By::new().role(egui::Role::Button).label(row.as_str()), false);
    assert!(disabled(&h, "Delete Brush"), "built-ins are read-only");
    assert!(disabled(&h, "Rename Brush…"));
    let before = h.state().library.library.entries().len();
    choose(&mut h, "Duplicate Brush");
    assert_eq!(h.state().library.library.entries().len(), before + 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn library_item_menu_places_it_on_a_new_layer() {
    let mut h = harness_with(Some(Pane::Drawings));
    two_drawings(&mut h);
    assert!(h.state_mut().run(Command::SaveToLibrary { layer: 0, from: 0, to: 2, name: "Blink".into() }));
    h.run();
    h.run();
    let item = || By::new().label("Blink (3 frames)");
    h.get(item()).scroll_to_me();
    h.run();
    open_menu(&mut h, item(), false);
    assert!(disabled(&h, "Show in Folder"), "items live in the shot");
    let layers = h.state().engine.project.layers.len();
    choose(&mut h, "Place as New Layer");
    assert_eq!(h.state().engine.project.layers.len(), layers + 1);
    assert_eq!(h.state().engine.undo_label(), Some(tl!("Place as new layer")));
}

#[test]
fn tool_button_extras_reset_the_brush_and_open_the_shortcut_editor() {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(500.0, 400.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let r = ui.button("Ink pen tool");
            let now = r.has_focus() && crate::timeline::menu_key(ui);
            crate::timeline::keyboard_menu(ui, &r, now, |ui| crate::context_menus::tool_button_extras(app, ui, crate::app::Tool::Ink));
        },
        app,
    );
    h.run();
    let kind = crate::app::Tool::Ink.brush().expect("ink");
    h.state_mut().brush_mut(kind).size = 61.0;
    open_menu(&mut h, By::new().label("Ink pen tool"), false);
    choose(&mut h, "Reset Tool Settings");
    assert_eq!(h.state_mut().brush_mut(kind).size, tooncraft_brush::Brush::preset(kind).size);
    open_menu(&mut h, By::new().label("Ink pen tool"), true);
    choose(&mut h, "Shortcut…");
    assert!(h.state().show_shortcuts);
}

#[test]
fn panel_tab_menu_has_reset_panel() {
    let mut h = harness_with(None);
    // the tab menu comes from the docking library (Float, Move To, Close) plus Reset Panel
    h.get(By::new().role(egui::Role::Tab).label("Onion Skin")).click_secondary();
    h.run();
    assert!(!disabled(&h, "Reset Panel"));
    choose(&mut h, "Reset Panel");
    assert!(h.state().workspace.is_open(Pane::Onion));
}

#[test]
fn the_launch_option_opens_each_menu() {
    for (target, item) in [
        ("canvas", "Pick Colour Here"),
        ("selection", "Fill Selection"),
        ("cell", "Hold For"),
        ("ruler", "Set Loop In Here"),
        ("layer", "Add Peg"),
        ("tool", "Reset Tool Settings"),
        ("tab", "Reset Panel"),
        ("onion", "More Onion Settings…"),
    ] {
        let ctx = egui::Context::default();
        let cli = Cli { context_menu: Some(target.into()), ..Default::default() };
        let app = ToonApp::with_context(&ctx, cli).without_autosave();
        let mut h = Harness::builder().with_size(vec2(1700.0, 1200.0)).with_max_steps(8).build_ui_state(
            |ui, app: &mut ToonApp| {
                let ctx = ui.ctx().clone();
                app.tick(&ctx);
                app.draw(ui);
            },
            app,
        );
        for _ in 0..4 {
            h.run();
        }
        assert!(h.query_by_label(item).is_some(), "--context-menu {target} shows {item}");
    }
}
