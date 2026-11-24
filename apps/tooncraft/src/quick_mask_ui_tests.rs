//! UI tests for quick mask: Shift+Q turns the selection into a mask, brush strokes paint into
//! it (the eraser takes away) without touching the drawing, and turning it off selects what
//! was painted. The accessibility check is on.

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

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

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> Pos2 {
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(rect, size, egui::pos2(x, y))
}

fn drag(h: &mut Harness<'static, ToonApp>, from: (f32, f32), to: (f32, f32)) {
    let (a, b) = (screen(h, from.0, from.1), screen(h, to.0, to.1));
    h.drag_at(a);
    h.run();
    for k in 1..=8 {
        h.hover_at(a + (b - a) * (k as f32 / 8.0));
        h.run();
    }
    h.drop_at(b);
    h.run();
}

#[test]
fn shift_q_paints_the_selection_and_back() {
    let mut h = harness();
    let undo = h.state().engine.undo_len();
    h.key_press_modifiers(Modifiers::SHIFT, Key::Q);
    h.run();
    assert!(crate::quick_mask::is_on(h.state()), "Shift+Q turns quick mask on");
    h.state_mut().tool = Tool::Ink;
    h.state_mut().brush_mut(tooncraft_brush::BrushKind::Ink).size = 40.0;
    h.run();
    drag(&mut h, (200.0, 200.0), (500.0, 200.0));
    assert_eq!(h.state().engine.undo_len(), undo, "painting the mask does not paint the drawing");
    let q = h.state().sel.quick.clone().expect("on");
    assert!(q.get(350, 200) && q.get(350, 215), "the stroke is in the mask");
    assert!(!q.get(350, 260), "away from it is not");
    // the eraser takes part of it away
    h.state_mut().tool = Tool::Eraser;
    h.state_mut().brush_mut(tooncraft_brush::BrushKind::Eraser).size = 40.0;
    h.run();
    drag(&mut h, (420.0, 140.0), (420.0, 260.0));
    assert!(!h.state().sel.quick.as_ref().expect("on").get(420, 200), "erased from the mask");
    h.key_press_modifiers(Modifiers::SHIFT, Key::Q);
    h.run();
    assert!(!crate::quick_mask::is_on(h.state()));
    let m = crate::selection_tools::mask(h.state()).expect("a selection");
    assert!(m.get(300, 200) && m.get(480, 200), "both painted parts are selected");
    assert!(!m.get(420, 200) && !m.get(300, 300), "the erased gap and the rest are not");
    // with a selection already, quick mask starts from it
    h.key_press_modifiers(Modifiers::SHIFT, Key::Q);
    h.run();
    assert!(h.state().sel.quick.as_ref().expect("on").get(300, 200), "the selection becomes the mask");
}
