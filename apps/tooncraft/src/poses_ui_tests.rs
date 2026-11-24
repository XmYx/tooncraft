//! UI tests for the pose library in the Pegs panel, driven from the keyboard: name a pose, save
//! it, put it back on another frame, rename and delete it.

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use tooncraft_doc::{Ease, PegId};
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::args::Cli;

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
    s.run(Command::AddPeg { name: "Arm".into(), parent: None });
    s.run(Command::AddPeg { name: "Hand".into(), parent: Some(PegId(1)) });
    s.run(Command::SetPegKey { peg: PegId(1), frame: 0, x: 30.0, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear });
    s.run(Command::SetPegKey { peg: PegId(2), frame: 0, x: 0.0, y: 0.0, angle: 20.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear });
    s.peg_ui.selected = Some(PegId(1));
    s.workspace.raise(crate::dock::Pane::Pegs);
    h.run();
    h.run();
    h
}

/// Focus a button by its label and press Enter.
fn press(h: &mut Harness<'static, ToonApp>, label: &str) {
    h.get_by_label(label).focus();
    h.run();
    h.key_press(Key::Enter);
    h.run();
}

fn type_name(h: &mut Harness<'static, ToonApp>, text: &str) {
    h.get_by_label("Pose name").focus();
    h.run();
    h.get_by_label("Pose name").type_text(text);
    h.run();
}

#[test]
fn a_pose_is_saved_put_back_on_another_frame_renamed_and_deleted_from_the_keyboard() {
    let mut h = harness();
    type_name(&mut h, "Reach");
    press(&mut h, "Save Pose");
    let poses = &h.state().engine.project.poses;
    assert_eq!(poses.len(), 1);
    assert_eq!(poses[0].name, "Reach");
    assert_eq!(poses[0].keys.len(), 2, "the arm and the hand below it");
    assert_eq!(h.state().engine.undo_label(), Some("Save pose"));
    assert!(h.state().peg_ui.pose_name.is_empty(), "the name field clears after saving");
    // move the arm away on frame 9, then put the pose back there
    h.state_mut().run(Command::SetPegKey { peg: PegId(1), frame: 9, x: -80.0, y: 5.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear });
    h.state_mut().frame = 9;
    h.run();
    let depth = h.state().engine.undo_len();
    press(&mut h, "Apply Pose");
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step");
    let at = |h: &Harness<'static, ToonApp>, id| h.state().engine.project.peg(PegId(id)).map(|p| p.at(9));
    assert_eq!(at(&h, 1).map(|k| (k.x, k.y)), Some((30.0, 0.0)));
    assert_eq!(at(&h, 2).map(|k| k.angle), Some(20.0));
    assert_eq!(h.state().status, "Pose “Reach” on frame 10");
    type_name(&mut h, "Reach up");
    press(&mut h, "Rename Pose");
    assert_eq!(h.state().engine.project.poses[0].name, "Reach up");
    press(&mut h, "Delete Pose");
    assert!(h.state().engine.project.poses.is_empty());
    h.get_by_label("No poses yet: pose the pegs, then Save Pose");
}

#[test]
fn an_unnamed_pose_gets_a_numbered_name_and_rename_waits_for_a_name() {
    let mut h = harness();
    press(&mut h, "Save Pose");
    press(&mut h, "Save Pose");
    let names: Vec<String> = h.state().engine.project.poses.iter().map(|p| p.name.clone()).collect();
    assert_eq!(names, ["Pose 1", "Pose 2"]);
    assert_eq!(h.state().peg_ui.pose, Some(1), "the new pose is picked");
    assert!(h.get_by_label("Rename Pose").accesskit_node().is_disabled(), "nothing typed: nothing to rename to");
}
