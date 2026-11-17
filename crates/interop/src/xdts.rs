//! XDTS, the exposure-sheet interchange format ("exchange Digital Time Sheet") several 2D
//! animation tools read and write.
//!
//! A file is the line `exchangeDigitalTimeSheet Save Data` followed by one JSON object:
//!
//! ```text
//! exchangeDigitalTimeSheet Save Data
//! {"header":{"cut":"1","scene":"1"},
//!  "timeTables":[{"name":"Shot","duration":24,
//!    "timeTableHeaders":[{"fieldId":0,"names":["A","B"]}],
//!    "fields":[{"fieldId":0,"tracks":[
//!      {"trackNo":0,"frames":[{"frame":0,"data":[{"id":0,"values":["1"]}]},
//!                             {"frame":6,"data":[{"id":0,"values":["SYMBOL_NULL_CELL"]}]}]}]}]}],
//!  "version":5}
//! ```
//!
//! - `fieldId` 0 is the cell field (the drawings); other fields (camera, dialogue) are ignored.
//! - Track `trackNo` n is named by `timeTableHeaders[fieldId 0].names[n]`.
//! - A track lists only the frames (0-based) where its exposure changes; a value holds until the
//!   next entry or the end of the sheet. `SYMBOL_NULL_CELL` is a blank. The other `SYMBOL_…`
//!   marks (hyphen, ticks) annotate a continuing exposure and are read as holds.
//!
//! [`export`] writes one track per ToonCraft layer (bottom to top), named after the layer (made
//! unique), with the drawings numbered 1, 2, 3 … per layer in order of first appearance.
//! [`import`] reads the first time table into a [`Sheet`] with one entry per frame.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tooncraft_doc::{DrawingId, Layer, Project};

use crate::common::track_names;
use crate::{InteropError, Result};

/// The first line of every XDTS file.
pub const MAGIC: &str = "exchangeDigitalTimeSheet Save Data";
/// The file format version written.
pub const VERSION: u32 = 5;
/// Longest sheet read, in frames.
pub const MAX_DURATION: u32 = tooncraft_doc::MAX_FRAMES;
/// Most tracks read.
pub const MAX_TRACKS: usize = 512;
/// Most cells (frames × tracks) one import fills in.
pub const MAX_CELLS: u64 = 1 << 22;
/// Largest file read.
pub const MAX_TEXT: usize = 64 << 20;
/// Longest cell value or track name kept, in characters.
pub const MAX_NAME: usize = 200;

/// The blank-cell symbol.
pub const NULL_CELL: &str = "SYMBOL_NULL_CELL";

/// An exposure sheet read from XDTS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    /// Frames in the sheet.
    pub duration: u32,
    /// The cut and scene from the header, when given.
    pub cut: Option<String>,
    pub scene: Option<String>,
    /// In track order (track 0 first).
    pub tracks: Vec<Track>,
}

