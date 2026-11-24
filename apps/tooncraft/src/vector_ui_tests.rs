//! UI tests for vector layers: the Layers panel switch and pen strokes on the canvas.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> egui::Pos2 {
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y))
}

#[test]
fn a_pen_stroke_on_a_vector_layer_is_kept_as_a_line() {
    let mut h = harness();
    h.get_by_label("Vector lines").scroll_to_me();
    h.run();
    h.get_by_label("Vector lines").click();
    h.run();
    let layer = h.state().layer;
    assert!(h.state().engine.project.layers[layer].vector);
    h.state_mut().tool = Tool::Ink;
    h.run();
    let pts: Vec<egui::Pos2> = (0..12).map(|i| screen(&h, 200.0 + i as f32 * 40.0, 300.0 + i as f32 * 10.0)).collect();
    h.drag_at(pts[0]);
    h.run();
    for p in &pts[1..] {
        h.hover_at(*p);
        h.run();
    }
    h.drop_at(pts[11]);
    h.run();
    h.run();
    let p = &h.state().engine.project;
    let d = p.drawing_at(layer, 0).expect("the stroke made a drawing");
    let v = p.vectors.get(&d).expect("kept as vector data");
    assert_eq!(v.strokes.len(), 1);
    assert!(v.strokes[0].points.len() >= 2);
    assert_eq!(h.state().engine.undo_label(), Some("Vector stroke"));
}

#[test]
fn line_ends_and_corners_chosen_in_the_options_shape_the_next_line() {
    let mut h = harness();
    h.get_by_label("Vector lines").scroll_to_me();
    h.run();
    h.get_by_label("Vector lines").click();
    h.run();
    let layer = h.state().layer;
    h.state_mut().tool = Tool::Ink;
    h.run();
    // the options offer the ends and corners only on a vector layer
    let pick = |h: &mut Harness<'static, ToonApp>, combo: &str, item: &str| {
        h.get_by_label(combo).click();
        h.run();
        h.get_by_label(item).click();
        h.run();
    };
    pick(&mut h, "Line ends", "Flat");
    pick(&mut h, "Corners", "Sharp");
    assert_eq!(h.state().engine.line_style, tooncraft_vector::LineStyle { cap: tooncraft_vector::Cap::Flat, join: tooncraft_vector::Join::Sharp });
    let pts: Vec<egui::Pos2> = (0..10).map(|i| screen(&h, 200.0 + i as f32 * 40.0, 300.0)).collect();
    h.drag_at(pts[0]);
    h.run();
    for p in &pts[1..] {
        h.hover_at(*p);
        h.run();
    }
    h.drop_at(pts[9]);
    h.run();
    h.run();
    let p = &h.state().engine.project;
    let d = p.drawing_at(layer, 0).expect("the stroke made a drawing");
    let v = p.vectors.get(&d).expect("kept as vector data");
    assert_eq!(v.strokes[0].style.cap, tooncraft_vector::Cap::Flat);
    // a flat end stops at the end point: nothing painted a few pixels before the first point
    let first = v.strokes[0].points[0];
    let s = p.drawing(d).expect("painted");
    assert_eq!(s.pixel(first.x as i64 - 4, first.y as i64)[3], 0, "flat: no paint past the end");
    assert!(s.pixel(first.x as i64 + 4, first.y as i64)[3] > 0, "the line itself is painted");
}
