//! Touch gestures and palm rejection (UI-free).
//!
//! The canvas feeds every touch event to a [`Touch`] recogniser. A quick tap with two fingers is
//! undo, with three fingers redo (a tap: all the fingers go down within [`TAP_WINDOW`], none moves
//! more than [`TAP_SLOP`] and all come up within [`TAP_WINDOW`] of the first touch). While the pen
//! is near, or was a moment ago ([`PALM_HOLD`]), touches are ignored: a resting palm cannot draw,
//! zoom or tap.

use std::collections::BTreeMap;

/// Longest a tap may last, in seconds.
pub const TAP_WINDOW: f64 = 0.35;
/// How far a finger may drift in a tap, in screen points.
pub const TAP_SLOP: f32 = 14.0;
/// Touches are ignored this long (seconds) after the pen was last seen.
pub const PALM_HOLD: f64 = 1.2;
/// Most fingers tracked at once.
pub const MAX_FINGERS: usize = 10;

/// A phase of one finger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Move,
    End,
    Cancel,
}

/// What a recognised gesture asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Undo,
    Redo,
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    start: (f32, f32),
    started: f64,
    drifted: bool,
    up: bool,
}

/// What the canvas should do with one touch event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Verdict {
    /// The event is a palm (or the pen is near): ignore it entirely.
    pub rejected: bool,
    /// A second finger arrived: whatever one finger started drawing is a gesture instead.
    pub gesture_begun: bool,
    /// A completed tap gesture.
    pub gesture: Option<Gesture>,
}

#[derive(Default)]
pub struct Touch {
    fingers: BTreeMap<u64, Finger>,
    /// Fingers rejected as palms, until they lift.
    palms: BTreeMap<u64, (f32, f32)>,
    pen_seen: Option<f64>,
    /// When the current group of fingers began, and the most fingers it reached.
    group_start: Option<f64>,
    peak: usize,
    /// The group moved or ran too long: it is not a tap.
    spoiled: bool,
}

impl Touch {
    /// The pen is in proximity at `now` (seconds): touches are ignored until [`PALM_HOLD`] after
    /// it was last seen.
    pub fn pen_seen(&mut self, now: f64) {
        if now.is_finite() {
            self.pen_seen = Some(now);
        }
    }

    /// Whether the pen is near enough at `now` to ignore touches.
    pub fn pen_near(&self, now: f64) -> bool {
        self.pen_seen.is_some_and(|t| now - t < PALM_HOLD && now >= t - 1e-9)
    }

    /// Whether a finger rejected as a palm is down within `radius` of `pos`. The window system
    /// also reports a touch as pointer events at the same place; the canvas uses this to ignore
    /// those too.
    pub fn palm_at(&self, pos: (f32, f32), radius: f32) -> bool {
        self.palms.values().any(|p| (p.0 - pos.0).hypot(p.1 - pos.1) <= radius)
    }

    /// Fingers currently down (palms not counted).
    pub fn fingers(&self) -> usize {
        self.fingers.values().filter(|f| !f.up).count()
    }

