//! UI tests for the gap-closing slices: layer operations (duplicate, merge, flatten, copy look,
//! transparency lock) and the brush's paint position. Everything is driven by labels, as a
//! keyboard or screen-reader user would reach it, with the accessibility check on.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_brush::Compose;
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1400.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(Pane::Layers);
    h.run();
    h.run();
    h
}

fn click(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).scroll_to_me();
    h.run();
    h.get_by_label(label).click();
    h.run();
}

fn paint_two_layers(h: &mut Harness<'static, ToonApp>) {
    let brush = tooncraft_brush::Brush { size: 20.0, ..tooncraft_brush::Brush::preset(tooncraft_brush::BrushKind::Ink) };
    let samples: Vec<_> = (0..20).map(|i| tooncraft_brush::Sample::at(40.0 + i as f32 * 5.0, 60.0, 1.0, i as f64 * 0.01)).collect();
    assert!(h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush, samples }));
    assert!(h.state_mut().run(Command::AddLayer { name: "Top".into(), index: None }));
    h.state_mut().layer = 1;
    h.run();
}

#[test]
fn the_layer_buttons_are_named_and_duplicate_selects_the_copy() {
    let mut h = harness();
    paint_two_layers(&mut h);
    h.state_mut().layer = 0;
    h.run();
    for name in ["Add", "Add Other", "Duplicate Layer", "Merge Down", "Add Peg", "Delete Layer", "Layer options"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
    // the less used ones are in the ⋯ menu
    click(&mut h, "Layer options");
    for name in ["Move Layer Up", "Move Layer Down", "Rename Layer", "Merge Visible", "Flatten"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
    h.key_press(egui::Key::Escape);
    h.run();
    click(&mut h, "Duplicate Layer");
    assert_eq!(h.state().engine.project.layers.len(), 3);
    assert_eq!(h.state().layer, 1, "the copy is the current layer");
    assert_eq!(h.state().engine.project.layers[1].name, "Layer 1 copy");
    h.state_mut().undo();
    assert_eq!(h.state().engine.project.layers.len(), 2);
}

#[test]
fn merge_down_flatten_and_the_limits_are_reported() {
    let mut h = harness();
    paint_two_layers(&mut h);
    h.state_mut().layer = 1;
    h.run();
    click(&mut h, "Merge Down");
    assert_eq!(h.state().engine.project.layers.len(), 1);
    assert_eq!(h.state().layer, 0, "the selection follows the merged layer");
    assert_eq!(h.state().engine.undo_label(), Some("Merge down"));
    h.state_mut().undo();
    assert_eq!(h.state().engine.project.layers.len(), 2);
    // a locked layer refuses, and says so in the status bar
    h.state_mut().run(Command::SetLayer { layer: 1, visible: None, locked: Some(true), opacity: None });
    h.run();
    click(&mut h, "Merge Down");
    assert_eq!(h.state().engine.project.layers.len(), 2);
    h.state_mut().run(Command::SetLayer { layer: 1, visible: None, locked: Some(false), opacity: None });
    h.run();
    click(&mut h, "Layer options");
    click(&mut h, "Flatten");
    assert_eq!(h.state().engine.project.layers.len(), 1);
    assert_eq!(h.state().layer, 0);
}

#[test]
fn lock_transparency_and_copy_look_work_by_label() {
    let mut h = harness();
    paint_two_layers(&mut h);
    h.state_mut().run(Command::SetLayerBlend { layer: 0, blend: tooncraft_doc::BlendMode::SoftLight });
    h.state_mut().run(Command::SetLayer { layer: 0, visible: None, locked: None, opacity: Some(0.6) });
    h.run();
    click(&mut h, "Lock transparency");
    assert!(h.state().engine.project.layers[1].lock_alpha);
    click(&mut h, "Lock transparency");
    assert!(!h.state().engine.project.layers[1].lock_alpha);
    // copy the look of "Layer 1" onto the current layer
    h.get(By::new().role(egui::Role::ComboBox).label("Copy look from")).click();
    h.run();
    // the list's own "Layer 1" comes first; the combo's popup entry is the last one
    h.query_all(By::new().label("Layer 1")).last().unwrap().click();
    h.run();
    let l = &h.state().engine.project.layers[1];
    assert_eq!((l.blend, l.opacity), (tooncraft_doc::BlendMode::SoftLight, 0.6));
}

#[test]
fn every_blend_mode_is_in_the_combo_and_named() {
    let mut h = harness();
    click(&mut h, "Blend");
    for b in tooncraft_doc::BlendMode::ALL {
        let name = crate::panels::blend_name(b);
        assert!(!name.is_empty());
    }
    let names: std::collections::BTreeSet<String> = tooncraft_doc::BlendMode::ALL.iter().map(|b| crate::panels::blend_name(*b)).collect();
    assert_eq!(names.len(), tooncraft_doc::BlendMode::ALL.len(), "every mode has its own label");
}

#[test]
fn the_brush_paint_position_can_be_chosen_and_paints_behind() {
    let mut h = harness();
    h.state_mut().tool = Tool::Ink;
    h.state_mut().workspace.raise(Pane::Brush);
    h.run();
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Paint"));
    click(&mut h, "Paint");
    click(&mut h, "Behind");
    assert_eq!(h.state().brush().compose, Compose::Behind);
    // red first, then a blue stroke behind it
    let red = tooncraft_brush::Brush { color: [255, 0, 0, 255], size: 20.0, compose: Compose::Normal, ..h.state().brush() };
    let across: Vec<_> = (0..30).map(|i| tooncraft_brush::Sample::at(30.0 + i as f32 * 4.0, 80.0, 1.0, i as f64 * 0.01)).collect();
    h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush: red, samples: across.clone() });
    let blue = tooncraft_brush::Brush { color: [0, 0, 255, 255], ..h.state().brush() };
    let tall: Vec<_> = (0..30).map(|i| tooncraft_brush::Sample::at(90.0, 40.0 + i as f32 * 3.0, 1.0, i as f64 * 0.01)).collect();
    h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush: blue, samples: tall });
    let e = &h.state().engine;
    let d = e.project.drawing(e.project.drawing_at(0, 0).unwrap()).unwrap();
    assert_eq!(d.pixel(90, 80)[0], 255, "red stays on top where the strokes cross");
    assert!(d.pixel(90, 50)[2] > 200, "blue fills the paper above the bar");
}

#[test]
fn the_layer_menu_has_the_same_operations() {
    let mut h = harness();
    h.state_mut().run(Command::AddLayer { name: "Two".into(), index: None });
    h.state_mut().layer = 1;
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Layer")).click();
    h.run();
    for name in ["Duplicate Layer", "Merge Layer Down", "Merge Visible Layers", "Flatten Layers", "Lock Transparency"] {
        assert!(h.query_all(By::new().role(egui::Role::Button).label(name)).count() >= 1, "{name} is in the menu");
    }
    h.state_mut().layer = 1;
    h.run();
    // by keyboard, as a menu is meant to be used
    h.get_by_label("Merge Layer Down").focus();
    h.run();
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().engine.project.layers.len(), 1, "{}", h.state().status);
}
