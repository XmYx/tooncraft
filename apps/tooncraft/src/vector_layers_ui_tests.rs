//! UI tests for vector layers: View ▸ Vector Lines shows the centre lines whatever the tool, and
//! a pixel edit that turns a vector drawing into pixels says so in the status bar (undo brings
//! the lines back). The accessibility check is on.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;
use tooncraft_raster::fill::FillOptions;
use tooncraft_vector::VPoint;

use crate::app::ToonApp;
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 940.0)).with_accessibility_check(true).build_ui_state(
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

/// A closed square of four vector lines on the current layer.
fn square(h: &mut Harness<'static, ToonApp>) {
    let layer = h.state().layer;
    assert!(h.state_mut().run(Command::SetLayerVector { layer, vector: true }));
    for pts in [[(200.0, 200.0), (600.0, 200.0)], [(600.0, 200.0), (600.0, 500.0)], [(600.0, 500.0), (200.0, 500.0)], [(200.0, 500.0), (200.0, 200.0)]] {
        let frame = h.state().frame;
        let c = Command::VectorStroke {
            layer,
            frame,
            color: [0, 0, 0, 255],
            points: pts.iter().map(|(x, y)| VPoint::new(*x, *y, 6.0)).collect(),
            tolerance: 0.6,
            style: Default::default(),
        };
        assert!(h.state_mut().run(c));
    }
    h.run();
}

#[test]
fn the_view_menu_shows_the_vector_lines() {
    let mut h = harness();
    square(&mut h);
    assert!(!h.state().aids.vector_lines);
    h.get_by_label("View").click();
    h.run();
    h.get(By::new().label("Vector Lines")).click();
    h.run();
    assert!(h.state().aids.vector_lines, "on from the menu");
    h.get_by_label("View").click();
    h.run();
    h.get(By::new().label("Vector Lines")).click();
    h.run();
    assert!(!h.state().aids.vector_lines, "and off again");
}

#[test]
fn filling_a_vector_drawing_says_its_lines_are_pixels_and_undo_brings_them_back() {
    let mut h = harness();
    square(&mut h);
    let (layer, frame) = (h.state().layer, h.state().frame);
    let fill = Command::Fill {
        layer,
        frame,
        x: 400,
        y: 350,
        color: [200, 40, 40, 255],
        options: FillOptions::default(),
        reference: tooncraft_engine::FillReference::AllLayers,
    };
    assert!(h.state_mut().run(fill));
    h.run();
    assert!(h.state().status.contains("vector lines are pixels now"), "{}", h.state().status);
    let d = h.state().engine.project.drawing_at(layer, frame).unwrap();
    assert!(!h.state().engine.project.vectors.contains_key(&d));
    h.state_mut().undo();
    h.run();
    assert!(h.state().engine.project.vectors.contains_key(&d), "the lines are back");
}