    /// Feed one touch event: finger `id` at `pos` (screen points) at time `now` (seconds).
    pub fn event(&mut self, now: f64, id: u64, phase: Phase, pos: (f32, f32)) -> Verdict {
        let none = Verdict { rejected: false, gesture_begun: false, gesture: None };
        if !now.is_finite() || !pos.0.is_finite() || !pos.1.is_finite() {
            return Verdict { rejected: true, ..none };
        }
        if let Some(p) = self.palms.get_mut(&id) {
            if matches!(phase, Phase::End | Phase::Cancel) {
                self.palms.remove(&id);
            } else {
                *p = pos;
            }
            return Verdict { rejected: true, ..none };
        }
        match phase {
            Phase::Start => {
                if self.pen_near(now) || self.fingers.len() >= MAX_FINGERS {
                    if self.palms.len() < MAX_FINGERS {
                        self.palms.insert(id, pos);
                    }
                    return Verdict { rejected: true, ..none };
                }
                if self.fingers.values().all(|f| f.up) {
                    // a new group of fingers
                    self.fingers.clear();
                    self.group_start = Some(now);
                    self.peak = 0;
                    self.spoiled = false;
                }
                if self.group_start.is_some_and(|t0| now - t0 > TAP_WINDOW) {
                    self.spoiled = true; // the last finger came too late for a tap
                }
                self.fingers.insert(id, Finger { start: pos, started: now, drifted: false, up: false });
                let down = self.fingers();
                self.peak = self.peak.max(down);
                Verdict { gesture_begun: down >= 2, ..none }
            }
            Phase::Move => {
                if let Some(f) = self.fingers.get_mut(&id)
                    && (pos.0 - f.start.0).hypot(pos.1 - f.start.1) > TAP_SLOP
                {
                    f.drifted = true;
                    self.spoiled = true;
                }
                none
            }
            Phase::End | Phase::Cancel => {
                let Some(f) = self.fingers.get_mut(&id) else { return none };
                if f.up {
                    return none;
                }
                f.up = true;
                if phase == Phase::Cancel || f.drifted || now - f.started > TAP_WINDOW || self.group_start.is_some_and(|t0| now - t0 > TAP_WINDOW) {
                    self.spoiled = true;
                }
                if self.fingers() > 0 {
                    return none;
                }
                // the last finger is up: was the group a tap?
                let tap = !self.spoiled;
                let peak = self.peak;
                self.group_start = None;
                self.peak = 0;
                self.spoiled = false;
                let gesture = match (tap, peak) {
                    (true, 2) => Some(Gesture::Undo),
                    (true, 3) => Some(Gesture::Redo),
                    _ => None,
                };
                Verdict { gesture, ..none }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(t: &mut Touch, t0: f64, n: u64) -> Option<Gesture> {
        for i in 0..n {
            t.event(t0 + i as f64 * 0.02, i, Phase::Start, (100.0 + i as f32 * 30.0, 100.0));
        }
        let mut out = None;
        for i in 0..n {
            let v = t.event(t0 + 0.12 + i as f64 * 0.01, i, Phase::End, (100.0 + i as f32 * 30.0, 100.0));
            out = out.or(v.gesture);
        }
        out
    }

    #[test]
    fn two_fingers_undo_and_three_redo() {
        let mut t = Touch::default();
        assert_eq!(tap(&mut t, 1.0, 2), Some(Gesture::Undo));
        assert_eq!(tap(&mut t, 2.0, 3), Some(Gesture::Redo));
        assert_eq!(tap(&mut t, 3.0, 1), None, "one finger is a stroke, not a gesture");
        assert_eq!(tap(&mut t, 4.0, 4), None);
        assert_eq!(t.fingers(), 0);
    }

    #[test]
    fn a_second_finger_turns_a_stroke_into_a_gesture() {
        let mut t = Touch::default();
        assert!(!t.event(0.0, 1, Phase::Start, (10.0, 10.0)).gesture_begun);
        assert!(t.event(0.05, 2, Phase::Start, (60.0, 10.0)).gesture_begun);
    }

    #[test]
    fn drags_pinches_and_slow_touches_are_not_taps() {
        let mut t = Touch::default();
        // a pinch: the fingers move apart
        t.event(0.0, 1, Phase::Start, (100.0, 100.0));
        t.event(0.01, 2, Phase::Start, (140.0, 100.0));
        t.event(0.1, 2, Phase::Move, (200.0, 100.0));
        t.event(0.2, 1, Phase::End, (100.0, 100.0));
        assert_eq!(t.event(0.21, 2, Phase::End, (200.0, 100.0)).gesture, None);
        // a long press with two fingers
        t.event(5.0, 1, Phase::Start, (0.0, 0.0));
        t.event(5.0, 2, Phase::Start, (40.0, 0.0));
        t.event(5.2, 1, Phase::End, (0.0, 0.0));
        assert_eq!(t.event(5.6, 2, Phase::End, (40.0, 0.0)).gesture, None);
        // the second finger arrives late
        t.event(8.0, 1, Phase::Start, (0.0, 0.0));
        t.event(8.5, 2, Phase::Start, (40.0, 0.0));
        t.event(8.55, 1, Phase::End, (0.0, 0.0));
        assert_eq!(t.event(8.56, 2, Phase::End, (40.0, 0.0)).gesture, None);
        // a cancelled touch
        t.event(10.0, 1, Phase::Start, (0.0, 0.0));
        t.event(10.0, 2, Phase::Start, (40.0, 0.0));
        t.event(10.05, 1, Phase::Cancel, (0.0, 0.0));
        assert_eq!(t.event(10.06, 2, Phase::End, (40.0, 0.0)).gesture, None);
        // small jitter is fine
        t.event(12.0, 1, Phase::Start, (0.0, 0.0));
        t.event(12.0, 2, Phase::Start, (40.0, 0.0));
        t.event(12.05, 1, Phase::Move, (3.0, 2.0));
        t.event(12.1, 1, Phase::End, (3.0, 2.0));
        assert_eq!(t.event(12.11, 2, Phase::End, (40.0, 0.0)).gesture, Some(Gesture::Undo));
    }

    #[test]
    fn a_palm_is_ignored_while_the_pen_is_near_and_a_moment_after() {
        let mut t = Touch::default();
        t.pen_seen(10.0);
        assert!(t.pen_near(10.5));
        assert!(!t.pen_near(11.3), "after the hold time touches count again");
        let v = t.event(10.5, 7, Phase::Start, (5.0, 5.0));
        assert!(v.rejected);
        // the palm stays rejected until it lifts, even after the hold time, and cannot make a tap
        assert!(t.event(12.0, 7, Phase::Move, (5.0, 6.0)).rejected);
        assert_eq!(t.fingers(), 0);
        assert!(t.event(12.1, 7, Phase::End, (5.0, 6.0)).rejected);
        let mut t2 = Touch::default();
        t2.pen_seen(1.0);
        assert!(t2.event(1.1, 3, Phase::Start, (50.0, 60.0)).rejected);
        assert!(t2.palm_at((51.0, 60.0), 2.0), "pointer events at the palm's place are the palm");
        assert!(!t2.palm_at((90.0, 60.0), 2.0));
        t2.event(1.2, 3, Phase::End, (50.0, 60.0));
        assert!(!t2.palm_at((50.0, 60.0), 2.0), "a lifted palm stops counting");
        // a later finger is accepted again
        assert!(!t.event(12.2, 8, Phase::Start, (5.0, 5.0)).rejected);
        // two real fingers tapping while the pen is hovering do nothing
        let mut t = Touch::default();
        t.pen_seen(20.0);
        assert_eq!(tap(&mut t, 20.1, 2), None);
        assert_eq!(tap(&mut t, 22.0, 2), Some(Gesture::Undo));
    }

    #[test]
    fn hostile_input_does_not_panic_or_grow_without_bound() {
        let mut t = Touch::default();
        for id in 0..1000u64 {
            t.event(id as f64 * 0.001, id, Phase::Start, (id as f32, 0.0));
        }
        assert!(t.fingers() <= MAX_FINGERS);
        assert!(t.palms.len() <= MAX_FINGERS);
        assert!(t.event(f64::NAN, 1, Phase::Start, (0.0, 0.0)).rejected);
        assert!(t.event(1.0, 1, Phase::Move, (f32::INFINITY, 0.0)).rejected);
        t.pen_seen(f64::NAN);
        assert!(!t.pen_near(0.0));
        assert_eq!(t.event(0.0, 99999, Phase::End, (0.0, 0.0)).gesture, None, "an unknown finger lifting is nothing");
    }
}
