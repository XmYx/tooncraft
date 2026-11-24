//! UI tests for mesh warp: Mesh Warp in the selection options puts a grid over the selection,
//! dragging a grid point and Apply bends the pixels as one undo step. The accessibility check
//! is on.

use egui::{Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

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

fn alpha(h: &Harness<'static, ToonApp>, x: i64, y: i64) -> u8 {
    let app = h.state();
    let p = &app.engine.project;
    p.drawing_at(app.layer, app.frame).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)[3]).unwrap_or(0)
}

#[test]
fn mesh_warp_bends_the_selection_by_its_grid() {
    let mut h = harness();
    let (layer, frame) = (h.state().layer, h.state().frame);
    let sq = vec![(300.0, 300.0), (420.0, 300.0), (420.0, 420.0), (300.0, 420.0)];
    let c = [0, 120, 0, 255];
    assert!(h.state_mut().run(Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(sq.clone()),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new()
    }));
    h.state_mut().tool = Tool::Select;
    crate::selection_tools::set_rings(h.state_mut(), vec![sq]);
    h.state_mut().sel.mesh_cells = 2;
    h.run();
    let start = By::new().role(egui::Role::Button).label("Mesh Warp");
    h.get(start.clone()).scroll_to_me();
    h.run();
    h.get(start).click();
    h.run();
    let (cols, rows, pts) = h.state().sel.mesh.clone().expect("a grid");
    assert_eq!((cols, rows, pts.len()), (2, 2, 9));
    // drag the middle point 60 px right
    let (a, b) = (screen(&h, 360.0, 360.0), screen(&h, 420.0, 360.0));
    h.drag_at(a);
    h.run();
    for k in 1..=6 {
        h.hover_at(a + (b - a) * (k as f32 / 6.0));
        h.run();
    }
    h.drop_at(b);
    h.run();
    let moved = h.state().sel.mesh.as_ref().and_then(|m| m.2.get(4).copied()).expect("grid");
    assert!((moved.0 - 420.0).abs() < 3.0, "the middle point followed: {moved:?}");
    let undo = h.state().engine.undo_len();
    h.get(By::new().role(egui::Role::Button).label("Apply Mesh Warp")).click();
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo + 1);
    assert_eq!(h.state().engine.undo_label(), Some("Mesh warp"));
    assert!(h.state().sel.mesh.is_none());
    // the right half was squeezed against the right edge: the old middle column of paint moved
    // right, the outline stayed, so the left half was stretched
    assert!(alpha(&h, 410, 360) > 0 && alpha(&h, 305, 360) > 0, "still inside");
    assert_eq!(alpha(&h, 440, 360), 0, "the right edge stayed put");
}
