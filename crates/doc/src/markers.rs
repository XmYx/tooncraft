//! Frame markers (a short note on a frame of the shot) and timecode.

use crate::{DocError, Project, Result};

/// Most markers in a shot.
pub const MAX_MARKERS: usize = 4096;
/// Longest marker note, in characters.
pub const MAX_MARKER_TEXT: usize = 500;

/// Longest drawing name, in characters.
pub const MAX_NAME: usize = 80;

/// `HH:MM:SS:FF` for a 0-based frame at `fps` (non-drop-frame: `FF` counts frames within the
/// second, 0 … fps-1). Hours do not wrap.
pub fn timecode(frame: u32, fps: u32) -> String {
    let fps = fps.max(1) as u64;
    let f = frame as u64;
    let (s, ff) = (f / fps, f % fps);
    format!("{:02}:{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60, ff)
}

/// The frame a timecode names, from `HH:MM:SS:FF`, `MM:SS:FF`, `SS:FF` or a bare frame number
/// (1-based, as the exposure sheet shows). `None` for anything else, a frame count past `fps`,
/// or a result that does not fit.
pub fn parse_timecode(text: &str, fps: u32) -> Option<u32> {
    let fps = fps.max(1) as u64;
    let text = text.trim();
    if text.is_empty() || text.len() > 32 {
        return None;
    }
    let parts: Vec<&str> = text.split([':', ';']).collect();
    let mut n = Vec::with_capacity(parts.len());
    for p in &parts {
        n.push(p.trim().parse::<u64>().ok()?);
    }
    let frame = match n.as_slice() {
        [one] => return u32::try_from(one.checked_sub(1)?).ok(),
        [s, f] => (*s, 0, 0, *f),
        [m, s, f] => (0, *m, *s, *f),
        [h, m, s, f] => (*h, *m, *s, *f),
        _ => return None,
    };
    let (h, m, s, f) = (frame.0, frame.1, frame.2, frame.3);
    // the two-part form is seconds:frames
    let (h, m, s) = if n.len() == 2 { (0, 0, frame.0) } else { (h, m, s) };
    if f >= fps || (n.len() > 2 && (s >= 60 || (n.len() == 4 && m >= 60))) {
        return None;
    }
    let total = h.checked_mul(3600)?.checked_add(m.checked_mul(60)?)?.checked_add(s)?.checked_mul(fps)?.checked_add(f)?;
    u32::try_from(total).ok()
}

impl Project {
    /// Put a note on `frame` (an empty note removes the marker). Notes are cut to
    /// [`MAX_MARKER_TEXT`] characters.
    pub fn set_marker(&mut self, frame: u32, text: &str) -> Result<()> {
        if frame >= self.length() {
            return Err(DocError::NoFrame(frame));
        }
        let text: String = text.trim().chars().filter(|c| !c.is_control()).take(MAX_MARKER_TEXT).collect();
        if text.is_empty() {
            self.markers.remove(&frame);
            return Ok(());
        }
        if self.markers.len() >= MAX_MARKERS && !self.markers.contains_key(&frame) {
            return Err(DocError::TooManyMarkers);
        }
        self.markers.insert(frame, text);
        Ok(())
    }

    /// Give a drawing a name so it can be exposed again by name (an empty name removes it).
    /// Names are unique, ignoring case.
    pub fn set_drawing_name(&mut self, id: crate::DrawingId, name: &str) -> Result<()> {
        if self.drawing_ids().all(|d| d != id) {
            return Err(DocError::NoDrawingId(id.0));
        }
        let name: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME).collect();
        if name.is_empty() {
            self.drawing_names.remove(&id);
            return Ok(());
        }
        if self.drawing_names.iter().any(|(d, n)| *d != id && n.to_lowercase() == name.to_lowercase() && self.drawing_ids().any(|x| x == *d)) {
            return Err(DocError::NameInUse(name));
        }
        if self.drawing_names.len() >= MAX_MARKERS && !self.drawing_names.contains_key(&id) {
            return Err(DocError::TooManyMarkers);
        }
        self.drawing_names.insert(id, name);
        Ok(())
    }

    /// The drawing called `name` (ignoring case), if it still exists.
    pub fn drawing_by_name(&self, name: &str) -> Option<crate::DrawingId> {
        let want = name.trim().to_lowercase();
        if want.is_empty() {
            return None;
        }
        self.drawing_names.iter().find(|(d, n)| n.to_lowercase() == want && self.drawing_ids().any(|x| x == **d)).map(|(d, _)| *d)
    }

    /// The markers inside the shot, in frame order. (Markers past the end are kept for when the
    /// shot grows again, but not shown.)
    pub fn shown_markers(&self) -> impl Iterator<Item = (u32, &str)> {
        self.markers.range(..self.length()).map(|(f, t)| (*f, t.as_str()))
    }

    /// The first marker after `frame`.
    pub fn next_marker(&self, frame: u32) -> Option<u32> {
        self.markers.range(frame.saturating_add(1)..self.length()).next().map(|(f, _)| *f)
    }

    /// The last marker before `frame`.
    pub fn prev_marker(&self, frame: u32) -> Option<u32> {
        self.markers.range(..frame.min(self.length())).next_back().map(|(f, _)| *f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_counts_frames_within_seconds() {
        assert_eq!(timecode(0, 24), "00:00:00:00");
        assert_eq!(timecode(23, 24), "00:00:00:23");
        assert_eq!(timecode(24, 24), "00:00:01:00");
        assert_eq!(timecode(24 * 3600 + 24 * 61 + 5, 24), "01:01:01:05");
        assert_eq!(timecode(7, 0), "00:00:07:00", "a zero rate is treated as 1");
        assert!(timecode(u32::MAX, 1).starts_with("1193046:"), "hours do not wrap");
    }

    #[test]
    fn timecode_round_trips_and_rejects_nonsense() {
        for fps in [1, 12, 24, 25, 30, 120] {
            for f in [0, 1, fps - 1, fps, fps * 61 + 3, 100_000] {
                assert_eq!(parse_timecode(&timecode(f, fps), fps), Some(f), "{f} at {fps}");
            }
        }
        assert_eq!(parse_timecode("1:02", 24), Some(26), "seconds:frames");
        assert_eq!(parse_timecode("1:00:02", 24), Some(60 * 24 + 2), "minutes:seconds:frames");
        assert_eq!(parse_timecode("5", 24), Some(4), "a bare number is a 1-based frame");
        for bad in ["", "0", "abc", "1:2:3:4:5", "00:00:00:24", "00:60:00:00", "00:00:60:00", "-1", "99999999999:00:00:00", "1:-2"] {
            assert_eq!(parse_timecode(bad, 24), None, "{bad:?}");
        }
    }

    #[test]
    fn drawings_are_named_uniquely_and_found_ignoring_case() {
        let mut p = Project::new("n", 16, 16, 24, 10).unwrap();
        let a = p.add_drawing(None).unwrap();
        let b = p.add_drawing(None).unwrap();
        p.set_drawing_name(a, "  Mouth open ").unwrap();
        assert_eq!(p.drawing_by_name("mouth OPEN"), Some(a));
        assert!(matches!(p.set_drawing_name(b, "MOUTH open"), Err(DocError::NameInUse(_))));
        p.set_drawing_name(a, "mouth open").unwrap(); // renaming itself is fine
        p.set_drawing_name(b, "Blink").unwrap();
        assert_eq!(p.drawing_by_name("blink"), Some(b));
        assert_eq!(p.drawing_by_name("  "), None);
        assert_eq!(p.drawing_by_name("nothing"), None);
        assert!(p.set_drawing_name(crate::DrawingId(999), "ghost").is_err());
        p.set_drawing_name(b, "").unwrap();
        assert_eq!(p.drawing_by_name("blink"), None, "an empty name removes it");
        p.set_drawing_name(b, &"é\u{7}".repeat(500)).unwrap();
        assert_eq!(p.drawing_names.get(&b).map(|n| n.chars().count()), Some(MAX_NAME));
        // a name whose drawing was purged is free again
        p.swap_drawing(a, None);
        assert_eq!(p.drawing_by_name("mouth open"), None);
        let c = p.add_drawing(None).unwrap();
        p.set_drawing_name(c, "Mouth open").unwrap();
    }

    #[test]
    fn markers_are_bounded_and_navigable() {
        let mut p = Project::new("m", 32, 32, 24, 100).unwrap();
        p.set_marker(10, "  start of the run  ").unwrap();
        p.set_marker(40, "hit").unwrap();
        assert_eq!(p.markers.get(&10).map(String::as_str), Some("start of the run"));
        assert_eq!(p.next_marker(10), Some(40));
        assert_eq!(p.next_marker(40), None);
        assert_eq!(p.prev_marker(10), None);
        assert_eq!(p.prev_marker(99), Some(40));
        assert!(p.set_marker(100, "past the end").is_err());
        p.set_marker(40, "").unwrap();
        assert_eq!(p.markers.len(), 1, "an empty note removes the marker");
        p.set_marker(11, &"x".repeat(5000)).unwrap();
        assert_eq!(p.markers.get(&11).map(|t| t.chars().count()), Some(MAX_MARKER_TEXT));
        p.set_marker(12, "line\nbreak\u{7}").unwrap();
        assert_eq!(p.markers.get(&12).map(String::as_str), Some("linebreak"), "control characters are dropped");
        p.set_length(11).unwrap();
        assert_eq!(p.shown_markers().map(|(f, _)| f).collect::<Vec<_>>(), vec![10], "markers past the end are not shown");
        assert_eq!(p.next_marker(0), Some(10));
        assert_eq!(p.next_marker(10), None);
        p.set_length(50).unwrap();
        assert_eq!(p.shown_markers().count(), 3, "and come back when the shot grows");
    }
}
