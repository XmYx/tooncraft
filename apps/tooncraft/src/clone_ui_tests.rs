//! UI tests for the clone stamp tool: chosen with its key, Alt+click sets the source, a drag
//! paints the copy as one undo step, and aligned strokes keep their offset. The accessibility
//! check is on.

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 940.0)).with_accessibility_check(true).build_ui_state(
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

/// The screen point over canvas pixel (x, y).
fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> Pos2 {
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(rect, size, egui::pos2(x, y))
}

fn pixel(h: &Harness<'static, ToonApp>, x: i64, y: i64) -> [u8; 4] {
    let app = h.state();
    let p = &app.engine.project;
    p.drawing_at(app.layer, app.frame).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)).unwrap_or([0; 4])
}

fn alt_click(h: &mut Harness<'static, ToonApp>, at: Pos2) {
    h.hover_at(at);
    h.run();
    h.event(egui::Event::ModifiersChanged(Modifiers::ALT));
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT });
    h.run();
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run();
}

fn drag_through(h: &mut Harness<'static, ToonApp>, points: &[Pos2]) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return };
    h.drag_at(*first);
    h.run();
    for p in &points[1..] {
        h.hover_at(*p);
        h.run();
    }
    h.drop_at(*last);
    h.run();
}

#[test]
fn alt_click_sets_the_source_and_a_drag_paints_the_copy() {
    let mut h = harness();
    let (layer, frame) = (h.state().layer, h.state().frame);
    let square = vec![(100.0, 100.0), (180.0, 100.0), (180.0, 180.0), (100.0, 180.0)];
    let red = [255, 0, 0, 255];
    assert!(h.state_mut().run(Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: red,
        end: red,
        area: Some(square),
        area_rings: Vec::new(),
        stops: Vec::new(),
        feather: 0.0
    }));
    h.key_press(Key::J);
    h.run();
    assert_eq!(h.state().tool, Tool::Clone);
    h.get(By::new().role(egui::Role::CheckBox).label("Aligned"));
    h.get(By::new().role(egui::Role::ComboBox).label("Copy from"));
    // painting before a source is set does nothing
    let (a, b) = (screen(&h, 400.0, 140.0), screen(&h, 410.0, 140.0));
    drag_through(&mut h, &[a, b]);
    assert_eq!(pixel(&h, 400, 140)[3], 0);
    let undo = h.state().engine.undo_len();
    let src = screen(&h, 140.0, 140.0);
    alt_click(&mut h, src);
    assert!(h.state().aids.clone.source.is_some_and(|s| (s.0 - 140.0).abs() < 2.0 && (s.1 - 140.0).abs() < 2.0), "{:?}", h.state().aids.clone.source);
    assert_eq!(h.state().engine.undo_len(), undo, "setting the source changes nothing");
    let path = [screen(&h, 400.0, 140.0), screen(&h, 405.0, 140.0), screen(&h, 410.0, 140.0)];
    drag_through(&mut h, &path);
    assert_eq!(h.state().engine.undo_label(), Some("Clone"));
    assert_eq!(h.state().engine.undo_len(), undo + 1, "one stroke, one undo step");
    assert_eq!(pixel(&h, 405, 140), red, "the copy of the square");
    // aligned: the next stroke keeps the offset (260 px left), so it copies what is left of the square
    // (70, 140), not the source point again
    let path = [screen(&h, 330.0, 140.0), screen(&h, 335.0, 140.0)];
    drag_through(&mut h, &path);
    assert_eq!(pixel(&h, 332, 140)[3], 0, "aligned: 72,140 is outside the square");
    h.get(By::new().role(egui::Role::CheckBox).label("Aligned")).click();
    h.run();
    assert!(!h.state().aids.clone.aligned);
    let path = [screen(&h, 330.0, 140.0), screen(&h, 335.0, 140.0)];
    drag_through(&mut h, &path);
    assert_eq!(pixel(&h, 332, 140), red, "not aligned: each stroke starts from the source");
}
