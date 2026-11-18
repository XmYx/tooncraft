//! UI tests for the Undo History panel and the recent colours strip.

use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;
use crate::dock::Pane;

fn harness(pane: Pane) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(pane);
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

#[test]
fn the_history_lists_every_step_and_a_click_jumps_there() {
    let mut h = harness(Pane::History);
    for name in ["Sky", "Ground", "Hero"] {
        h.state_mut().run(Command::AddLayer { name: name.into(), index: None });
    }
    h.run();
    h.get_by_label("Start");
    h.get_by_label("Step 1: Add layer");
    h.get_by_label("Step 2: Add layer");
    h.get_by_label("Step 3: Add layer (now)");
    assert_eq!(h.state().engine.project.layers.len(), 4);
    click(&mut h, "Step 1: Add layer");
    assert_eq!(h.state().engine.project.layers.len(), 2, "back to just after the first step");
    // the steps after it are still listed, dimmed, and redoable by clicking
    h.get_by_label("Step 3: Add layer");
    click(&mut h, "Step 3: Add layer");
    assert_eq!(h.state().engine.project.layers.len(), 4);
    click(&mut h, "Start");
    assert_eq!(h.state().engine.project.layers.len(), 1);
    h.get_by_label("Start (now)");
    assert!(h.state().layer < h.state().engine.project.layers.len(), "the selection stays on a layer that exists");
}

#[test]
fn the_history_buttons_follow_the_state_and_the_edit_menu_opens_the_panel() {
    let mut h = harness(Pane::Colour);
    h.state_mut().run(Command::AddLayer { name: "One".into(), index: None });
    h.get(By::new().role(egui::Role::Button).label("Edit")).click();
    h.run();
    h.get_by_label("Undo History").click();
    h.run();
    // the menu item sat over the canvas: its click must not have started a stroke under it
    assert!(!h.state().engine.is_stroking(), "a click on a menu is not a stroke");
    h.get(By::new().role(egui::Role::Button).label("Undo"));
    click(&mut h, "Undo");
    assert_eq!(h.state().engine.project.layers.len(), 1);
    click(&mut h, "Redo");
    assert_eq!(h.state().engine.project.layers.len(), 2);
}

#[test]
fn painting_fills_the_recent_colours_and_clicking_one_chooses_it() {
    let mut h = harness(Pane::Colour);
    h.get_by_label("Colours you paint with show up here");
    let brush = tooncraft_brush::Brush { color: [200, 40, 40, 255], ..h.state().brush() };
    let samples: Vec<_> = (0..8).map(|i| tooncraft_brush::Sample::at(30.0 + i as f32 * 5.0, 40.0, 1.0, i as f64 * 0.01)).collect();
    assert!(h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush, samples: samples.clone() }));
    let blue = tooncraft_brush::Brush { color: [20, 40, 200, 255], ..brush };
    assert!(h.state_mut().run(Command::Stroke { layer: 0, frame: 0, brush: blue, samples }));
    h.run();
    h.state_mut().color = [1, 2, 3, 255];
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Recent colour #C82828"));
    click(&mut h, "Recent colour #1428C8");
    assert_eq!(&h.state().color[..3], &[20, 40, 200]);
    assert_eq!(h.state().recent.first(), Some(&[20, 40, 200]), "newest first");
}
