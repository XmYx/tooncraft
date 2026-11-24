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
