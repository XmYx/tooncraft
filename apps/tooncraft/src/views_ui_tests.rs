//! UI tests for the second view and the navigator: drawing in the second view with its own pan
//! and zoom, the camera look's picture, the side look's depth by drag and by keys, and the
//! navigator moving the main view by click and by keys. The accessibility check is on.

use egui::{Key, Modifiers, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;
use crate::views::Look;

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

fn second_view(h: &mut Harness<'static, ToonApp>, look: Look) {
    h.state_mut().view2.look = look;
    h.state_mut().workspace.raise(Pane::SecondView);
    h.run();
    h.run();
}

fn drag(h: &mut Harness<'static, ToonApp>, from: Pos2, to: Pos2) {
    h.drag_at(from);
    h.run();
    for i in 1..=6 {
        h.hover_at(from + (to - from) * (i as f32 / 6.0));
        h.run();
    }
    h.drop_at(to);
    h.run();
    h.run();
}

fn press(h: &mut Harness<'static, ToonApp>, key: Key) {
    h.key_press_modifiers(Modifiers::NONE, key);
    h.run();
}

#[test]
fn a_stroke_in_the_second_view_draws_on_the_shot_and_leaves_the_main_view_alone() {
    let mut h = harness();
    second_view(&mut h, Look::Shot);
    h.state_mut().tool = Tool::Pencil;
    let main_view = h.state().view;
    let r = h.state().view2.rect;
    assert!(r.width() > 100.0 && r.height() > 100.0, "the second view is shown: {r:?}");
    let depth = h.state().engine.undo_len();
    drag(&mut h, r.center() - vec2(40.0, 10.0), r.center() + vec2(40.0, 20.0));
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one stroke, one undo step");
    assert_eq!(h.state().view, main_view, "the main view did not move");
    // the second view is named for screen readers, the main canvas keeps its own name
    h.get(By::new().role(egui::Role::Canvas).label_contains("Second view:"));
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas:"));
}

#[test]
fn the_second_view_pans_and_zooms_on_its_own() {
    let mut h = harness();
    second_view(&mut h, Look::Camera);
    let (main, before) = (h.state().view, h.state().view2.view);
    let (frame, undo) = (h.state().frame, h.state().engine.undo_len());
    h.get(By::new().role(egui::Role::Image).label("Camera look")).focus();
    h.run();
    press(&mut h, Key::Plus);
    press(&mut h, Key::ArrowLeft);
    let after = h.state().view2.view;
    assert!(after.zoom > before.zoom * 1.2, "{before:?} -> {after:?}");
    assert_ne!(after.pan, before.pan);
    assert_eq!(h.state().view, main, "the main view did not move");
    // + and the arrows belong to the focused view, not to the frame holds and steps
    assert_eq!(h.state().frame, frame, "the frame did not step");
    assert_eq!(h.state().engine.undo_len(), undo, "no hold was added");
}

#[test]
fn the_camera_look_films_the_frame_off_the_ui_thread() {
    let mut h = harness();
    second_view(&mut h, Look::Camera);
    let mut filmed = false;
    for _ in 0..400 {
        h.run();
        let (changes, frame) = (h.state().engine.changes(), h.state().frame);
        if h.state().view2.filmed_now(changes, frame) {
            filmed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(filmed, "the camera picture arrived");
    // the picture belongs to its frame: another frame is not claimed as filmed before it is
    let other = h.state().frame + 3;
    let changes = h.state().engine.changes();
    assert!(!h.state().view2.filmed_now(changes, other));
}

#[test]
fn the_side_look_moves_a_layer_in_depth_by_drag_and_by_keys() {
    let mut h = harness();
    second_view(&mut h, Look::Side);
    let layer = h.state().layer;
    assert_eq!(h.state().engine.project.layers[layer].depth, 0.0);
    let rect = h.state().view2.rect;
    let undo = h.state().engine.undo_len();
    // drag the current layer's line farther away
    let from = crate::views::side_point(h.state(), rect, 0.0);
    let to = crate::views::side_point(h.state(), rect, 0.8);
    drag(&mut h, from, to);
    let d = h.state().engine.project.layers[layer].depth;
    assert!((d - 0.8).abs() < 0.05, "dragged to depth {d}");
    assert_eq!(h.state().engine.undo_len(), undo + 1, "one undo step for the drag");
    assert_eq!(h.state().engine.undo_label(), Some("Layer depth"));
    // a drag cancelled with Escape changes nothing
    let from = crate::views::side_point(h.state(), rect, d);
    h.drag_at(from);
    h.run();
    h.hover_at(from + vec2(0.0, -40.0));
    h.run();
    h.hover_at(from + vec2(0.0, -60.0));
    h.run();
    press(&mut h, Key::Escape);
    h.drop_at(from + vec2(0.0, -60.0));
    h.run();
    h.run();
    assert_eq!(h.state().engine.project.layers[layer].depth, d);
    assert_eq!(h.state().engine.undo_len(), undo + 1);
    // Up and Down from the keyboard, each one step
    h.get(By::new().role(egui::Role::Canvas).label_contains("Side look:")).focus();
    h.run();
    press(&mut h, Key::ArrowUp);
    let up = h.state().engine.project.layers[layer].depth;
    assert!((up - d - 0.1).abs() < 1e-3, "{d} -> {up}");
    press(&mut h, Key::ArrowDown);
    press(&mut h, Key::ArrowDown);
    let down = h.state().engine.project.layers[layer].depth;
    assert!((down - d + 0.1).abs() < 1e-3, "{d} -> {down}");
    assert_eq!(h.state().engine.undo_len(), undo + 4);
}

#[test]
fn the_navigator_moves_the_main_view_by_click_and_by_keys() {
    let mut h = harness();
    h.state_mut().workspace.raise(Pane::Navigator);
    h.run();
    h.run();
    // zoom the main view in, so it shows only part of the picture
    let ctx = h.ctx.clone();
    for _ in 0..4 {
        crate::commands::run(h.state_mut(), &ctx, "view.zoom_in");
    }
    h.run();
    let nav = h.state().view2.nav_rect;
    assert!(nav.width() > 50.0, "the navigator is shown: {nav:?}");
    let size = vec2(h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let target = pos2(size.x * 0.25, size.y * 0.3);
    let at = crate::views::navigator_point(nav, size, target);
    h.drag_at(at);
    h.drop_at(at);
    h.run();
    h.run();
    let c = crate::views::main_view_centre(h.state());
    assert!((c - target).length() < size.x * 0.02, "{c:?} is near {target:?}");
    // a press of the right arrow moves the view a tenth of the picture to the right
    h.get(By::new().role(egui::Role::Canvas).label("Navigator")).focus();
    h.run();
    press(&mut h, Key::ArrowRight);
    let after = crate::views::main_view_centre(h.state());
    assert!((after.x - c.x - size.x * 0.1).abs() < 1.0 && (after.y - c.y).abs() < 1.0, "{c:?} -> {after:?}");
}

#[test]
fn every_look_is_picked_from_the_keyboard() {
    let mut h = harness();
    second_view(&mut h, Look::Shot);
    for look in [Look::Drawing, Look::Camera, Look::Side, Look::Shot] {
        h.get(By::new().role(egui::Role::RadioButton).label(&format!("{} look", look.label()))).focus();
        h.run();
        press(&mut h, Key::Space);
        assert_eq!(h.state().view2.look, look);
    }
}

#[test]
fn the_launch_option_picks_the_look() {
    let parsed = crate::args::parse(["--second-view", "side"].iter().map(|s| s.to_string()));
    assert!(matches!(parsed, Ok(crate::args::Launch::Run(ref c)) if c.second_view.as_deref() == Some("side")), "{parsed:?}");
    assert!(crate::args::parse(["--second-view", "top"].iter().map(|s| s.to_string())).is_err());
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli { second_view: Some("camera".into()), ..Default::default() }).without_autosave();
    assert_eq!(app.view2.look, Look::Camera);
    assert!(app.workspace.is_open(Pane::SecondView));
}

/// Starts a side-look depth drag on the current layer and leaves the button held.
fn start_side_drag(h: &mut Harness<'static, ToonApp>) {
    let rect = h.state().view2.rect;
    let from = crate::views::side_point(h.state(), rect, 0.0);
    h.drag_at(from);
    h.run();
    for i in 1..=3 {
        h.hover_at(from + vec2(0.0, -30.0 * i as f32));
        h.run();
    }
    assert!(h.state().view2.dragging(), "the drag is under way");
}

#[test]
fn switching_the_look_mid_drag_drops_the_depth_preview() {
    let mut h = harness();
    second_view(&mut h, Look::Side);
    let layer = h.state().layer;
    let undo = h.state().engine.undo_len();
    start_side_drag(&mut h);
    // switch the look from the keyboard with the button still held
    h.get(By::new().role(egui::Role::RadioButton).label("Shot look")).focus();
    h.run();
    press(&mut h, Key::Space);
    assert_eq!(h.state().view2.look, Look::Shot);
    assert!(!h.state().view2.dragging(), "the drag ended with the look");
    h.drop_at(h.state().view2.rect.center());
    h.run();
    // back in the side look the layer is drawn at the project's depth, and nothing was undone or added
    second_view(&mut h, Look::Side);
    assert!(!h.state().view2.dragging());
    assert_eq!(h.state().engine.project.layers[layer].depth, 0.0);
    assert_eq!(h.state().engine.undo_len(), undo);
}

#[test]
fn a_release_the_side_look_never_saw_drops_the_depth_preview() {
    let mut h = harness();
    second_view(&mut h, Look::Side);
    let undo = h.state().engine.undo_len();
    start_side_drag(&mut h);
    // another panel covers the second view, and the button is let go there
    h.state_mut().workspace.raise(Pane::Colour);
    h.run();
    h.drop_at(h.state().view2.rect.center());
    h.run();
    h.state_mut().workspace.raise(Pane::SecondView);
    h.run();
    h.run();
    assert!(!h.state().view2.dragging(), "no stale drag when the side look shows again");
    assert_eq!(h.state().engine.undo_len(), undo);
}
