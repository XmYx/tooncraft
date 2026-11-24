//! UI tests for the liquify tool: chosen with Shift+K, its options, and a push drag that moves
//! paint as one undo step. The accessibility check is on.

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

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> Pos2 {
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(rect, size, egui::pos2(x, y))
}

fn alpha(h: &Harness<'static, ToonApp>, x: i64, y: i64) -> u8 {
    let app = h.state();
    let p = &app.engine.project;
    p.drawing_at(app.layer, app.frame).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)[3]).unwrap_or(0)
}

#[test]
fn shift_k_liquifies_paint_along_a_drag() {
    let mut h = harness();
    let (layer, frame) = (h.state().layer, h.state().frame);
    let sq = vec![(300.0, 300.0), (400.0, 300.0), (400.0, 400.0), (300.0, 400.0)];
    let c = [0, 0, 200, 255];
    assert!(h.state_mut().run(Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(sq),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new()
    }));
    h.key_press_modifiers(Modifiers::SHIFT, Key::K);
    h.run();
    assert_eq!(h.state().tool, Tool::Liquify);
    h.get(By::new().role(egui::Role::ComboBox).label("Liquify mode"));
    h.state_mut().aids.liquify.brush.strength = 1.0;
    h.state_mut().aids.liquify.brush.radius = 50.0;
    h.run();
    assert_eq!(alpha(&h, 430, 350), 0);
    let undo = h.state().engine.undo_len();
    let (a, b) = (screen(&h, 380.0, 350.0), screen(&h, 460.0, 350.0));
    h.drag_at(a);
    h.run();
    for k in 1..=16 {
        h.hover_at(a + (b - a) * (k as f32 / 16.0));
        h.run();
    }
    h.drop_at(b);
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo + 1, "one stroke, one undo step");
    assert_eq!(h.state().engine.undo_label(), Some("Liquify"));
    assert!(alpha(&h, 430, 350) > 0, "paint pushed right past the square's edge");
}
