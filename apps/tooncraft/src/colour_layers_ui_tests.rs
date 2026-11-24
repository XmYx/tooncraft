//! UI tests for line and colour art: Add Colour Layer from a layer's menu, painting colour that
//! follows the line layer's timing, and Stop Following. Menus open with a secondary click on the
//! layer row; items are chosen with focus and Enter. The accessibility check is on.

use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_brush::{Brush, BrushKind, Sample};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_accessibility_check(true).with_max_steps(8).build_ui_state(
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

fn open_layer_menu(h: &mut Harness<'static, ToonApp>, li: usize) {
    let name = h.state().engine.project.layers[li].name.clone();
    h.get(By::new().role(egui::Role::Button).label(&format!("Layer {name}"))).click_secondary();
    h.run();
}

fn choose(h: &mut Harness<'static, ToonApp>, item: &str) {
    h.get_by_label(item).focus();
    h.run();
    assert!(h.get_by_label(item).is_focused(), "{item} has the focus");
    h.key_press(Key::Enter);
    h.run();
}

fn stroke(layer: usize, frame: u32) -> Command {
    let samples = (0..10).map(|i| Sample::at(100.0 + i as f32 * 8.0, 100.0, 1.0, i as f64 * 0.01)).collect();
    Command::Stroke { layer, frame, brush: Brush { size: 8.0, ..Brush::preset(BrushKind::Ink) }, samples }
}

#[test]
fn a_colour_layer_is_added_from_the_layer_menu_and_follows_the_lines() {
    let mut h = harness();
    assert!(h.state_mut().run(stroke(0, 0)), "line art on frame 1");
    h.run();
    open_layer_menu(&mut h, 0);
    choose(&mut h, "Add Colour Layer");
    let p = &h.state().engine.project;
    assert_eq!(p.layers.len(), 2);
    assert_eq!(p.line_layer_of(0), Some(1), "the new layer below follows the line layer");
    assert_eq!(h.state().layer, 0, "the colour layer is the current layer");
    // colour painted on frame 1 is held when the line drawing is held
    assert!(h.state_mut().run(stroke(0, 0)));
    assert!(h.state_mut().run(Command::SetExposure { layer: 1, frame: 0, length: 3 }));
    h.run();
    let c = h.state().engine.project.drawing_at(0, 0);
    assert!(c.is_some());
    assert_eq!(h.state().engine.project.drawing_at(0, 2), c, "the colour holds with the lines");
    // its menu names the line layer and stops following
    open_layer_menu(&mut h, 0);
    h.get_by_label("Colour of Layer 1");
    choose(&mut h, "Stop Following the Line Layer");
    assert_eq!(h.state().engine.project.layers[0].colour_of, None);
}
