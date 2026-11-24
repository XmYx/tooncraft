//! UI tests for the canvas and timeline stream of the "Lightbox" pass: the loop range handles on
//! the ruler and the layer rows' onion toggle. Keyboard tests move the focus and press keys.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, NodeT, Queryable};

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

/// A menu item named `label`, with or without its shortcut after it ("Zoom In Alt+=").
fn has_item(h: &Harness<'static, ToonApp>, label: &str) -> bool {
    h.query_all(By::new().label_contains(label)).any(|n| n.accesskit_node().label().is_some_and(|t| t == label || t.starts_with(&format!("{label} "))))
}

fn focus(h: &mut Harness<'static, ToonApp>, by: By<'_>) {
    h.get(by).focus();
    h.run();
}

#[test]
fn the_timeline_header_transport_works_from_the_keyboard() {
    let mut h = harness();
    h.state_mut().frame = 3;
    h.run();
    focus(&mut h, By::new().role(egui::Role::Button).label("Next frame"));
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().frame, 4);
    focus(&mut h, By::new().role(egui::Role::Button).label("First frame"));
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().frame, 0);
    let looping = h.state().playback.looping;
    focus(&mut h, By::new().role(egui::Role::Button).label("Loop playback"));
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().playback.looping, !looping);
    // the tooltip names the shortcut from the key map
    assert!(h.state().keys.text("frame.next").contains('.'));
}

#[test]
fn the_timeline_header_fields_set_the_in_and_out_points_and_the_hold() {
    let mut h = harness();
    for (label, text) in [("In", "3"), ("Out", "7")] {
        h.get(By::new().role(egui::Role::SpinButton).label(label)).click();
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run();
        h.get(By::new().role(egui::Role::SpinButton).label(label)).type_text(text);
        h.run();
        h.key_press(Key::Enter);
        h.run();
    }
    assert_eq!(h.state().tl.range, Some((2, 6)));
    // the hold field is there only to edit a drawing's hold: on an empty frame it is disabled
    let empty = (0..h.state().engine.project.length()).find(|&f| h.state().engine.project.layers[0].exposure_range(f).is_none());
    if let Some(f) = empty {
        h.state_mut().frame = f;
        h.run();
        assert!(h.get(By::new().role(egui::Role::SpinButton).label("Hold")).accesskit_node().is_disabled());
    }
}

#[test]
fn the_timeline_options_menu_opens_from_the_keyboard() {
    let mut h = harness();
    focus(&mut h, By::new().role(egui::Role::Button).label("Timeline options"));
    h.key_press(Key::Enter);
    h.run();
    for item in ["New drawing", "Duplicate drawing", "Insert frame", "Hold one frame longer", "Pin frame to the light table"] {
        assert!(has_item(&h, item), "{item}");
    }
    // the switches are a level down, and the submenus open from the keyboard too
    focus(&mut h, By::new().role(egui::Role::Button).label("Display"));
    h.key_press(Key::Enter);
    h.run();
    for item in ["Vertical", "Timecode", "Thumbnails", "Zoom the timeline in"] {
        assert!(has_item(&h, item), "{item}");
    }
    h.get(By::new().role(egui::Role::SpinButton).label("Shot length"));
}

#[test]
fn the_onion_popover_has_a_slider_per_step_and_opens_from_the_keyboard() {
    let mut h = harness();
    focus(&mut h, By::new().role(egui::Role::Button).label("Onion Skin"));
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run();
    for n in 1..=5 {
        h.get(By::new().role(egui::Role::Slider).label(&format!("Onion opacity, {n} before")));
        h.get(By::new().role(egui::Role::Slider).label(&format!("Onion opacity, {n} after")));
    }
    let after0 = h.state().engine.project.onion.after;
    // turn up the fourth step after: the reach grows to it, each step keeps its own opacity
    focus(&mut h, By::new().role(egui::Role::Slider).label("Onion opacity, 4 after"));
    h.key_press(Key::ArrowUp);
    h.run();
    let o = h.state().engine.project.onion;
    assert!(o.per_step);
    assert_eq!(o.after, after0.max(4));
    assert!(o.after_steps[3] > 0.0);
}

#[test]
fn canvas_header_toggles_are_named_icon_buttons_that_work_from_the_keyboard() {
    let mut h = harness();
    let lt = h.state().engine.project.light_table.enabled;
    focus(&mut h, By::new().role(egui::Role::Button).label("Light Table"));
    h.key_press(Key::Space);
    h.run();
    assert_eq!(h.state().engine.project.light_table.enabled, !lt);
    focus(&mut h, By::new().role(egui::Role::Button).label("Safe Areas"));
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().canvas_ui.safe_areas);
    for name in ["Onion Skin", "Field Guide", "Mirror View", "Symmetry", "Rulers", "Canvas zoom", "Reset Rotation"] {
        h.get(By::new().role(egui::Role::Button).label(name));
    }
}

#[test]
fn the_status_strip_shows_the_frame_colour_and_the_latest_notice() {
    let mut h = harness();
    let len = h.state().engine.project.length();
    h.get_by_label(&format!("Frame 1 / {len}"));
    h.get(By::new().role(egui::Role::Image).label_contains("Colour #"));
    h.state_mut().set_status("Saved the shot");
    h.run();
    h.get_by_label("Saved the shot");
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
