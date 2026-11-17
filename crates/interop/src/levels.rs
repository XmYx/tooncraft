//! Levels: an XDTS exposure sheet plus one PNG per drawing, the layout other 2D animation tools
//! can relink.
//!
//! ```text
//! <dir>/<Shot>.xdts
//! <dir>/<Layer>/<Layer>_0001.png      drawing 1 of the layer, as numbered in the sheet
//! <dir>/<Layer>/<Layer>_0002.png
//! ```
//!
//! Folder and file names are the sheet's track names made safe for every file system
//! ([`safe_file_name`]); track names are unique, so folders never collide. PNGs are the full
//! canvas with a transparent background, at full opacity (the layer's opacity is not baked in).
//!
//! [`import`] reads the sheet and, for each track, the PNG of every cell value from the track's
//! folder next to the sheet. For a value `v` (drawing number `n`) it tries, in order,
//! `<Layer>_<nnnn>.png`, `<Layer>_<v>.png`, `<nnnn>.png` and `<v>.png`. A drawing whose
//! picture is missing is listed in [`ImportedTrack::missing`]; a damaged picture is an error.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tooncraft_doc::Project;
use tooncraft_raster::{MAX_SIDE, RgbaImage};

use crate::common::{Budget, create_dir, decode_png, encode_png, read_file, track_names, write_file};
use crate::xdts::{self, Sheet, drawing_numbers};
use crate::{InteropError, Result, safe_file_name};

/// A sheet with its drawings.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSheet {
    pub duration: u32,
    pub tracks: Vec<ImportedTrack>,
}

/// One track: its cells and the pictures they show.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTrack {
    pub name: String,
    /// One entry per frame, as in [`xdts::Track::cells`].
    pub cells: Vec<Option<String>>,
    /// Premultiplied pictures by cell value.
    pub drawings: BTreeMap<String, RgbaImage>,
    /// Cell values no picture was found for, in order of first appearance.
    pub missing: Vec<String>,
}

/// File name of drawing `n` of a track: `<Track>_<nnnn>.png`.
pub fn drawing_file_name(track: &str, n: u32) -> String {
    format!("{}_{n:04}.png", safe_file_name(track))
}

/// Write the sheet and the drawings under `dir`. Returns the files written, the sheet first.
pub fn export(p: &Project, dir: &Path) -> Result<Vec<PathBuf>> {
    create_dir(dir)?;
    let sheet = dir.join(format!("{}.xdts", safe_file_name(&p.name)));
    write_file(&sheet, xdts::export(p).as_bytes())?;
    let mut out = vec![sheet];
    for (layer, name) in p.layers.iter().zip(track_names(&p.layers)) {
        let numbers = drawing_numbers(layer);
        if numbers.is_empty() {
            continue;
        }
        let folder = dir.join(safe_file_name(&name));
        create_dir(&folder)?;
        for (id, n) in numbers {
            let Some(surface) = p.drawing(id) else { continue };
            let path = folder.join(drawing_file_name(&name, n));
            write_file(&path, &encode_png(&surface.to_image())?)?;
            out.push(path);
        }
    }
    Ok(out)
}

fn candidates(track: &str, value: &str) -> Vec<String> {
    let t = safe_file_name(track);
    let v = safe_file_name(value);
    let n = value.trim().parse::<u32>().ok();
    let mut c = Vec::new();
    if let Some(n) = n {
        c.push(format!("{t}_{n:04}.png"));
    }
    c.push(format!("{t}_{v}.png"));
    if let Some(n) = n {
        c.push(format!("{n:04}.png"));
    }
    c.push(format!("{v}.png"));
    c
}

