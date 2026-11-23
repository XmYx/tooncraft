//! UI tests for the right-click menus (exposure cells, layer names, canvas), the strip of
//! toggles above the canvas, and the sheet's hold keys and drawing steps. The menus are opened
//! with a secondary click, as a user would; keys are pressed as keys. The accessibility check is on.

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::Mark;
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

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

/// Two drawings on layer 0: A on frames 1 to 3, B from frame 4.
fn two_drawings(h: &mut Harness<'static, ToonApp>) -> (tooncraft_doc::DrawingId, tooncraft_doc::DrawingId) {
    let app = h.state_mut();
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(app.run(Command::SetExposure { layer: 0, frame: 0, length: 3 }));
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 3 }));
    let p = &app.engine.project;
    let ids = (p.drawing_at(0, 0).expect("A"), p.drawing_at(0, 3).expect("B"));
    h.run();
    ids
}

fn layer_name(h: &Harness<'static, ToonApp>) -> String {
    h.state().engine.project.layers[0].name.clone()
}

fn cell_label(h: &Harness<'static, ToonApp>, frame: u32) -> String {
    format!("{}, frame {frame}:", layer_name(h))
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

#[test]
fn the_cell_menu_swaps_marks_and_holds() {
    let mut h = harness();
    let (a, b) = two_drawings(&mut h);
    // swap: right-click a held frame of A and pick B; the whole exposure shows B
    h.get(By::new().role(egui::Role::Cell).label_contains(&cell_label(&h, 2))).click_secondary();
    h.run();
    assert_eq!(h.state().frame, 1, "the right-clicked cell becomes the current one");
    click(&mut h, "Swap Drawing");
    click(&mut h, &format!("Drawing {}", b.0));
    for f in 0..3 {
        assert_eq!(h.state().engine.project.drawing_at(0, f), Some(b));
    }
    assert_eq!(h.state().engine.undo_label(), Some("Swap drawing"));
    h.state_mut().undo();
    assert_eq!(h.state().engine.project.drawing_at(0, 1), Some(a));
    // mark it as a key
    h.get(By::new().role(egui::Role::Cell).label_contains(&cell_label(&h, 1))).click_secondary();
    h.run();
    click(&mut h, "Mark Drawing");
    click(&mut h, "Key");
    assert_eq!(h.state().engine.project.marks.get(&a), Some(&Mark::Key));
    // hold it for five frames
    h.get(By::new().role(egui::Role::Cell).label_contains(&cell_label(&h, 1))).click_secondary();
    h.run();
    click(&mut h, "Hold For");
    click(&mut h, "5 frames");
    assert_eq!(h.state().engine.project.layers[0].exposure_range(0), Some((0, 5)));
}

#[test]
fn the_layer_menus_in_the_timeline_and_the_layers_panel() {
    let mut h = harness();
    let name = layer_name(&h);
    h.get(By::new().role(egui::Role::Button).label(&format!("Layer {name}"))).click_secondary();
    h.run();
    click(&mut h, "Add Layer Above");
    assert_eq!(h.state().engine.project.layers.len(), 2);
    assert_eq!(h.state().layer, 1);
    // the Layers panel row has the same menu
    h.state_mut().workspace.raise(crate::dock::Pane::Layers);
    h.run();
    h.run();
    let first = crate::layer_tree::path_name(&h.state().engine.project, &h.state().engine.project.layers[0]);
    h.get(By::new().role(egui::Role::Button).label(&first)).click_secondary();
    h.run();
    assert_eq!(h.state().layer, 0, "the menu acts on the row it was opened on");
    click(&mut h, "Label Colour");
    h.get(By::new().role(egui::Role::Button).label("Green")).click();
    h.run();
    assert_eq!(h.state().engine.project.layers[0].color, Some([92, 170, 92]));
    h.get(By::new().role(egui::Role::Button).label(&first)).click_secondary();
    h.run();
    click(&mut h, "Hide Layer");
    assert!(!h.state().engine.project.layers[0].visible);
}

#[test]
fn the_canvas_menu_switches_tools_and_selects() {
    let mut h = harness();
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).click_secondary();
    h.run();
    click(&mut h, "Tool");
    h.get(By::new().role(egui::Role::Button).label("Paint bucket")).click();
    h.run();
    assert_eq!(h.state().tool, Tool::Fill);
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).click_secondary();
    h.run();
    click(&mut h, "Select All");
    assert!(h.state().selection.is_some());
}

