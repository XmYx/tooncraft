//! UI tests for the Transform tool: dragging a layer's peg on the canvas, the keyboard nudges and
//! the typed values in the tool options.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::{PegId, PegKey};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

fn harness(with_peg: bool) -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1000.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    let s = h.state_mut();
    if with_peg {
        s.run(Command::AddPeg { name: "Body".into(), parent: None });
        s.run(Command::SetLayerPeg { layer: 0, peg: Some(PegId(1)) });
    }
    s.tool = Tool::Transform;
    h.run();
    h.run();
    h
}

/// The screen point of a canvas pixel.
fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> egui::Pos2 {
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y))
}

fn key(h: &Harness<'static, ToonApp>) -> PegKey {
    h.state().engine.project.peg(PegId(1)).map(|p| p.at(0)).unwrap_or(PegKey::rest(0))
}

fn drag(h: &mut Harness<'static, ToonApp>, from: egui::Pos2, to: egui::Pos2, cancel: bool) {
    h.drag_at(from);
    h.run();
    h.hover_at(from + (to - from) * 0.5);
    h.run();
    h.hover_at(to);
    h.run();
    if cancel {
        h.key_press(Key::Escape);
        h.run();
    }
    h.drop_at(to);
    h.run();
    h.run();
}

#[test]
fn a_drag_on_the_canvas_is_one_peg_key_equal_to_the_typed_values() {
    let mut h = harness(true);
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    // away from the pivot, which a new peg puts in the middle
    let (cx, cy) = (w * 0.5 + 120.0, hh * 0.5 + 90.0);
    let depth = h.state().engine.undo_len();
    let (a, b) = (screen(&h, cx, cy), screen(&h, cx + 40.0, cy - 20.0));
    drag(&mut h, a, b, false);
    let k = key(&h);
    assert!(h.state().engine.project.peg(PegId(1)).is_some_and(|p| p.pivot == (w * 0.5, hh * 0.5)), "the pivot stays");
    assert!((k.x - 40.0).abs() < 0.6 && (k.y + 20.0).abs() < 0.6, "{k:?}");
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step for the drag");
    assert_eq!(h.state().engine.undo_label(), Some("Peg key"));
    let dragged = h.state().engine.project.layer_transform(0, 0);
    // the same values typed into the tool options give the same transform
    h.state_mut().engine.undo().unwrap();
    h.run();
    for (label, v) in [("Move X", format!("{:.2}", k.x)), ("Move Y", format!("{:.2}", k.y))] {
        h.get(By::new().role(egui::Role::SpinButton).label(label)).click();
        h.run();
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.get(By::new().role(egui::Role::SpinButton).label(label)).type_text(&v);
        h.key_press(Key::Enter);
        h.run();
        h.run();
    }
    let typed = h.state().engine.project.layer_transform(0, 0);
    let (a, b) = (dragged.apply(10.0, 10.0), typed.apply(10.0, 10.0));
    assert!((a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01, "{a:?} {b:?}");
}

#[test]
fn the_knob_turns_a_corner_scales_and_escape_cancels() {
    let mut h = harness(true);
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    assert!(h.state().engine.project.peg(PegId(1)).is_some_and(|p| p.pivot == (w * 0.5, hh * 0.5)), "a new peg turns about the middle");
    // the knob above the box, swept a quarter turn about the pivot, turns the peg 90°
    let c = screen(&h, w * 0.5, hh * 0.5);
    let knob = screen(&h, w * 0.5, 0.0) - egui::vec2(0.0, 28.0);
    let r = knob - c;
    let quarter = c + egui::vec2(-r.y, r.x);
    drag(&mut h, knob, quarter, false);
    assert!((key(&h).angle - 90.0).abs() < 1.0, "{:?}", key(&h));
    h.state_mut().engine.undo().unwrap();
    h.run();
    // a corner dragged twice as far from the pivot doubles the scale
    let corner = screen(&h, w, hh);
    let further = screen(&h, w * 0.5 + w, hh * 0.5 + hh);
    drag(&mut h, corner, further, false);
    let k = key(&h);
    assert!((k.scale_x - 2.0).abs() < 0.02 && (k.scale_y - 2.0).abs() < 0.02, "{k:?}");
    // a drag cancelled with Escape leaves the key and the history alone
    let depth = h.state().engine.undo_len();
    let (a, b) = (screen(&h, w * 0.5 + 30.0, hh * 0.5 + 30.0), screen(&h, w * 0.5 + 200.0, hh * 0.5));
    drag(&mut h, a, b, true);
    assert_eq!(key(&h), k);
    assert_eq!(h.state().engine.undo_len(), depth);
    assert!(!h.state().transform_ui.dragging());
}

#[test]
fn the_keyboard_nudges_turns_and_scales_the_peg() {
    let mut h = harness(true);
    h.ctx.memory_mut(|m| m.request_focus(crate::canvas::canvas_id()));
    h.run();
    let depth = h.state().engine.undo_len();
    for (m, k) in [
        (Modifiers::NONE, Key::ArrowRight),
        (Modifiers::NONE, Key::ArrowRight),
        (Modifiers::SHIFT, Key::ArrowDown),
        (Modifiers::NONE, Key::CloseBracket),
        (Modifiers::SHIFT, Key::OpenBracket),
        (Modifiers::NONE, Key::PageUp),
    ] {
        h.key_press_modifiers(m, k);
        h.run();
    }
    let k = key(&h);
    assert_eq!((k.x, k.y), (2.0, 10.0));
    assert!((k.angle + 14.0).abs() < 1e-4, "{}", k.angle);
    assert!((k.scale_x - 1.01).abs() < 1e-4);
    assert_eq!(h.state().engine.undo_len(), depth + 6, "one step per key press");
}

#[test]
fn dragging_the_pivot_moves_it_under_the_pointer() {
    let mut h = harness(true);
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let (a, b) = (screen(&h, w * 0.5, hh * 0.5), screen(&h, w * 0.25, hh * 0.25));
    drag(&mut h, a, b, false);
    let p = h.state().engine.project.peg(PegId(1)).map(|p| p.pivot).unwrap_or_default();
    assert!((p.0 - w * 0.25).abs() < 1.0 && (p.1 - hh * 0.25).abs() < 1.0, "{p:?}");
    assert_eq!(h.state().engine.undo_label(), Some("Peg pivot"));
    assert_eq!(key(&h), PegKey::rest(0), "no key was set");
}

#[test]
fn a_layer_without_a_peg_gets_one_from_the_tool_options() {
    let mut h = harness(false);
    h.get_by_label("Add a Peg for This Layer").click();
    h.run();
    h.run();
    assert_eq!(h.state().engine.project.layers[0].peg, Some(PegId(1)));
    h.get(By::new().role(egui::Role::SpinButton).label("Rotation"));
}
