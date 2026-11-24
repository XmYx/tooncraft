//! UI tests for record motion: armed in the Transform tool's options, a drag on the canvas plays
//! the shot and keys every frame the layer passes in one undo step; Escape drops the take; the
//! arrow keys record while the shot plays.

use std::time::Duration;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::{PegId, PegKey};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

/// Playback runs on the wall clock: wait long enough for a few frames to pass at 24 fps.
const FRAMES: Duration = Duration::from_millis(130);

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
    s.run(Command::AddPeg { name: "Body".into(), parent: None });
    s.run(Command::SetLayerPeg { layer: 0, peg: Some(PegId(1)) });
    s.tool = Tool::Transform;
    h.run();
    h.run();
    h
}

fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> egui::Pos2 {
    let s = h.state();
    let size = egui::vec2(s.engine.project.width() as f32, s.engine.project.height() as f32);
    s.view.to_screen(s.workspace.canvas_rect, size, egui::pos2(x, y))
}

fn keys(h: &Harness<'static, ToonApp>) -> Vec<PegKey> {
    h.state().engine.project.peg(PegId(1)).map(|p| p.keys.clone()).unwrap_or_default()
}

/// Arm recording from the keyboard: focus the tool option and press Space.
fn arm(h: &mut Harness<'static, ToonApp>) {
    h.get(By::new().role(egui::accesskit::Role::CheckBox).label("Record Motion")).focus();
    h.run();
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().recorder.armed, "the tool option arms recording");
}

/// Press on the canvas, move there in two steps while the shot plays, optionally press Escape,
/// then let go.
fn take(h: &mut Harness<'static, ToonApp>, from: egui::Pos2, to: egui::Pos2, cancel: bool) {
    h.drag_at(from);
    h.run();
    // a drag starts once the pointer moves
    h.hover_at(from + (to - from) * 0.05);
    h.run();
    assert!(h.state().playback.playing, "a drag with recording armed plays the shot");
    for p in [from + (to - from) * 0.5, to] {
        std::thread::sleep(FRAMES);
        h.hover_at(p);
        h.run();
    }
    assert!(h.state().recorder.recording());
    if cancel {
        h.key_press(Key::Escape);
        h.run();
    }
    h.drop_at(to);
    h.run();
    h.run();
}

#[test]
fn a_drag_while_playing_keys_every_frame_it_passes_in_one_undo_step() {
    let mut h = harness();
    arm(&mut h);
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let (cx, cy) = (w * 0.5 + 120.0, hh * 0.5 + 90.0);
    let depth = h.state().engine.undo_len();
    let (a, b) = (screen(&h, cx, cy), screen(&h, cx + 40.0, cy - 20.0));
    take(&mut h, a, b, false);
    assert!(!h.state().playback.playing, "the take started playback, so letting go stops it");
    assert!(!h.state().recorder.recording());
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step for the take");
    assert_eq!(h.state().engine.undo_label(), Some("Record motion"));
    let k = keys(&h);
    assert!(k.len() >= 3, "frames passed while dragging are keyed: {k:?}");
    let frames: Vec<u32> = k.iter().map(|k| k.frame).collect();
    assert!(frames.windows(2).all(|w| w[1] == w[0] + 1), "no frame is skipped: {frames:?}");
    assert_eq!(frames.first(), Some(&0), "the take starts on the frame it was pressed on");
    let (first, last) = (k[0], k[k.len() - 1]);
    // the first frame holds the first small move (5 % of the way) that started the drag
    assert!(first.x.abs() < 2.6 && first.y.abs() < 1.6, "it starts near where the layer was: {first:?}");
    assert!((last.x - 40.0).abs() < 0.6 && (last.y + 20.0).abs() < 0.6, "it ends under the pointer: {last:?}");
    assert!(k.iter().all(|k| k.x >= -0.6 && k.x <= 40.6), "the keys lie along the move: {k:?}");
    h.state_mut().engine.undo().unwrap();
    assert!(keys(&h).is_empty(), "undo takes the whole take back");
}

