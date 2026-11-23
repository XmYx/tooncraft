//! Real-time playback timing.
//!
//! Playback is driven by the wall clock, not by counting UI frames: the frame shown is the frame
//! the clock says, so a slow redraw drops frames instead of slowing the animation down.

/// The playback range and mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Playback {
    pub playing: bool,
    pub looping: bool,
    /// Play forward to the end of the range and back again, instead of jumping back to the start.
    pub ping_pong: bool,
    /// First frame of the range.
    pub start: u32,
    /// One past the last frame of the range.
    pub end: u32,
    /// Clock time (seconds) and frame when play started.
    anchor_time: f64,
    anchor_frame: u32,
    /// Speed as a fraction of the frame rate ([`MIN_SPEED`] … [`MAX_SPEED`]; 1 is real time).
    speed: f64,
}

pub const MIN_SPEED: f64 = 0.1;
pub const MAX_SPEED: f64 = 4.0;

impl Default for Playback {
    fn default() -> Self {
        Self { playing: false, looping: true, ping_pong: false, start: 0, end: 1, anchor_time: 0.0, anchor_frame: 0, speed: 1.0 }
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

    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Change the speed; while playing, the frame on screen stays where it is.
    pub fn set_speed(&mut self, now: f64, fps: u32, speed: f64) {
        let speed = if speed.is_finite() { speed.clamp(MIN_SPEED, MAX_SPEED) } else { 1.0 };
        if self.playing {
            let f = self.frame(now, fps);
            self.anchor_frame = f;
            self.anchor_time = now;
        }
        self.speed = speed;
    }

    /// The frame to show at clock time `now`. Stops at the end of the range unless looping.
    pub fn frame(&mut self, now: f64, fps: u32) -> u32 {
        let (f, still) = frame_at_mode((now - self.anchor_time) * self.speed, fps, self.start, self.end, self.anchor_frame, self.looping, self.ping_pong);
        if !still {
            self.playing = false;
        }
        f
    }

    /// Seconds until the next frame boundary (for scheduling the next repaint).
    pub fn until_next(&self, now: f64, fps: u32) -> f64 {
        let fps = fps.max(1) as f64;
        let t = (now - self.anchor_time).max(0.0) * self.speed * fps;
        ((t.floor() + 1.0 - t) / (fps * self.speed)).max(0.001)
    }
}

/// The frame shown `elapsed` seconds after starting at `from` within `start..end`.
/// Returns the frame and whether playback continues.
pub fn frame_at(elapsed: f64, fps: u32, start: u32, end: u32, from: u32, looping: bool) -> (u32, bool) {
    frame_at_mode(elapsed, fps, start, end, from, looping, false)
}

/// Like [`frame_at`], with ping-pong: forward to the last frame, back to the first (neither end
/// frame is shown twice in a row), and on. Without `looping` it plays there and back once and
/// stops on the first frame.
pub fn frame_at_mode(elapsed: f64, fps: u32, start: u32, end: u32, from: u32, looping: bool, ping_pong: bool) -> (u32, bool) {
    let len = end.saturating_sub(start).max(1) as u64;
    if !ping_pong || len < 2 {
        return frame_at_plain(elapsed, fps, start, end, from, looping);
    }
    let elapsed = if elapsed.is_finite() { elapsed.max(0.0) } else { 0.0 };
    let advanced = (elapsed * fps.max(1) as f64).floor().min(1e15) as u64;
    let offset = from.saturating_sub(start) as u64 + advanced;
    let period = 2 * (len - 1);
    if !looping && offset >= period {
        return (start, false);
    }
    let m = offset % period;
    let step = if m < len { m } else { period - m };
    (start + step as u32, true)
}

fn frame_at_plain(elapsed: f64, fps: u32, start: u32, end: u32, from: u32, looping: bool) -> (u32, bool) {
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
    fn ping_pong_goes_there_and_back() {
        // a 5 frame range at 1 fps: 0 1 2 3 4 3 2 1 | 0 1 ...
        let at = |t: f64, looping: bool| frame_at_mode(t, 1, 0, 5, 0, looping, true);
        let seen: Vec<u32> = (0..10).map(|t| at(t as f64, true).0).collect();
        assert_eq!(seen, vec![0, 1, 2, 3, 4, 3, 2, 1, 0, 1]);
        assert!((0..40).all(|t| at(t as f64, true).1), "a looping ping-pong never stops");
        // not looping: there and back once, then it stops on the first frame
        assert_eq!(at(7.0, false), (1, true));
        assert_eq!(at(8.0, false), (0, false));
        assert_eq!(at(80.0, false), (0, false));
        // starting in the middle goes forward first
        assert_eq!(frame_at_mode(0.0, 1, 0, 5, 3, true, true), (3, true));
        assert_eq!(frame_at_mode(1.0, 1, 0, 5, 3, true, true), (4, true));
        assert_eq!(frame_at_mode(2.0, 1, 0, 5, 3, true, true), (3, true));
        // a one-frame range and hostile numbers do not divide by zero or panic
        assert_eq!(frame_at_mode(5.0, 24, 7, 8, 7, true, true), (7, true));
        assert_eq!(frame_at_mode(f64::NAN, 24, 0, 5, 2, true, true), (2, true));
        assert!(frame_at_mode(f64::INFINITY, 24, 0, 5, 2, true, true).1);
        assert_eq!(frame_at_mode(1.0, 0, 0, 0, 0, false, true).0, 0, "an empty range and a zero rate");
        // the player uses it
        let mut p = Playback { ping_pong: true, ..Default::default() };
        p.play(0.0, 0, 0, 5);
        assert_eq!(p.frame(5.0, 1), 3);
        assert!(p.playing);
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

    #[test]
    fn speed_scales_time_and_keeps_the_frame_when_changed() {
        let mut p = Playback::default();
        p.play(0.0, 0, 0, 100);
        p.set_speed(0.0, 10, 0.5);
        assert_eq!(p.frame(2.0, 10), 10, "half speed: 2 s show 10 frames at 10 fps");
        p.set_speed(2.0, 10, 2.0);
        assert_eq!(p.frame(2.0, 10), 10, "changing speed does not jump");
        assert_eq!(p.frame(3.0, 10), 30);
        p.set_speed(3.0, 10, f64::NAN);
        assert_eq!(p.speed(), 1.0);
        p.set_speed(3.0, 10, 1e9);
        assert_eq!(p.speed(), MAX_SPEED);
    }
}
