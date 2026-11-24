//! Poses: named sets of peg keys, a peg and every peg that hangs from it, kept with the shot to put
//! back on any frame (a character's stances, a mouth or hand position).

use serde::{Deserialize, Serialize};

use crate::peg::{MAX_PEGS, Peg, PegId, PegKey};

/// Most poses a shot keeps.
pub const MAX_POSES: usize = 512;
/// Longest pose name (characters).
pub const MAX_POSE_NAME: usize = 200;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub name: String,
    /// Each peg's own key (its frame is ignored when the pose is applied). The first is the peg
    /// the pose was saved from.
    pub keys: Vec<(PegId, PegKey)>,
}

/// `root` and every peg that hangs from it, directly or through others, `root` first and the
/// rest in project order. A chain longer than there can be pegs (a loop) counts as not below.
pub fn below(pegs: &[Peg], root: PegId) -> Vec<PegId> {
    let parent = |id: PegId| pegs.iter().find(|p| p.id == id).and_then(|p| p.parent);
    let mut out = vec![root];
    for p in pegs.iter().take(MAX_PEGS) {
        if p.id == root {
            continue;
        }
        let mut at = p.parent;
        for _ in 0..MAX_PEGS {
            match at {
                Some(id) if id == root => {
                    out.push(p.id);
                    break;
                }
                Some(id) => at = parent(id),
                None => break,
            }
        }
    }
    out
}

impl Pose {
    /// The pose of `root` and the pegs below it at `frame`; `None` when there is no such peg.
    pub fn capture(pegs: &[Peg], root: PegId, frame: u32, name: &str) -> Option<Pose> {
        pegs.iter().find(|p| p.id == root)?;
        let keys = below(pegs, root)
            .into_iter()
            .filter_map(|id| pegs.iter().find(|p| p.id == id))
            .map(|p| {
                let mut k = p.at(frame);
                k.frame = 0;
                (p.id, k)
            })
            .collect();
        Some(Pose { name: name.to_owned(), keys }.sanitized())
    }

    /// Hostile-input repair: a bounded name, at most one key per peg, at most [`MAX_PEGS`] keys,
    /// every value sanitized.
    pub fn sanitized(self) -> Self {
        let mut keys: Vec<(PegId, PegKey)> = Vec::new();
        for (id, k) in self.keys.into_iter().take(MAX_PEGS) {
            if !keys.iter().any(|(i, _)| *i == id) {
                keys.push((id, PegKey { frame: 0, ..k }.sanitized()));
            }
        }
        Pose { name: self.name.trim().chars().take(MAX_POSE_NAME).collect(), keys }
    }

    /// Key every peg of the pose that the shot still has at `frame`. Returns how many were keyed.
    pub fn apply(&self, pegs: &mut [Peg], frame: u32) -> usize {
        let mut n = 0;
        for (id, k) in &self.keys {
            if let Some(p) = pegs.iter_mut().find(|p| p.id == *id) {
                p.set_key(PegKey { frame, ..*k });
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peg(id: u64, parent: Option<u64>, x: f32) -> Peg {
        let mut p = Peg { id: PegId(id), name: format!("P{id}"), parent: parent.map(PegId), pivot: (0.0, 0.0), keys: Vec::new() };
        p.set_key(PegKey { x, ..PegKey::rest(4) });
        p
    }

    #[test]
    fn a_pose_holds_the_peg_and_everything_below_it_and_goes_back_on_any_frame() {
        // 1 ─ 2 ─ 3, and 4 apart
        let mut pegs = vec![peg(1, None, 10.0), peg(2, Some(1), 20.0), peg(3, Some(2), 30.0), peg(4, None, 40.0)];
        assert_eq!(below(&pegs, PegId(2)), [PegId(2), PegId(3)]);
        let pose = Pose::capture(&pegs, PegId(1), 4, "  Stand  ").unwrap();
        assert_eq!(pose.name, "Stand");
        assert_eq!(pose.keys.iter().map(|(i, k)| (i.0, k.x)).collect::<Vec<_>>(), [(1, 10.0), (2, 20.0), (3, 30.0)]);
        assert!(Pose::capture(&pegs, PegId(9), 4, "x").is_none());
        pegs.remove(1);
        assert_eq!(pose.apply(&mut pegs, 9), 2, "a peg the shot lost is skipped");
        assert_eq!(pegs[0].keys.iter().map(|k| (k.frame, k.x)).collect::<Vec<_>>(), [(4, 10.0), (9, 10.0)]);
        assert_eq!(pegs[2].keys.len(), 1, "a peg outside the pose is not keyed");
    }

    #[test]
    fn a_loop_in_the_parents_ends_and_hostile_poses_are_repaired() {
        let pegs = vec![peg(1, Some(2), 0.0), peg(2, Some(1), 0.0), peg(3, Some(3), 0.0)];
        assert_eq!(below(&pegs, PegId(3)), [PegId(3)]);
        assert_eq!(below(&pegs, PegId(1)), [PegId(1), PegId(2)], "the loop ends");
        let k = PegKey { x: f32::NAN, scale_x: f32::INFINITY, ..PegKey::rest(7) };
        let p = Pose { name: "n".repeat(1000), keys: vec![(PegId(1), k); MAX_PEGS * 4] }.sanitized();
        assert_eq!(p.name.chars().count(), MAX_POSE_NAME);
        assert_eq!(p.keys.len(), 1, "one key per peg");
        assert_eq!((p.keys[0].1.frame, p.keys[0].1.x), (0, 0.0));
        assert!(p.keys[0].1.scale_x.is_finite());
    }
}