/// One column of the sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub name: String,
    /// One entry per frame: the cell value shown (usually a drawing number), `None` when blank.
    pub cells: Vec<Option<String>>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct File {
    header: Option<Header>,
    #[serde(rename = "timeTables")]
    time_tables: Vec<TimeTable>,
    version: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Header {
    cut: Option<serde_json::Value>,
    scene: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct TimeTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    duration: i64,
    fields: Vec<Field>,
    #[serde(rename = "timeTableHeaders")]
    time_table_headers: Vec<TableHeader>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Field {
    #[serde(rename = "fieldId")]
    field_id: i64,
    tracks: Vec<TrackJson>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct TrackJson {
    #[serde(rename = "trackNo")]
    track_no: i64,
    frames: Vec<FrameJson>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct FrameJson {
    frame: i64,
    data: Vec<Data>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Data {
    id: i64,
    values: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct TableHeader {
    #[serde(rename = "fieldId")]
    field_id: i64,
    names: Vec<serde_json::Value>,
}

/// Drawing numbers of a layer: its drawings in order of first appearance, numbered from 1.
pub(crate) fn drawing_numbers(layer: &Layer) -> BTreeMap<DrawingId, u32> {
    let mut map = BTreeMap::new();
    for c in &layer.cells {
        if let Some(d) = c.key() {
            let next = map.len() as u32 + 1;
            map.entry(d).or_insert(next);
        }
    }
    map
}

/// The drawing number shown on each frame of a layer.
pub(crate) fn numbered_cells(layer: &Layer, length: u32) -> Vec<Option<u32>> {
    let numbers = drawing_numbers(layer);
    (0..length).map(|f| layer.drawing_at(f).and_then(|d| numbers.get(&d).copied())).collect()
}

fn cell(frame: u32, value: String) -> FrameJson {
    FrameJson { frame: frame as i64, data: vec![Data { id: 0, values: vec![serde_json::Value::String(value)] }] }
}

/// The project's exposure sheet as XDTS text.
pub fn export(p: &Project) -> String {
    let names = track_names(&p.layers);
    let mut tracks = Vec::with_capacity(p.layers.len());
    for (i, layer) in p.layers.iter().enumerate() {
        let mut frames = Vec::new();
        let mut prev: Option<u32> = None;
        for (f, now) in numbered_cells(layer, p.length()).into_iter().enumerate() {
            if now != prev {
                frames.push(cell(f as u32, now.map_or_else(|| NULL_CELL.to_string(), |n| n.to_string())));
            }
            prev = now;
        }
        tracks.push(TrackJson { track_no: i as i64, frames });
    }
    let file = File {
        header: Some(Header { cut: Some("1".into()), scene: Some("1".into()) }),
        time_tables: vec![TimeTable {
            name: Some(p.name.clone()),
            duration: p.length() as i64,
            fields: vec![Field { field_id: 0, tracks }],
            time_table_headers: vec![TableHeader { field_id: 0, names: names.into_iter().map(serde_json::Value::String).collect() }],
        }],
        version: Some(VERSION.into()),
    };
    // Serializing plain structs of strings and numbers cannot fail; an empty object is the
    // harmless fallback.
    let json = serde_json::to_string(&file).unwrap_or_else(|_| "{}".into());
    format!("{MAGIC}\n{json}\n")
}

fn text_of(v: &serde_json::Value) -> Option<String> {
    let s = match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    Some(s.chars().take(MAX_NAME).collect())
}

fn bad(m: impl Into<String>) -> InteropError {
    InteropError::Decode(m.into())
}

/// Read an XDTS file's first time table.
pub fn import(text: &str) -> Result<Sheet> {
    if text.len() > MAX_TEXT {
        return Err(InteropError::Limit(format!("an exposure sheet over {} MB", MAX_TEXT >> 20)));
    }
    let text = text.trim_start_matches('\u{feff}').trim_start();
    let Some(json) = text.strip_prefix(MAGIC) else {
        return Err(bad("not an XDTS exposure sheet (the first line is wrong)"));
    };
    let file: File = serde_json::from_str(json).map_err(|e| bad(format!("XDTS: {e}")))?;
    let table = file.time_tables.first().ok_or_else(|| bad("XDTS: the file has no time table"))?;
    if table.duration < 1 || table.duration > MAX_DURATION as i64 {
        return Err(InteropError::Limit(format!("a sheet of {} frames (1…{MAX_DURATION})", table.duration)));
    }
    let duration = table.duration as u32;
    let names: Vec<String> =
        table.time_table_headers.iter().find(|h| h.field_id == 0).map(|h| h.names.iter().map(|n| text_of(n).unwrap_or_default()).collect()).unwrap_or_default();
    let empty = Vec::new();
    let field_tracks = table.fields.iter().find(|f| f.field_id == 0).map_or(&empty, |f| &f.tracks);
    let mut count = names.len();
    for t in field_tracks {
        if t.track_no < 0 || t.track_no >= MAX_TRACKS as i64 {
            return Err(InteropError::Limit(format!("track number {} (at most {MAX_TRACKS} tracks)", t.track_no)));
        }
        count = count.max(t.track_no as usize + 1);
    }
    if count > MAX_TRACKS {
        return Err(InteropError::Limit(format!("{count} tracks (at most {MAX_TRACKS})")));
    }
    if count as u64 * duration as u64 > MAX_CELLS {
        return Err(InteropError::Limit(format!("{count} tracks of {duration} frames")));
    }
    let mut tracks: Vec<Track> = (0..count)
        .map(|i| {
            let name = names.get(i).filter(|n| !n.trim().is_empty()).cloned().unwrap_or_else(|| format!("Track {}", i + 1));
            Track { name, cells: Vec::new() }
        })
        .collect();
    for t in field_tracks {
        // Changes by frame; a later entry for the same frame wins.
        let mut changes: BTreeMap<u32, Option<Option<String>>> = BTreeMap::new();
        for f in &t.frames {
            if f.frame < 0 || f.frame >= duration as i64 {
                continue;
            }
            let Some(v) = f.data.iter().find(|d| d.id == 0).and_then(|d| d.values.first()).and_then(text_of) else { continue };
            let change = if v == NULL_CELL {
                Some(None)
            } else if v.starts_with("SYMBOL_") {
                None // a mark on a continuing exposure
            } else {
                Some(Some(v))
            };
            changes.insert(f.frame as u32, change);
        }
        let mut cells = Vec::with_capacity(duration as usize);
        let mut now: Option<String> = None;
        for frame in 0..duration {
            if let Some(Some(c)) = changes.get(&frame) {
                now = c.clone();
            }
            cells.push(now.clone());
        }
        if let Some(track) = tracks.get_mut(t.track_no as usize) {
            track.cells = cells;
        }
    }
    for t in &mut tracks {
        t.cells.resize(duration as usize, None);
    }
    let header_text = |v: Option<&serde_json::Value>| v.and_then(text_of);
    Ok(Sheet {
        duration,
        cut: header_text(file.header.as_ref().and_then(|h| h.cut.as_ref())),
        scene: header_text(file.header.as_ref().and_then(|h| h.scene.as_ref())),
        tracks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::shot;
    use proptest::prelude::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn export_lists_changes_only() {
        let text = export(&shot());
        assert!(text.starts_with("exchangeDigitalTimeSheet Save Data\n{"));
        let v: serde_json::Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
        let t = &v["timeTables"][0];
        assert_eq!(t["duration"], 6);
        assert_eq!(t["timeTableHeaders"][0]["names"], serde_json::json!(["Ink", "Colour"]));
        let ink = &t["fields"][0]["tracks"][0];
        assert_eq!(ink["trackNo"], 0);
        let frames: Vec<(i64, String)> =
            ink["frames"].as_array().unwrap().iter().map(|f| (f["frame"].as_i64().unwrap(), f["data"][0]["values"][0].as_str().unwrap().to_string())).collect();
        assert_eq!(frames, [(0, "1".into()), (2, "2".into()), (4, "1".into()), (5, NULL_CELL.into())]);
        assert_eq!(v["version"], 5);
    }

    #[test]
    fn round_trip() {
        let sheet = import(&export(&shot())).unwrap();
        assert_eq!(sheet.duration, 6);
        assert_eq!(sheet.cut.as_deref(), Some("1"));
        assert_eq!(sheet.tracks.len(), 2);
        assert_eq!(sheet.tracks[0].name, "Ink");
        assert_eq!(sheet.tracks[0].cells, [s("1"), s("1"), s("2"), s("2"), s("1"), None]);
        assert_eq!(sheet.tracks[1].cells, vec![s("1"); 6]);
    }

    #[test]
    fn reads_other_writers() {
        // Unnamed tracks, a numeric value, symbols, a missing track, unsorted and out-of-range
        // frames, other fields.
        let text = r#"exchangeDigitalTimeSheet Save Data
{"header":{"cut":12,"scene":"3"},"timeTables":[{"duration":6,
 "fields":[{"fieldId":3,"tracks":[]},{"fieldId":0,"tracks":[
   {"trackNo":2,"frames":[{"frame":3,"data":[{"id":0,"values":["SYMBOL_NULL_CELL"]}]},
                          {"frame":0,"data":[{"id":0,"values":["A1"]}]},
                          {"frame":1,"data":[{"id":0,"values":["SYMBOL_HYPHEN"]}]},
                          {"frame":2,"data":[{"id":0,"values":[7]}]},
                          {"frame":99,"data":[{"id":0,"values":["9"]}]},
                          {"frame":-1,"data":[{"id":0,"values":["9"]}]}]}]}],
 "timeTableHeaders":[{"fieldId":0,"names":["BG"]}]}],"version":5}"#;
        let sheet = import(text).unwrap();
        assert_eq!(sheet.cut.as_deref(), Some("12"));
        assert_eq!(sheet.tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["BG", "Track 2", "Track 3"]);
        assert_eq!(sheet.tracks[0].cells, vec![None; 6]);
        assert_eq!(sheet.tracks[2].cells, [s("A1"), s("A1"), s("7"), None, None, None]);
    }

    #[test]
    fn hostile_sheets_fail() {
        let head = "exchangeDigitalTimeSheet Save Data\n";
        assert!(import("").is_err());
        assert!(import("{\"timeTables\":[]}").is_err());
        assert!(import(&format!("{head}{{")).is_err());
        assert!(import(&format!("{head}{{\"timeTables\":[]}}")).is_err());
        assert!(matches!(import(&format!("{head}{{\"timeTables\":[{{\"duration\":0}}]}}")), Err(InteropError::Limit(_))));
        assert!(matches!(import(&format!("{head}{{\"timeTables\":[{{\"duration\":99999999}}]}}")), Err(InteropError::Limit(_))));
        assert!(matches!(
            import(&format!("{head}{{\"timeTables\":[{{\"duration\":4,\"fields\":[{{\"fieldId\":0,\"tracks\":[{{\"trackNo\":100000}}]}}]}}]}}")),
            Err(InteropError::Limit(_))
        ));
        assert!(matches!(
            import(&format!("{head}{{\"timeTables\":[{{\"duration\":65536,\"fields\":[{{\"fieldId\":0,\"tracks\":[{{\"trackNo\":511}}]}}]}}]}}")),
            Err(InteropError::Limit(_))
        ));
        assert!(import(&format!("{head}{{\"timeTables\":\"x\"}}")).is_err());
        let full = export(&shot());
        for cut in 0..full.len() - 1 {
            if full.is_char_boundary(cut) {
                assert!(import(&full[..cut]).is_err(), "a sheet cut at {cut} must not load");
            }
        }
    }

    proptest! {
        #[test]
        fn any_text_never_panics(body in ".{0,300}") {
            let _ = import(&format!("{MAGIC}\n{body}"));
        }

        #[test]
        fn corrupt_json_never_panics(at in 0usize..2000, c in any::<char>()) {
            let mut text: Vec<char> = export(&shot()).chars().collect();
            let i = at % text.len();
            text[i] = c;
            let _ = import(&text.into_iter().collect::<String>());
        }
    }
}