#[test]
fn escape_drops_the_take_and_leaves_the_project_as_it_was() {
    let mut h = harness();
    arm(&mut h);
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let before = h.state().engine.project.clone();
    let depth = h.state().engine.undo_len();
    let (a, b) = (screen(&h, w * 0.5 + 100.0, hh * 0.5), screen(&h, w * 0.5 + 160.0, hh * 0.5 + 30.0));
    take(&mut h, a, b, true);
    assert!(!h.state().recorder.recording());
    assert!(!h.state().playback.playing, "dropping the take stops the playback it started");
    assert_eq!(h.state().engine.project, before);
    assert_eq!(h.state().engine.undo_len(), depth);
    assert_eq!(h.state().status, "Take dropped");
}

#[test]
fn unarmed_the_drag_is_an_ordinary_single_key() {
    let mut h = harness();
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let (a, b) = (screen(&h, w * 0.5 + 120.0, hh * 0.5 + 90.0), screen(&h, w * 0.5 + 160.0, hh * 0.5 + 70.0));
    h.drag_at(a);
    h.run();
    assert!(!h.state().playback.playing, "without recording a drag does not play the shot");
    h.hover_at(b);
    h.run();
    h.drop_at(b);
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Peg key"));
    assert_eq!(keys(&h).len(), 1);
}

#[test]
fn the_arrow_keys_record_while_the_shot_plays() {
    let mut h = harness();
    arm(&mut h);
    h.ctx.memory_mut(|m| m.request_focus(crate::canvas::canvas_id()));
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().playback.playing, "Enter plays the shot");
    let depth = h.state().engine.undo_len();
    for _ in 0..3 {
        std::thread::sleep(FRAMES);
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
    }
    assert!(h.state().recorder.recording(), "an arrow key while playing starts a take");
    assert!(h.state().recorder.take_len() >= 3);
    std::thread::sleep(FRAMES);
    h.run();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert!(!h.state().playback.playing);
    assert!(!h.state().recorder.recording(), "stopping playback ends the take");
    assert_eq!(h.state().engine.undo_len(), depth + 1, "one undo step for the take");
    assert_eq!(h.state().engine.undo_label(), Some("Record motion"));
    let k = keys(&h);
    assert!(k.len() >= 3, "{k:?}");
    assert!(k.last().is_some_and(|k| (k.x - 3.0).abs() < 0.01 && k.y.abs() < 0.01), "three one-pixel steps: {k:?}");
    assert!(k.windows(2).all(|w| w[1].x >= w[0].x), "the layer only moved right: {k:?}");
}

#[test]
fn the_registry_toggle_arms_and_disarms_recording() {
    let mut h = harness();
    let cmd = crate::commands::get("timeline.record_motion").expect("registered");
    let checked = cmd.checked.expect("a toggle");
    assert!(!checked(h.state()));
    let ctx = h.ctx.clone();
    (cmd.run)(h.state_mut(), &ctx, cmd.id);
    assert!(checked(h.state()) && h.state().recorder.armed);
    (cmd.run)(h.state_mut(), &ctx, cmd.id);
    assert!(!h.state().recorder.armed);
}

#[test]
fn undo_or_another_tool_mid_take_drops_it_without_a_trace() {
    for (mods, key, what) in [(egui::Modifiers::COMMAND, Key::Z, "undo"), (egui::Modifiers::NONE, Key::P, "the Pencil tool")] {
        let mut h = harness();
        // something to undo
        h.state_mut().run(Command::SetPegKey {
            peg: PegId(1),
            frame: 20,
            x: 5.0,
            y: 0.0,
            angle: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            skew: 0.0,
            ease: tooncraft_doc::Ease::Linear,
        });
        arm(&mut h);
        h.ctx.memory_mut(|m| m.request_focus(crate::canvas::canvas_id()));
        h.run();
        h.key_press(Key::Enter);
        h.run();
        std::thread::sleep(FRAMES);
        h.run();
        h.key_press(Key::ArrowRight);
        h.run();
        assert!(h.state().recorder.recording(), "{what}: recording");
        h.key_press_modifiers(mods, key);
        h.run();
        h.run();
        assert!(!h.state().recorder.recording(), "{what} drops the take");
        let after = keys(&h);
        assert!(after.iter().all(|k| k.frame == 20), "{what}: nothing of the take was keyed: {after:?}");
    }
}
