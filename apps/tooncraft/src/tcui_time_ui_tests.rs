//! UI tests for the canvas and timeline stream of the "Lightbox" pass: the loop range handles on
//! the ruler and the layer rows' onion toggle. Keyboard tests move the focus and press keys.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::ToonApp;
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h.run();
    h
}

#[test]
fn loop_handles_move_with_the_arrow_keys_and_never_cross() {
    let mut h = harness();
    h.state_mut().tl.range = Some((4, 10));
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Loop in")).focus();
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    h.key_press(Key::ArrowRight);
    h.run();
    assert_eq!(h.state().tl.range, Some((6, 10)));
    // Shift moves five; past the out point the range closes to one frame instead of flipping
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run();
    assert_eq!(h.state().tl.range, Some((10, 10)));
    h.get(By::new().role(egui::Role::Slider).label("Loop out")).focus();
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowRight);
    h.run();
    assert_eq!(h.state().tl.range, Some((10, 15)));
    // the playback span follows the range
    assert_eq!(crate::timeline_extras::play_span(h.state()), (10, 16));
}

#[test]
fn without_a_range_the_ruler_has_no_loop_handles() {
    let h = harness();
    assert!(h.state().tl.range.is_none());
    assert!(h.query(By::new().role(egui::Role::Slider).label("Loop in")).is_none());
}

#[test]
fn the_onion_toggle_leaves_a_layer_out_of_the_onion_skin_from_the_keyboard() {
    let mut h = harness();
    h.state_mut().engine.project.onion.enabled = true;
    h.run();
    let name = h.state().engine.project.layers[0].name.clone();
    let label = format!("Include {name} in onion skin");
    assert!(h.state().engine.project.onion_for(0).enabled);
    h.get(By::new().role(egui::Role::CheckBox).label(&label)).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(!h.state().engine.project.onion_for(0).enabled, "the layer is left out");
    assert!(h.state().engine.project.onion.enabled, "the shot's onion skin stays on");
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().engine.project.onion_for(0).enabled);
    assert_eq!(h.state().engine.project.layers[0].onion, None, "back to the shot's settings");
}

#[test]
fn dragging_the_loop_out_handle_moves_the_range_and_not_the_playhead() {
    let mut h = harness();
    h.state_mut().tl.range = Some((2, 5));
    h.run();
    let r = h.get(By::new().role(egui::Role::Slider).label("Loop out")).rect();
    let cell_w = h.get(By::new().role(egui::Role::Cell).label_contains("frame 2:")).rect().width();
    let start = r.center();
    let end = start + egui::vec2(cell_w * 4.0, 0.0);
    let frame = h.state().frame;
    h.event(egui::Event::PointerMoved(start));
    h.run();
    h.event(egui::Event::PointerButton { pos: start, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run();
    for i in 1..=8 {
        h.event(egui::Event::PointerMoved(start + (end - start) * (i as f32 / 8.0)));
        h.run();
    }
    h.event(egui::Event::PointerButton { pos: end, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
    assert_eq!(h.state().tl.range, Some((2, 9)));
    assert_eq!(h.state().frame, frame, "the ruler did not scrub under the handle");
}

#[test]
fn the_layers_toolbar_adds_from_the_split_button_by_keyboard_and_rows_show_thumbnails() {
    let mut h = harness();
    h.state_mut().workspace.raise(crate::dock::Pane::Layers);
    h.run();
    let n = h.state().engine.project.layers.len();
    h.get(By::new().role(egui::Role::Button).label("Add")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().engine.project.layers.len(), n + 1, "+ adds a drawing layer");
    // the arrow half offers the other kinds
    h.get(By::new().role(egui::Role::Button).label("Add Other")).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.get_by_label("Reference Image…");
    h.get_by_label("Drawing Layer").click();
    h.run();
    assert_eq!(h.state().engine.project.layers.len(), n + 2);
    // a drawing on this frame gets a thumbnail in its row
    let (layer, frame) = (h.state().layer, h.state().frame);
    let brush = tooncraft_brush::Brush { size: 12.0, ..tooncraft_brush::Brush::preset(tooncraft_brush::BrushKind::Ink) };
    let samples: Vec<_> = (0..10).map(|i| tooncraft_brush::Sample::at(20.0 + i as f32 * 4.0, 30.0, 1.0, i as f64 * 0.01)).collect();
    assert!(h.state_mut().run(tooncraft_engine::Command::Stroke { layer, frame, brush, samples }));
    h.run();
    h.run();
    let d = h.state().engine.project.drawing_at(layer, frame).expect("the stroke made a drawing");
    assert!(h.state().thumbs.contains_key(&d), "the row's thumbnail is made");
    // with only one layer below, Merge Down is offered; on the bottom layer it says why not
    h.state_mut().layer = 0;
    h.run();
    h.get(By::new().role(egui::Role::Button).label("Merge Down"));
}

#[test]
fn the_row_eye_hides_a_layer_from_the_keyboard() {
    let mut h = harness();
    let name = h.state().engine.project.layers[0].name.clone();
    h.get(By::new().role(egui::Role::CheckBox).label(&format!("Show {name}"))).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(!h.state().engine.project.layers[0].visible);
}
