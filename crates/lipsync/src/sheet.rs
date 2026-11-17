//! Mouth timing sheets as text, documented in `docs/file-format.md`:
//!
//! - **ToonCraft mouth sheet**: a first line `tooncraft-mouths 1`, an optional `fps N` line, then
//!   one `FRAME MOUTH` line per change (frames from 1, mouths by their [`Mouth::id`]); `#` starts
//!   a comment.
//! - **Switch data**: a first line `MohoSwitch1`, then `FRAME NAME` lines (frames from 1) with the
//!   phoneme-group names `AI E O U WQ MBP FV L etc rest`, as several lip-sync tools write it.
//!
//! Reading takes either; lines that do not parse are reported with their line number.

use crate::Mouth;

/// Most lines read from a sheet.
pub const MAX_LINES: usize = 200_000;
/// Largest frame number accepted.
pub const MAX_FRAME: u32 = 1 << 20;

/// A parsed sheet: its frame rate if it says one, and the changes (0-based frames, sorted).
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub fps: Option<u32>,
    pub changes: Vec<(u32, Mouth)>,
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let Some((_, head)) = lines.next() else { return Err("the sheet is empty".into()) };
    let head = head.trim();
    let ours = head.eq_ignore_ascii_case("tooncraft-mouths 1");
    if !ours && !head.eq_ignore_ascii_case("MohoSwitch1") {
        return Err("not a mouth sheet (the first line must be “tooncraft-mouths 1” or “MohoSwitch1”)".into());
    }
    let mut fps = None;
    let mut changes = Vec::new();
    for (n, line) in lines.take(MAX_LINES) {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut parts = line.split_whitespace();
        let (Some(a), Some(b)) = (parts.next(), parts.next()) else { return Err(format!("line {}: expected a frame and a mouth", n + 1)) };
        if ours && a.eq_ignore_ascii_case("fps") {
            fps = Some(b.parse::<u32>().map_err(|_| format!("line {}: bad frame rate", n + 1))?.clamp(1, 240));
            continue;
        }
        let frame: u32 = a.parse().map_err(|_| format!("line {}: bad frame number “{a}”", n + 1))?;
        if frame == 0 || frame > MAX_FRAME {
            return Err(format!("line {}: frame {frame} is out of range", n + 1));
        }
        let mouth = Mouth::parse(b).ok_or_else(|| format!("line {}: unknown mouth “{b}”", n + 1))?;
        changes.push((frame - 1, mouth));
    }
    changes.sort_by_key(|(f, _)| *f);
    changes.dedup_by_key(|(f, _)| *f);
    Ok(Sheet { fps, changes })
}

/// Write a ToonCraft mouth sheet.
pub fn write(fps: u32, changes: &[(u32, Mouth)]) -> String {
    let mut s = format!("tooncraft-mouths 1\nfps {fps}\n");
    for (f, m) in changes {
        s.push_str(&format!("{} {}\n", f + 1, m.id()));
    }
    s
}

/// Write switch data with phoneme-group names.
pub fn write_switch(changes: &[(u32, Mouth)]) -> String {
    let mut s = String::from("MohoSwitch1\n");
    for (f, m) in changes {
        let name = match m {
            Mouth::Rest => "rest",
            Mouth::Closed => "MBP",
            Mouth::Open => "AI",
            Mouth::Wide => "E",
            Mouth::Round => "O",
            Mouth::Pucker => "U",
            Mouth::Teeth => "FV",
            Mouth::Tongue => "L",
            Mouth::Consonant => "etc",
        };
        s.push_str(&format!("{} {name}\n", f + 1));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_formats_round_trip() {
        let ch = vec![(0, Mouth::Rest), (4, Mouth::Open), (7, Mouth::Closed), (9, Mouth::Pucker)];
        let ours = parse(&write(24, &ch)).unwrap();
        assert_eq!(ours, Sheet { fps: Some(24), changes: ch.clone() });
        let sw = parse(&write_switch(&ch)).unwrap();
        assert_eq!(sw.changes, ch);
        assert_eq!(sw.fps, None);
    }

    #[test]
    fn hostile_sheets_are_errors() {
        assert!(parse("").is_err());
        assert!(parse("hello\n1 rest").is_err());
        assert!(parse("tooncraft-mouths 1\n0 rest").is_err(), "frames count from 1");
        assert!(parse("tooncraft-mouths 1\n3 yawn").unwrap_err().contains("line 2"));
        assert!(parse("tooncraft-mouths 1\n99999999999 rest").is_err());
        let p = parse("MohoSwitch1\n# a comment\n5 ai\n1 rest # start\n5 E\n").unwrap();
        assert_eq!(p.changes, vec![(0, Mouth::Rest), (4, Mouth::Open)]);
    }
}
