//! Inverse kinematics on peg chains: move the end of a chain (a hand) and the pegs it hangs from
//! (forearm, upper arm) turn about their pivots to follow, instead of the end sliding on its own.
//!
//! The solver turns one joint at a time towards the target, nearest joint first, a few passes
//! over the chain (cyclic coordinate descent). Only the turned pegs' angles change; their moves,
//! scales and skews stay. Work is bounded: at most [`MAX_IK_JOINTS`] joints and [`PASSES`] passes.

use crate::peg::{MAX_PEG_DEPTH, Peg, PegId, PegKey, chain, pivot_on_canvas};

/// The most pegs above the end that IK turns.
pub const MAX_IK_JOINTS: usize = 8;
/// Passes over the chain.
pub const PASSES: usize = 16;
/// Close enough (canvas pixels).
const NEAR: f32 = 0.25;

/// The pegs `end` hangs from, nearest first, at most `joints` of them (a loop in the parents
/// ends the walk).
pub fn joints_above(pegs: &[Peg], end: PegId, joints: usize) -> Vec<PegId> {
    let parent = |id: PegId| pegs.iter().find(|p| p.id == id).and_then(|p| p.parent);
    let mut out: Vec<PegId> = Vec::new();
    let mut at = parent(end);
    while let Some(id) = at {
        if out.len() >= joints.min(MAX_IK_JOINTS) || out.contains(&id) || id == end || out.len() >= MAX_PEG_DEPTH {
            break;
        }
        out.push(id);
        at = parent(id);
    }
    out
}

/// The keys at `frame` that turn up to `joints` pegs above `end` so `end`'s pivot comes as close
/// as it can to `target` (canvas pixels). Empty when `end` hangs from no peg or the target is not
/// a finite point.
pub fn solve(pegs: &[Peg], end: PegId, frame: u32, target: (f32, f32), joints: usize) -> Vec<(PegId, PegKey)> {
    if !(target.0.is_finite() && target.1.is_finite()) {
        return Vec::new();
    }
    let above = joints_above(pegs, end, joints);
    if above.is_empty() {
        return Vec::new();
    }
    let mut work: Vec<Peg> = pegs.to_vec();
    let key_of = |work: &[Peg], id: PegId| work.iter().find(|p| p.id == id).map(|p| PegKey { frame, ..p.at(frame) });
    for _ in 0..PASSES {
        for id in &above {
            let (Some(e), Some(j)) = (pivot_on_canvas(&work, end, frame), pivot_on_canvas(&work, *id, frame)) else { continue };
            let (a, b) = ((e.0 - j.0, e.1 - j.1), (target.0 - j.0, target.1 - j.1));
            if a.0.hypot(a.1) < 1e-3 || b.0.hypot(b.1) < 1e-3 {
                continue;
            }
            let mut turn = (b.1.atan2(b.0) - a.1.atan2(a.0)).to_degrees();
            if turn > 180.0 {
                turn -= 360.0;
            } else if turn < -180.0 {
                turn += 360.0;
            }
            // a mirrored parent turns its children the other way
            let parent = work.iter().find(|p| p.id == *id).and_then(|p| p.parent).map(|p| chain(&work, p, frame));
            if parent.is_some_and(|m| m.a * m.d - m.b * m.c < 0.0) {
                turn = -turn;
            }
            let Some(mut k) = key_of(&work, *id) else { continue };
            k.angle += turn;
            if let Some(p) = work.iter_mut().find(|p| p.id == *id) {
                p.set_key(k);
            }
        }
        if pivot_on_canvas(&work, end, frame).is_some_and(|e| (e.0 - target.0).hypot(e.1 - target.1) < NEAR) {
            break;
        }
    }
    above.iter().filter_map(|id| key_of(&work, *id).map(|k| (*id, k))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arm: shoulder pivot at (0, 0), elbow at (100, 0), hand at (200, 0).
    fn arm() -> Vec<Peg> {
        let peg =
            |id: u64, parent: Option<u64>, x: f32| Peg { id: PegId(id), name: format!("P{id}"), parent: parent.map(PegId), pivot: (x, 0.0), keys: Vec::new() };
        vec![peg(1, None, 0.0), peg(2, Some(1), 100.0), peg(3, Some(2), 200.0)]
    }

    fn apply(pegs: &mut [Peg], keys: &[(PegId, PegKey)]) {
        for (id, k) in keys {
            if let Some(p) = pegs.iter_mut().find(|p| p.id == *id) {
                p.set_key(*k);
            }
        }
    }

    #[test]
    fn the_hand_reaches_a_point_in_reach_by_turning_the_arm_only() {
        let mut pegs = arm();
        let target = (120.0, 120.0);
        let keys = solve(&pegs, PegId(3), 4, target, 2);
        assert_eq!(keys.iter().map(|(id, _)| id.0).collect::<Vec<_>>(), [2, 1], "elbow, then shoulder");
        assert!(keys.iter().all(|(_, k)| k.frame == 4 && k.x == 0.0 && k.y == 0.0 && k.scale_x == 1.0), "only angles change");
        apply(&mut pegs, &keys);
        let hand = pivot_on_canvas(&pegs, PegId(3), 4).unwrap();
        assert!((hand.0 - target.0).hypot(hand.1 - target.1) < 1.0, "the hand is there: {hand:?}");
        let shoulder = pivot_on_canvas(&pegs, PegId(1), 4).unwrap();
        assert_eq!(shoulder, (0.0, 0.0), "the shoulder stays put");
        let elbow = pivot_on_canvas(&pegs, PegId(2), 4).unwrap();
        assert!((elbow.0.hypot(elbow.1) - 100.0).abs() < 0.01, "the upper arm keeps its length");
    }

    #[test]
    fn out_of_reach_the_arm_points_at_the_target_and_one_joint_turns_only_the_elbow() {
        let mut pegs = arm();
        let keys = solve(&pegs, PegId(3), 0, (0.0, 500.0), 2);
        apply(&mut pegs, &keys);
        let hand = pivot_on_canvas(&pegs, PegId(3), 0).unwrap();
        assert!(hand.0.abs() < 1.0 && (hand.1 - 200.0).abs() < 1.0, "stretched straight towards it: {hand:?}");
        let keys = solve(&arm(), PegId(3), 0, (100.0, 100.0), 1);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].0, PegId(2));
        assert!((keys[0].1.angle - 90.0).abs() < 0.5, "a quarter turn of the elbow: {}", keys[0].1.angle);
    }

    #[test]
    fn nothing_to_turn_or_a_hostile_target_or_a_loop_gives_nothing_or_stops() {
        assert!(solve(&arm(), PegId(1), 0, (5.0, 5.0), 3).is_empty(), "the shoulder hangs from nothing");
        assert!(solve(&arm(), PegId(3), 0, (f32::NAN, 5.0), 3).is_empty());
        assert!(solve(&arm(), PegId(9), 0, (5.0, 5.0), 3).is_empty());
        let mut looped = arm();
        looped[0].parent = Some(PegId(3));
        assert_eq!(joints_above(&looped, PegId(3), 8), [PegId(2), PegId(1)], "the loop back to the end stops the walk");
        let keys = solve(&looped, PegId(3), 0, (50.0, 50.0), 8);
        assert!(keys.iter().all(|(_, k)| k.angle.is_finite()));
    }
}