#[test]
fn the_canvas_strip_toggles_what_the_canvas_shows() {
    let mut h = harness();
    let onion = h.state().engine.project.onion.enabled;
    h.get(By::new().role(egui::Role::Button).label("Onion Skin")).click();
    h.run();
    assert_eq!(h.state().engine.project.onion.enabled, !onion);
    h.get(By::new().role(egui::Role::Button).label("Symmetry")).click();
    h.run();
    assert!(h.state().engine.project.symmetry.is_on());
    h.get(By::new().role(egui::Role::Button).label("Mirror View")).click();
    h.run();
    assert!(h.state().view.flip);
}

#[test]
fn number_keys_set_the_hold_and_ctrl_arrows_step_between_drawings() {
    let mut h = harness();
    two_drawings(&mut h);
    h.get(By::new().role(egui::Role::Cell).label_contains(&cell_label(&h, 1))).click();
    h.run();
    assert!(h.ctx.memory(|m| m.has_focus(crate::timeline::grid_id())));
    h.key_press(Key::Num4);
    h.run();
    assert_eq!(h.state().engine.project.layers[0].exposure_range(0), Some((0, 4)));
    h.get(By::new().label("Hold"));
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    h.run();
    assert_eq!(h.state().frame, 4, "next drawing skips the holds");
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowRight);
    h.run();
    assert_eq!(h.state().frame, 4, "no later drawing: it stays");
    h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowLeft);
    h.run();
    assert_eq!(h.state().frame, 0);
}

#[test]
fn the_onion_toggle_has_quick_settings_and_its_reach_is_on_the_ruler() {
    let mut h = harness();
    two_drawings(&mut h);
    {
        let app = h.state_mut();
        app.engine.project.onion.enabled = true;
        app.engine.project.onion.mode = tooncraft_doc::OnionMode::Frames;
        app.engine.project.onion.before = 2;
        app.engine.project.onion.after = 1;
        app.frame = 3;
    }
    h.run();
    assert_eq!(crate::timeline::onion_span(h.state()), Some((1, 4)));
    h.state_mut().engine.project.onion.mode = tooncraft_doc::OnionMode::Drawings;
    h.state_mut().frame = 1;
    assert_eq!(crate::timeline::onion_span(h.state()), Some((0, 3)), "from drawing A's start to the end of drawing B");
    h.get(By::new().role(egui::Role::Button).label("Onion Skin")).click_secondary();
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Outlines only")).click();
    h.run();
    assert!(h.state().engine.project.onion.outline);
    h.state_mut().engine.project.onion.enabled = false;
    assert_eq!(crate::timeline::onion_span(h.state()), None);
}

#[test]
fn swap_drawings_and_open_the_menus_from_the_keyboard() {
    let mut h = harness();
    let (a, b) = two_drawings(&mut h);
    // focus the sheet on frame 2 (A's hold) and swap to the next drawing by key
    h.get(By::new().role(egui::Role::Cell).label_contains(&cell_label(&h, 2))).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowDown);
    h.run();
    assert_eq!(h.state().engine.project.drawing_at(0, 0), Some(b), "the whole exposure shows B");
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowUp);
    h.run();
    assert_eq!(h.state().engine.project.drawing_at(0, 1), Some(a), "and back");
    // Shift+F10 opens the cell's menu
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run();
    h.run();
    h.get_by_label("Swap Drawing");
    h.key_press(Key::Escape);
    h.run();
    // the Menu key opens the canvas menu when the canvas has the focus
    h.key_press(Key::F6);
    h.run();
    assert!(h.ctx.memory(|m| m.has_focus(crate::canvas::canvas_id())));
    h.key_press(Key::Menu);
    h.run();
    h.run();
    h.get_by_label("Pick Colour Here");
    // a blank cell has nothing to swap
    h.state_mut().frame = 6;
    h.state_mut().swap_step(true);
    assert_eq!(h.state().engine.undo_label(), Some("Swap drawing"));
}
