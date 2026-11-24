//! Curve editor commands: moving, easing and removing several peg keys as one undo step.

use super::*;
use tooncraft_doc::{Channel, Ease, PegId};

fn engine_with_keys() -> (Engine, PegId) {
    let mut e = Engine::new(Project::new("C", 64, 64, 24, 40).unwrap());
    e.apply(Command::AddPeg { name: "Arm".into(), parent: None }).unwrap();
    let peg = PegId(1);
    for (frame, x) in [(0, 0.0), (10, 50.0), (20, 20.0)] {
        e.apply(Command::SetPegKey { peg, frame, x, y: 0.0, angle: 0.0, scale_x: 1.0, scale_y: 1.0, skew: 0.0, ease: Ease::Linear }).unwrap();
    }
    (e, peg)
}

fn frames(e: &Engine, peg: PegId) -> Vec<(u32, f32)> {
    e.project.peg(peg).map(|p| p.keys.iter().map(|k| (k.frame, k.x)).collect()).unwrap_or_default()
}

#[test]
fn moving_keys_is_one_undo_step_and_refusals_change_nothing() {
    let (mut e, peg) = engine_with_keys();
    let depth = e.undo_len();
    e.apply(Command::MovePegKeys { peg, frames: vec![10, 20], by_frames: 3, channel: Some(Channel::X), by_value: -5.0 }).unwrap();
    assert_eq!(frames(&e, peg), [(0, 0.0), (13, 45.0), (23, 15.0)]);
    assert_eq!(e.undo_len(), depth + 1);
    assert_eq!(e.undo_label(), Some("Move keys"));
    e.apply(Command::MovePegKeys { peg, frames: vec![13], by_frames: 0, channel: Some(Channel::X), by_value: 1.0 }).unwrap();
    assert_eq!(e.undo_label(), Some("Change key values"));
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(frames(&e, peg), [(0, 0.0), (10, 50.0), (20, 20.0)]);
    let before = e.project.clone();
    let depth = e.undo_len();
    for bad in [
        Command::MovePegKeys { peg, frames: vec![10], by_frames: -10, channel: None, by_value: 0.0 },
        Command::MovePegKeys { peg, frames: vec![20], by_frames: 20, channel: None, by_value: 0.0 },
        Command::MovePegKeys { peg, frames: vec![5], by_frames: 1, channel: None, by_value: 0.0 },
        Command::MovePegKeys { peg: PegId(9), frames: vec![0], by_frames: 1, channel: None, by_value: 0.0 },
    ] {
        assert!(e.apply(bad).is_err());
    }
    assert_eq!(e.project, before, "refused moves leave the project as it was");
    assert_eq!(e.undo_len(), depth, "and add no undo step");
}

#[test]
fn eases_are_set_on_every_chosen_key_at_once() {
    let (mut e, peg) = engine_with_keys();
    let depth = e.undo_len();
    e.apply(Command::SetPegEases { peg, frames: vec![0, 10], ease: Ease::Curve { x1: 7.0, y1: f32::NAN, x2: 0.4, y2: -9.0 } }).unwrap();
    assert_eq!(e.undo_len(), depth + 1);
    let eases: Vec<Ease> = e.project.peg(peg).map(|p| p.keys.iter().map(|k| k.ease).collect()).unwrap_or_default();
    let clamped = Ease::Curve { x1: 1.0, y1: 0.5, x2: 0.4, y2: -2.0 };
    assert_eq!(eases, [clamped, clamped, Ease::Linear]);
    e.apply(Command::RemovePegKeys { peg, frames: vec![0, 20, 33] }).unwrap();
    assert_eq!(frames(&e, peg), [(10, 50.0)]);
    assert_eq!(e.undo_len(), depth + 2);
    e.undo().unwrap();
    assert_eq!(frames(&e, peg).len(), 3);
}

#[test]
fn curve_commands_read_from_json_lines() {
    let line = r#"{"cmd":"move_peg_keys","peg":1,"frames":[10],"by_frames":2,"channel":"scale_x","by_value":0.5}"#;
    let cmd: Command = serde_json::from_str(line).unwrap();
    assert_eq!(cmd, Command::MovePegKeys { peg: PegId(1), frames: vec![10], by_frames: 2, channel: Some(Channel::ScaleX), by_value: 0.5 });
    let line = r#"{"cmd":"set_peg_eases","peg":1,"frames":[0,10],"ease":{"kind":"hold"}}"#;
    let cmd: Command = serde_json::from_str(line).unwrap();
    assert_eq!(cmd, Command::SetPegEases { peg: PegId(1), frames: vec![0, 10], ease: Ease::Hold });
}

