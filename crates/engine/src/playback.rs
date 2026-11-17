//! Real-time playback timing.
//!
//! Playback is driven by the wall clock, not by counting UI frames: the frame shown is the frame
//! the clock says, so a slow redraw drops frames instead of slowing the animation down.

/// The playback range and mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub playing: bool,
    pub looping: bool,
    /// First frame of the range.
    pub start: u32,
    /// One past the last frame of the range.
    pub end: u32,
    /// Clock time (seconds) and frame when play started.
    anchor_time: f64,
    anchor_frame: u32,
}

impl Default for Playback {
    fn default() -> Self {
        Self { playing: false, looping: true, start: 0, end: 1, anchor_time: 0.0, anchor_frame: 0 }
    }
}

impl Playback {
    /// Start playing from `frame` at clock time `now`.
    pub fn play(&mut self, now: f64, frame: u32, start: u32, end: u32) {
        self.start = start;
        self.end = end.max(start + 1);
        self.anchor_frame = if frame < self.start || frame >= self.end { self.start } else { frame };
        self.anchor_time = now;
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// The frame to show at clock time `now`. Stops at the end of the range unless looping.
    pub fn frame(&mut self, now: f64, fps: u32) -> u32 {
        let (f, still) = frame_at(now - self.anchor_time, fps, self.start, self.end, self.anchor_frame, self.looping);
        if !still {
            self.playing = false;
        }
        f
    }

    /// Seconds until the next frame boundary (for scheduling the next repaint).
    pub fn until_next(&self, now: f64, fps: u32) -> f64 {
        let fps = fps.max(1) as f64;
        let t = (now - self.anchor_time).max(0.0) * fps;
        ((t.floor() + 1.0 - t) / fps).max(0.001)
    }
}

/// The frame shown `elapsed` seconds after starting at `from` within `start..end`.
/// Returns the frame and whether playback continues.
pub fn frame_at(elapsed: f64, fps: u32, start: u32, end: u32, from: u32, looping: bool) -> (u32, bool) {
    let len = end.saturating_sub(start).max(1) as u64;
    let elapsed = if elapsed.is_finite() { elapsed.max(0.0) } else { 0.0 };
    let advanced = (elapsed * fps.max(1) as f64).floor().min(1e15) as u64;
    let offset = from.saturating_sub(start) as u64 + advanced;
    if looping {
        (start + (offset % len) as u32, true)
    } else if offset >= len {
        (end.saturating_sub(1).max(start), false)
    } else {
        (start + offset as u32, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing() {
        assert_eq!(frame_at(0.0, 24, 0, 48, 0, true), (0, true));
        assert_eq!(frame_at(1.0, 24, 0, 48, 0, true), (24, true));
        assert_eq!(frame_at(2.5, 24, 0, 48, 0, true), (12, true));
        assert_eq!(frame_at(2.5, 24, 0, 48, 0, false), (47, false));
        assert_eq!(frame_at(0.5, 12, 10, 20, 15, true), (11, true));
        assert_eq!(frame_at(f64::NAN, 12, 0, 5, 3, true), (3, true));
    }

    #[test]
    fn player_stops_at_the_end() {
        let mut p = Playback { looping: false, ..Default::default() };
        p.play(100.0, 0, 0, 10);
        assert_eq!(p.frame(100.2, 10), 2);
        assert!(p.playing);
        assert_eq!(p.frame(102.0, 10), 9);
        assert!(!p.playing);
        assert!(p.until_next(100.05, 10) > 0.0);
    }
}
