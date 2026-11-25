//! UI tests for Paint Like This Drawing: from the paint bucket's options, the next drawings
//! take the colours of the current one, as one undo step. The accessibility check is on.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

const INK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [220, 30, 30, 255];

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1800.0, 940.0)).with_accessibility_check(true).build_ui_state(
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

fn ring(frame: u32, dx: f32) -> [Command; 2] {
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
    [
        Command::NewDrawing { layer: 0, frame },
        Command::Gradient {
            layer: 0,
            frame,
            kind: Default::default(),
            from: (0.0, 0.0),
            to: (1.0, 0.0),
            start: INK,
            end: INK,
            area: Some(r(200.0 + dx, 200.0, 400.0 + dx, 400.0)),
            area_rings: vec![r(206.0 + dx, 206.0, 394.0 + dx, 394.0)],
            feather: 0.0,
            stops: Vec::new(),
        },
    ]
}

fn px(h: &Harness<'static, ToonApp>, frame: u32, x: i64, y: i64) -> [u8; 4] {
    let p = &h.state().engine.project;
    p.drawing_at(0, frame).and_then(|d| p.drawing(d)).map(|s| tooncraft_raster::unpremultiply(s.pixel(x, y))).unwrap_or([0; 4])
}

#[test]
fn paint_like_this_drawing_colours_the_next_drawings() {
    let mut h = harness();
    for (f, dx) in [(0, 0.0), (1, 6.0), (2, 12.0)] {
        for c in ring(f, dx) {
            assert!(h.state_mut().run(c));
        }
    }
    assert!(h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 300, y: 300, color: RED, options: Default::default(), reference: Default::default() }));
    let app = h.state_mut();
    app.layer = 0;
    app.frame = 0;
    app.sel_anchor = None;
    app.tool = Tool::Fill;
    app.aids.auto_paint_count = 2;
    h.run();
    let undo = h.state().engine.undo_len();
    let button = By::new().role(egui::Role::Button).label("Paint Like This Drawing");
    h.get(button.clone()).scroll_to_me();
    h.run();
    h.get(button).click();
    h.run();
    assert_eq!(h.state().engine.undo_len(), undo + 1, "one undo step");
    for f in [1, 2] {
        assert_eq!(px(&h, f, 310, 300), RED, "frame {f} painted like frame 0");
        assert_eq!(px(&h, f, 50, 50)[3], 0, "frame {f}: outside left empty");
    }
}