/// Read an XDTS sheet and the drawings in the track folders next to it.
pub fn import(xdts_path: &Path) -> Result<ImportedSheet> {
    let bytes = read_file(xdts_path)?;
    let text = String::from_utf8(bytes).map_err(|_| InteropError::Decode("the exposure sheet is not UTF-8 text".into()))?;
    let Sheet { duration, tracks, .. } = xdts::import(&text)?;
    let base = xdts_path.parent().unwrap_or(Path::new("."));
    let mut budget = Budget::default();
    let mut count = 0usize;
    let mut out = Vec::with_capacity(tracks.len());
    for t in tracks {
        let folder = base.join(safe_file_name(&t.name));
        let mut drawings = BTreeMap::new();
        let mut missing = Vec::new();
        for v in t.cells.iter().flatten() {
            if drawings.contains_key(v) || missing.contains(v) {
                continue;
            }
            count += 1;
            if count > tooncraft_doc::MAX_DRAWINGS {
                return Err(InteropError::Limit(format!("more than {} drawings", tooncraft_doc::MAX_DRAWINGS)));
            }
            let Some(path) = candidates(&t.name, v).into_iter().map(|c| folder.join(c)).find(|p| p.is_file()) else {
                missing.push(v.clone());
                continue;
            };
            let img = decode_png(&read_file(&path)?, MAX_SIDE).map_err(|e| InteropError::Decode(format!("{}: {e}", path.display())))?;
            budget.take(img.width, img.height)?;
            drawings.insert(v.clone(), img);
        }
        out.push(ImportedTrack { name: t.name, cells: t.cells, drawings, missing });
    }
    Ok(ImportedSheet { duration, tracks: out })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{shot, temp_dir};

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn export_then_import_round_trips() {
        let p = shot();
        let dir = temp_dir("levels");
        let files = export(&p, &dir).unwrap();
        let rel: Vec<String> = files.iter().map(|f| f.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(rel, ["Test shot.xdts", "Ink/Ink_0001.png", "Ink/Ink_0002.png", "Colour/Colour_0001.png"]);
        let sheet = import(&files[0]).unwrap();
        assert_eq!(sheet.duration, 6);
        let ink = &sheet.tracks[0];
        assert_eq!(ink.name, "Ink");
        assert_eq!(ink.cells, [s("1"), s("1"), s("2"), s("2"), s("1"), None]);
        assert!(ink.missing.is_empty());
        let a = p.drawing(p.drawing_at(0, 0).unwrap()).unwrap().to_image();
        let b = p.drawing(p.drawing_at(0, 2).unwrap()).unwrap().to_image();
        assert_eq!(ink.drawings["1"], a);
        assert_eq!(ink.drawings["2"], b);
        assert_eq!(sheet.tracks[1].drawings["1"].pixel(0, 0), [0, 0, 255, 255]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_names_missing_and_damaged() {
        let dir = temp_dir("levels-other");
        let text = r#"exchangeDigitalTimeSheet Save Data
{"timeTables":[{"duration":4,"fields":[{"fieldId":0,"tracks":[{"trackNo":0,"frames":[
 {"frame":0,"data":[{"id":0,"values":["1"]}]},{"frame":1,"data":[{"id":0,"values":["2"]}]},
 {"frame":2,"data":[{"id":0,"values":["3"]}]},{"frame":3,"data":[{"id":0,"values":["../../x"]}]}]}]}],
 "timeTableHeaders":[{"fieldId":0,"names":["A"]}]}]}"#;
        std::fs::write(dir.join("s.xdts"), text).unwrap();
        std::fs::create_dir_all(dir.join("A")).unwrap();
        let png = encode_png(&RgbaImage::filled(3, 2, [1, 2, 3, 255])).unwrap();
        std::fs::write(dir.join("A/A_1.png"), &png).unwrap();
        std::fs::write(dir.join("A/0002.png"), &png).unwrap();
        let sheet = import(&dir.join("s.xdts")).unwrap();
        let a = &sheet.tracks[0];
        assert_eq!(a.drawings.keys().collect::<Vec<_>>(), ["1", "2"]);
        assert_eq!(a.missing, ["3", "../../x"]);
        std::fs::write(dir.join("A/3.png"), &png[..png.len() / 2]).unwrap();
        assert!(import(&dir.join("s.xdts")).is_err(), "a damaged picture is an error");
        assert!(import(&dir.join("nothing.xdts")).is_err());
        std::fs::write(dir.join("bin.xdts"), [0xff, 0xfe, 0]).unwrap();
        assert!(import(&dir.join("bin.xdts")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
