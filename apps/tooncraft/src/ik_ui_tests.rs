//! UI tests for inverse kinematics with the Transform tool: with IK on, moving a hand peg turns
//! the arm it hangs from (one undo step), from the keyboard and with the pointer.

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::PegId;
use tooncraft_doc::peg::pivot_on_canvas;
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

/// Shoulder, elbow and hand pegs across the canvas; the layer hangs from the hand.
fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(egui::vec2(1500.0, 1100.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    let s = h.state_mut();
    for (name, parent, x) in [("Shoulder", None, 300.0), ("Elbow", Some(PegId(1)), 500.0), ("Hand", Some(PegId(2)), 700.0)] {
        s.run(Command::AddPeg { name: name.into(), parent });
        let id = s.engine.project.pegs.last().map(|p| p.id).unwrap_or(PegId(0));
        s.run(Command::SetPegPivot { peg: id, x, y: 300.0 });
    }
    s.run(Command::SetLayerPeg { layer: 0, peg: Some(PegId(3)) });
    s.tool = Tool::Transform;
    h.run();
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Inverse Kinematics")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().transform_ui.ik);
    h
}

fn hand(h: &Harness<'static, ToonApp>) -> (f32, f32) {
    pivot_on_canvas(&h.state().engine.project.pegs, PegId(3), 0).unwrap_or((0.0, 0.0))
}

fn has_key(h: &Harness<'static, ToonApp>, id: u64) -> bool {
    h.state().engine.project.peg(PegId(id)).is_some_and(|p| p.has_key(0))
}

#[test]
fn a_keyboard_nudge_with_ik_turns_the_arm_and_leaves_the_hand_unkeyed() {
    let mut h = harness();
    h.ctx.memory_mut(|m| m.request_focus(crate::canvas::canvas_id()));
    h.run();
    let depth = h.state().engine.undo_len();
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowUp);
    h.run();
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step");
    assert_eq!(h.state().engine.undo_label(), Some("Move with IK"));
    let (x, y) = hand(&h);
    assert!((x - 700.0).abs() < 1.0 && (y - 290.0).abs() < 1.0, "the hand went up ten pixels: {x}, {y}");
    assert!(has_key(&h, 1) && has_key(&h, 2), "shoulder and elbow are keyed");
    assert!(!has_key(&h, 3), "the hand itself is not moved");
    // one joint: only the elbow turns
    h.state_mut().engine.undo().unwrap();
    h.state_mut().transform_ui.ik_joints = 1;
    h.run();
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowUp);
    h.run();
    assert!(!has_key(&h, 1) && has_key(&h, 2));
}

#[test]
fn a_drag_with_ik_pulls_the_arm_after_the_hand_in_one_undo_step() {
    let mut h = harness();
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    let at = |x, y| s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y));
    // away from the hand's own handles: the middle of the drawing area moves it
    let (from, to) = (at(640.0, 420.0), at(640.0, 420.0) + egui::vec2(-60.0, 60.0));
    let ppc = (at(100.0, 0.0).x - at(0.0, 0.0).x) / 100.0;
    let depth = h.state().engine.undo_len();
    h.drag_at(from);
    h.run();
    h.hover_at(from + (to - from) * 0.5);
    h.run();
    h.hover_at(to);
    h.run();
    assert_eq!(h.state().engine.undo_len(), depth, "nothing changes while dragging");
    h.drop_at(to);
    h.run();
    h.run();
    assert_eq!(h.state().engine.undo_len(), depth + 1);
    let (x, y) = hand(&h);
    let want = (700.0 - 60.0 / ppc, 300.0 + 60.0 / ppc);
    assert!((x - want.0).hypot(y - want.1) < 1.5, "the hand followed the pointer: {x}, {y} (want {want:?})");
    let elbow = pivot_on_canvas(&h.state().engine.project.pegs, PegId(2), 0).unwrap_or((0.0, 0.0));
    assert!(((elbow.0 - 300.0).hypot(elbow.1 - 300.0) - 200.0).abs() < 0.1, "the upper arm keeps its length: {elbow:?}");
}
