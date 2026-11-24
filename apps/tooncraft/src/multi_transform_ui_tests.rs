//! UI tests for transforming several drawings at once: with cells selected on two layers and
//! "All selected cells" ticked, a move changes both drawings as one undo step. The accessibility
//! check is on.

use egui::vec2;
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

fn square(layer: usize, frame: u32) -> Command {
    let c = [200, 0, 0, 255];
    Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: c,
        end: c,
        area: Some(vec![(100.0, 100.0), (160.0, 100.0), (160.0, 160.0), (100.0, 160.0)]),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new(),
    }
}

fn alpha(h: &Harness<'static, ToonApp>, layer: usize, x: i64, y: i64) -> u8 {
    let p = &h.state().engine.project;
    p.drawing_at(layer, 0).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)[3]).unwrap_or(0)
}

#[test]
fn all_selected_cells_moves_every_drawing_in_the_block_as_one_step() {
    let mut h = harness();
    assert!(h.state_mut().run(Command::AddLayer { name: "Second".into(), index: None }));
    assert_eq!(h.state().engine.project.layers.len(), 2);
    assert!(h.state_mut().run(square(0, 0)));
    assert!(h.state_mut().run(square(1, 0)));
    // a block over layers 0 and 1 at frame 0, a selection round the squares
    h.state_mut().layer = 1;
    h.state_mut().frame = 0;
    h.state_mut().sel_anchor = Some((0, 0));
    h.state_mut().tool = Tool::Select;
    crate::selection_tools::set_rings(h.state_mut(), vec![vec![(90.0, 90.0), (170.0, 90.0), (170.0, 170.0), (90.0, 170.0)]]);
    h.run();
    let tick = By::new().role(egui::Role::CheckBox).label("All selected cells");
    h.get(tick.clone()).scroll_to_me();
    h.run();
    h.get(tick).click();
    h.run();
    assert!(h.state().sel.all_cells);
    let undo = h.state().engine.undo_len();
    h.state_mut().transform_selection(200.0, 0.0, (1.0, 1.0), 0.0);
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo + 1, "one undo step for both drawings");
    assert_eq!(h.state().engine.undo_label(), Some("Transform drawings"));
    for layer in [0, 1] {
        assert_eq!(alpha(&h, layer, 130, 130), 0, "layer {layer}: moved away");
        assert!(alpha(&h, layer, 330, 130) > 0, "layer {layer}: moved here");
    }
    // without the option only the current drawing moves
    h.state_mut().engine.undo().unwrap();
    h.state_mut().sel.all_cells = false;
    crate::selection_tools::set_rings(h.state_mut(), vec![vec![(90.0, 90.0), (170.0, 90.0), (170.0, 170.0), (90.0, 170.0)]]);
    h.state_mut().transform_selection(200.0, 0.0, (1.0, 1.0), 0.0);
    h.run();
    assert!(alpha(&h, 0, 130, 130) > 0 && alpha(&h, 1, 330, 130) > 0, "only layer 1 moved");
}

fn canvas_at(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> egui::Pos2 {
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(rect, size, egui::pos2(x, y))
}

/// Two layers with a square each, a block over both at frame 0, the option on.
fn two_squares() -> Harness<'static, ToonApp> {
    let mut h = harness();
    assert!(h.state_mut().run(Command::AddLayer { name: "Second".into(), index: None }));
    assert!(h.state_mut().run(square(0, 0)));
    assert!(h.state_mut().run(square(1, 0)));
    let app = h.state_mut();
    app.layer = 1;
    app.frame = 0;
    app.sel_anchor = Some((0, 0));
    app.tool = Tool::Select;
    app.sel.all_cells = true;
    crate::selection_tools::set_rings(app, vec![vec![(90.0, 90.0), (170.0, 90.0), (170.0, 170.0), (90.0, 170.0)]]);
    h.run();
    h
}

#[test]
fn a_canvas_drag_previews_then_moves_every_drawing_once() {
    let mut h = two_squares();
    let undo = h.state().engine.undo_len();
    let (a, b) = (canvas_at(&h, 130.0, 130.0), canvas_at(&h, 330.0, 130.0));
    h.drag_at(a);
    h.run();
    for k in 1..=8 {
        h.hover_at(a + (b - a) * (k as f32 / 8.0));
        h.run();
        // while dragging only the current drawing follows
        if k == 8 {
            assert_eq!(alpha(&h, 0, 330, 130), 0, "layer 0 waits for the release");
        }
    }
    h.drop_at(b);
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo + 1, "exactly one undo step after the drag");
    assert_eq!(h.state().engine.undo_label(), Some("Transform drawings"));
    for layer in [0, 1] {
        assert!(alpha(&h, layer, 330, 130) > 0 && alpha(&h, layer, 130, 130) == 0, "layer {layer} moved");
    }
}

#[test]
fn locked_layers_stay_and_the_limits_are_reported() {
    // a locked layer in the block keeps its drawing where it was
    let mut h = two_squares();
    assert!(h.state_mut().run(Command::SetLayer { layer: 0, visible: None, locked: Some(true), opacity: None }));
    h.state_mut().transform_selection(200.0, 0.0, (1.0, 1.0), 0.0);
    h.run();
    assert!(alpha(&h, 0, 130, 130) > 0, "locked layer 0 stayed");
    assert!(alpha(&h, 1, 330, 130) > 0, "layer 1 moved");
    // an empty block says so and changes nothing
    let mut h = two_squares();
    let app = h.state_mut();
    app.frame = 3;
    app.sel_anchor = Some((0, 2));
    let undo = app.engine.undo_len();
    app.transform_selection(10.0, 0.0, (1.0, 1.0), 0.0);
    assert_eq!(app.engine.undo_len(), undo);
    assert_eq!(app.status, "No drawings in the selected cells to transform");
    // more drawings than one batch holds is refused, with no undo step
    let mut h = harness();
    let n = tooncraft_engine::MAX_BATCH as u32 + 1;
    assert!(h.state_mut().run(Command::SetLength { frames: n }));
    for f in 0..n {
        assert!(h.state_mut().run(Command::NewDrawing { layer: 0, frame: f }), "frame {f}");
    }
    let app = h.state_mut();
    app.layer = 0;
    app.frame = n - 1;
    app.sel_anchor = Some((0, 0));
    app.sel.all_cells = true;
    crate::selection_tools::set_rings(app, vec![vec![(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (0.0, 50.0)]]);
    let undo = app.engine.undo_len();
    app.transform_selection(10.0, 0.0, (1.0, 1.0), 0.0);
    assert_eq!(app.engine.undo_len(), undo, "refused");
    assert!(app.status.starts_with("Select at most"), "{}", app.status);
}