#[test]
fn a_recorded_take_sets_its_keys_as_one_undo_step_and_refusals_change_nothing() {
    let (mut e, peg) = engine_with_keys();
    let depth = e.undo_len();
    let key = |frame, x| tooncraft_doc::PegKey { x, ..tooncraft_doc::PegKey::rest(frame) };
    e.apply(Command::SetPegKeys { peg, keys: vec![key(10, 7.0), key(11, 8.0), key(12, f32::NAN)] }).unwrap();
    assert_eq!(frames(&e, peg), [(0, 0.0), (10, 7.0), (11, 8.0), (12, 0.0), (20, 20.0)], "a hostile value is sanitized");
    assert_eq!(e.undo_len(), depth + 1);
    assert_eq!(e.undo_label(), Some("Record motion"));
    e.undo().unwrap();
    assert_eq!(frames(&e, peg), [(0, 0.0), (10, 50.0), (20, 20.0)]);
    let before = e.project.clone();
    let depth = e.undo_len();
    let many = (0..=tooncraft_doc::peg::MAX_PEG_KEYS as u32).map(|f| key(f % 40, 1.0)).collect();
    for bad in [
        Command::SetPegKeys { peg, keys: vec![key(5, 1.0), key(40, 1.0)] },
        Command::SetPegKeys { peg: PegId(9), keys: vec![key(5, 1.0)] },
        Command::SetPegKeys { peg, keys: many },
    ] {
        assert!(e.apply(bad).is_err());
    }
    e.apply(Command::SetPegKeys { peg, keys: vec![] }).unwrap();
    assert_eq!(e.project, before, "refused or empty takes change nothing");
    assert_eq!(e.undo_len(), depth, "and push no undo step");
}

#[test]
fn cameras_and_cuts_are_undo_steps_and_removing_a_camera_renumbers_its_cuts() {
    let mut e = Engine::new(Project::new("Cams", 100, 50, 24, 40).unwrap());
    let key = |frame, x, camera| Command::SetCameraKey { frame, x, y: 25.0, zoom: 1.0, angle: 0.0, camera };
    e.apply(Command::AddCamera { name: " Close ".into(), from: None }).unwrap();
    e.apply(Command::AddCamera { name: "Wide".into(), from: Some(1) }).unwrap();
    assert_eq!(e.undo_label(), Some("Add camera"));
    assert_eq!(e.project.cameras.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Close", "Wide"]);
    e.apply(key(0, 30.0, 1)).unwrap();
    e.apply(key(0, 70.0, 2)).unwrap();
    assert!(e.project.camera.keys.is_empty(), "the main camera is untouched");
    e.apply(Command::SetCameraCut { frame: 10, camera: 1 }).unwrap();
    e.apply(Command::SetCameraCut { frame: 20, camera: 2 }).unwrap();
    e.apply(Command::SetCameraCut { frame: 30, camera: 0 }).unwrap();
    assert_eq!([5, 10, 25, 35].map(|f| e.project.camera_index_at(f)), [0, 1, 2, 0]);
    let depth = e.undo_len();
    e.apply(Command::RemoveCamera { camera: 1 }).unwrap();
    assert_eq!(e.undo_len(), depth + 1);
    assert_eq!(e.project.cameras.len(), 1);
    assert_eq!(e.project.camera_cuts.iter().map(|c| (c.frame, c.camera)).collect::<Vec<_>>(), [(20, 1), (30, 0)], "its cut went, Wide moved down");
    assert_eq!(e.project.camera_at(25).keys[0].x, 70.0);
    e.undo().unwrap();
    assert_eq!(e.project.camera_index_at(12), 1, "undo brings the camera and its cut back");
    let before = e.project.clone();
    let depth = e.undo_len();
    for bad in [
        key(0, 1.0, 9),
        key(40, 1.0, 1),
        Command::RemoveCamera { camera: 0 },
        Command::RemoveCamera { camera: 5 },
        Command::RenameCamera { camera: 0, name: "x".into() },
        Command::SetCameraCut { frame: 3, camera: 3 },
        Command::SetCameraCut { frame: 99, camera: 0 },
        Command::AddCamera { name: "x".into(), from: Some(8) },
        Command::RemoveCameraShake { index: 0, camera: 1 },
        Command::SetCamera { enabled: false, camera: 4 },
    ] {
        assert!(e.apply(bad).is_err());
    }
    e.apply(Command::RemoveCameraCut { frame: 4 }).unwrap();
    e.apply(Command::RemoveCameraKey { frame: 7, camera: 2 }).unwrap();
    assert_eq!(e.project, before, "refusals and no-ops change nothing");
    assert_eq!(e.undo_len(), depth, "and push no undo step");
    for _ in 0..tooncraft_doc::camera::MAX_CAMERAS - 2 {
        e.apply(Command::AddCamera { name: "More".into(), from: None }).unwrap();
    }
    assert!(e.apply(Command::AddCamera { name: "One too many".into(), from: None }).is_err());
}
