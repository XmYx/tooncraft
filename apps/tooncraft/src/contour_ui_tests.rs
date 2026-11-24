//! UI tests for the contour tool on a vector layer: Shift+P chooses it, dragging a point moves
//! it, Shift+drag changes the width there, clicking a line shows its properties and Delete Line
//! removes it; each change is one undo step. The accessibility check is on.

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;
use tooncraft_vector::VPoint;

use crate::app::{Tool, ToonApp};
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

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> Pos2 {
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(rect, size, egui::pos2(x, y))
}

fn drag(h: &mut Harness<'static, ToonApp>, from: (f32, f32), to: (f32, f32), shift: bool) {
    let (a, b) = (screen(h, from.0, from.1), screen(h, to.0, to.1));
    if shift {
        h.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
    }
    h.hover_at(a);
    h.run();
    h.drag_at(a);
    h.run();
    for k in 1..=6 {
        h.hover_at(a + (b - a) * (k as f32 / 6.0));
        h.run();
    }
    h.drop_at(b);
    h.run();
    if shift {
        h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.run();
    }
}

fn line(h: &Harness<'static, ToonApp>) -> Option<tooncraft_vector::VStroke> {
    let app = h.state();
    let p = &app.engine.project;
    p.drawing_at(app.layer, app.frame).and_then(|d| p.vectors.get(&d)).and_then(|v| v.strokes.first().cloned())
}

#[test]
fn the_contour_tool_reshapes_a_vector_line() {
    let mut h = harness();
    let (layer, frame) = (h.state().layer, h.state().frame);
    assert!(h.state_mut().run(Command::SetLayerVector { layer, vector: true }));
    let points = vec![VPoint::new(200.0, 300.0, 8.0), VPoint::new(400.0, 260.0, 8.0), VPoint::new(600.0, 300.0, 8.0)];
    let c: Command =
        serde_json::from_value(serde_json::json!({"cmd":"vector_stroke","layer":layer,"frame":frame,"color":[0,0,0,255],"points":points,"tolerance":0.0}))
            .unwrap();
    assert!(h.state_mut().run(c));
    h.key_press_modifiers(Modifiers::SHIFT, Key::P);
    h.run();
    assert_eq!(h.state().tool, Tool::Contour);
    // drag the middle point down
    let undo = h.state().engine.undo_len();
    drag(&mut h, (400.0, 260.0), (400.0, 380.0), false);
    let s = line(&h).expect("the line");
    assert!((s.points[1].y - 380.0).abs() < 3.0, "moved: {:?}", s.points[1]);
    assert_eq!(h.state().engine.undo_len(), undo + 1);
    // Shift+drag up on the first point makes it thicker there
    drag(&mut h, (200.0, 300.0), (200.0, 260.0), true);
    let s = line(&h).expect("the line");
    assert!(s.points[0].width > 20.0, "thicker: {}", s.points[0].width);
    assert!((s.points[0].y - 300.0).abs() < 0.5, "a width drag does not move the point");
    // click the line: its properties show; Delete Line removes it as one step
    let at = screen(&h, 600.0 - 3.0, 300.0);
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    assert_eq!(h.state().aids.contour.stroke, Some(0), "the line is chosen");
    h.get(By::new().role(egui::Role::SpinButton).label("Line width"));
    let undo = h.state().engine.undo_len();
    h.get(By::new().role(egui::Role::Button).label("Delete Line")).click();
    h.run();
    assert!(line(&h).is_none(), "deleted");
    assert_eq!(h.state().engine.undo_len(), undo + 1);
}

/// A vector layer with one line on frame 0 and another drawing with a line on frame 1.
fn two_drawings() -> Harness<'static, ToonApp> {
    let mut h = harness();
    let layer = h.state().layer;
    assert!(h.state_mut().run(Command::SetLayerVector { layer, vector: true }));
    for (frame, y) in [(0u32, 300.0f32), (1, 500.0)] {
        assert!(h.state_mut().run(Command::NewDrawing { layer, frame }));
        let points = vec![VPoint::new(200.0, y, 8.0), VPoint::new(400.0, y - 40.0, 8.0), VPoint::new(600.0, y, 8.0)];
        let c: Command =
            serde_json::from_value(serde_json::json!({"cmd":"vector_stroke","layer":layer,"frame":frame,"color":[0,0,0,255],"points":points,"tolerance":0.0}))
                .unwrap();
        assert!(h.state_mut().run(c));
    }
    h.state_mut().frame = 0;
    h.state_mut().tool = Tool::Contour;
    h.run();
    h
}

fn press(h: &mut Harness<'static, ToonApp>, label: &str) {
    let b = By::new().role(egui::Role::Button).label(label);
    h.get(b.clone()).scroll_to_me();
    h.run();
    h.get(b).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

#[test]
fn a_line_is_chosen_and_deleted_from_the_keyboard_only() {
    let mut h = two_drawings();
    press(&mut h, "Next Line");
    assert_eq!(h.state().aids.contour.stroke, Some(0));
    let undo = h.state().engine.undo_len();
    press(&mut h, "Delete Line");
    assert!(line(&h).is_none(), "deleted by keyboard");
    assert_eq!(h.state().engine.undo_len(), undo + 1);
}

#[test]
fn a_choice_does_not_follow_to_another_drawing_and_tabbing_the_width_changes_nothing() {
    let mut h = two_drawings();
    press(&mut h, "Next Line");
    assert_eq!(h.state().aids.contour.stroke, Some(0));
    // tab in and out of the width field: no undo step
    let undo = h.state().engine.undo_len();
    let field = By::new().role(egui::Role::SpinButton).label("Line width");
    h.get(field.clone()).focus();
    h.run();
    h.key_press(Key::Tab);
    h.run();
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo, "an unchanged width is not an edit");
    // another frame with its own line: the choice is dropped, the options show the hint
    h.state_mut().frame = 1;
    h.run();
    h.run();
    assert_eq!(h.state().aids.contour.stroke, None);
    assert!(h.query(By::new().role(egui::Role::Button).label("Delete Line")).is_none(), "nothing to delete on this drawing");
    h.get(By::new().label_contains("click a line to choose it"));
}
