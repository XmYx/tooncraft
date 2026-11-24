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
