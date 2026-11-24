//! Drawing files (`.toondrawings`): one drawing-library item to keep between shots. A ZIP with a
//! stored `mimetype` entry first, `item.json` (`{"format":"tooncraft-drawings","version":1,"name",
//! "width","height","cells":["blank" | "hold" | {"key": <drawing index>}],"drawings":<count>}`) and
//! `drawings/<index>.png`, one straight-alpha PNG per drawing, all `width`×`height`.
//!
//! Files are hostile: the file size, the entry count, the picture sizes and the total of decoded
//! pixels are capped before anything is allocated, and the cells are repaired like a shot's.

use serde::{Deserialize, Serialize};
use tooncraft_doc::library::{self, MAX_ITEM_CELLS};
use tooncraft_raster::RgbaImage;

use crate::common::{Budget, decode_png};
use crate::{InteropError, Result};

pub const EXTENSION: &str = "toondrawings";
const MIMETYPE: &str = "application/vnd.storyteller.tooncraft-drawings";
const FORMAT: &str = "tooncraft-drawings";
const VERSION: u32 = 1;
/// Largest drawing file read.
pub const MAX_FILE: u64 = 256 << 20;
/// Most drawings in one file (an item has at most this many cells, so at most this many drawings).
pub const MAX_DRAWINGS: usize = MAX_ITEM_CELLS;

/// One cell of a drawing file: a drawing by its index in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileCell {
    Blank,
    Hold,
    Key(u32),
}

/// A drawing file's contents.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingFile {
    pub name: String,
    pub cells: Vec<FileCell>,
    /// Premultiplied, all the same size.
    pub drawings: Vec<RgbaImage>,
}

#[derive(Serialize, Deserialize)]
struct Head {
    format: String,
    version: u32,
    name: String,
    width: u32,
    height: u32,
    cells: Vec<FileCell>,
    drawings: u32,
}

fn entry(i: usize) -> String {
    format!("drawings/{i}.png")
}

pub fn to_bytes(f: &DrawingFile) -> Result<Vec<u8>> {
    let (w, h) = f.drawings.first().map(|d| (d.width, d.height)).unwrap_or((1, 1));
    write(&f.name, &f.cells, (w, h), f.drawings.len(), |i| f.drawings.get(i).cloned().ok_or_else(|| InteropError::Encode(format!("no drawing {i}"))))
}

/// Encode a drawing file whose `count` drawings, all `size`, come from `drawing(i)` one at a
/// time, in order: each is encoded to PNG and dropped before the next is asked for, so writing a
/// long item holds one picture at a time (plus the compressed file).
pub fn write(name: &str, cells: &[FileCell], size: (u32, u32), count: usize, mut drawing: impl FnMut(usize) -> Result<RgbaImage>) -> Result<Vec<u8>> {
    if count > MAX_DRAWINGS {
        return Err(InteropError::Limit(format!("{count} drawings")));
    }
    let head =
        Head { format: FORMAT.into(), version: VERSION, name: name.to_string(), width: size.0, height: size.1, cells: cells.to_vec(), drawings: count as u32 };
    let z = |e: craft_zip::Error| InteropError::Encode(e.to_string());
    let mut zip = craft_zip::Writer::new();
    zip.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).map_err(z)?;
    let json = serde_json::to_vec_pretty(&head).map_err(|e| InteropError::Encode(e.to_string()))?;
    zip.add("item.json", &json, craft_zip::Compression::Deflated).map_err(z)?;
    for i in 0..count {
        let d = drawing(i)?;
        if (d.width, d.height) != size {
            return Err(InteropError::Encode("the drawings are not all the same size".into()));
        }
        let png = tooncraft_format::encode_png(&d).map_err(|e| InteropError::Encode(e.to_string()))?;
        drop(d);
        zip.add(&entry(i), &png, craft_zip::Compression::Stored).map_err(z)?;
    }
    zip.finish().map_err(z)
}

pub fn from_bytes(bytes: &[u8]) -> Result<DrawingFile> {
    if bytes.len() as u64 > MAX_FILE {
        return Err(InteropError::Limit(format!("a drawing file of more than {} MB", MAX_FILE >> 20)));
    }
    if craft_zip::stored_mimetype(bytes) != Some(MIMETYPE.as_bytes()) {
        return Err(InteropError::Decode("not a ToonCraft drawing file".into()));
    }
    let bad = |e: craft_zip::Error| if e.is_limit() { InteropError::Limit(e.to_string()) } else { InteropError::Decode(e.to_string()) };
    let limits = craft_zip::Limits { max_entries: MAX_DRAWINGS as u64 + 4, max_total_bytes: MAX_FILE * 4 };
    let mut zip = craft_zip::Archive::open(bytes, limits).map_err(bad)?;
    let read = |zip: &mut craft_zip::Archive, name: &str, max: u64| match zip.read(name, max) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(InteropError::Decode(format!("{name} is missing"))),
        Err(e) => Err(bad(e)),
    };
    let json = read(&mut zip, "item.json", 4 << 20)?;
    let head: Head = serde_json::from_slice(&json).map_err(|e| InteropError::Decode(format!("item.json: {e}")))?;
    if head.format != FORMAT {
        return Err(InteropError::Decode("item.json is not a drawing file".into()));
    }
    if head.version > VERSION {
        return Err(InteropError::Decode(format!("made by a newer ToonCraft (version {})", head.version)));
    }
    let n = head.drawings as usize;
    if n > MAX_DRAWINGS || head.cells.len() > MAX_ITEM_CELLS * 4 {
        return Err(InteropError::Limit(format!("{n} drawings or {} cells", head.cells.len())));
    }
    if head.width == 0 || head.height == 0 || head.width > tooncraft_raster::MAX_SIDE || head.height > tooncraft_raster::MAX_SIDE {
        return Err(InteropError::Limit(format!("a {}×{} drawing", head.width, head.height)));
    }
    let mut budget = Budget::default();
    let mut drawings = Vec::with_capacity(n);
    for i in 0..n {
        budget.take(head.width, head.height)?;
        let png = read(&mut zip, &entry(i), MAX_FILE)?;
        // a picture larger than the declared size is refused before its pixels are allocated
        let img = decode_png(&png, head.width.max(head.height))?;
        if (img.width, img.height) != (head.width, head.height) {
            return Err(InteropError::Decode(format!("drawing {i} is {}×{}, not {}×{}", img.width, img.height, head.width, head.height)));
        }
        drawings.push(img);
    }
    // repair: keys past the drawings become blanks, holds with nothing to hold too
    let mut cells: Vec<FileCell> = head.cells.into_iter().take(MAX_ITEM_CELLS).collect();
    let mut showing = false;
    for c in &mut cells {
        match *c {
            FileCell::Key(k) if (k as usize) < n => showing = true,
            FileCell::Key(_) | FileCell::Blank => {
                *c = FileCell::Blank;
                showing = false;
            }
            FileCell::Hold if !showing => *c = FileCell::Blank,
            FileCell::Hold => {}
        }
    }
    while cells.last() == Some(&FileCell::Blank) {
        cells.pop();
    }
    Ok(DrawingFile { name: library::clean_name(&head.name), cells, drawings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32, px: [u8; 4]) -> RgbaImage {
        let mut i = RgbaImage::transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                i.set_pixel(x, y, px);
            }
        }
        i
    }

    fn sample() -> DrawingFile {
        DrawingFile {
            name: "Blink".into(),
            cells: vec![FileCell::Key(0), FileCell::Hold, FileCell::Key(1), FileCell::Key(0)],
            drawings: vec![img(8, 6, [200, 20, 20, 255]), img(8, 6, [10, 10, 100, 128])],
        }
    }

    #[test]
    fn round_trip() {
        let f = sample();
        let back = from_bytes(&to_bytes(&f).unwrap()).unwrap();
        assert_eq!(back.name, "Blink");
        assert_eq!(back.cells, f.cells);
        assert_eq!(back.drawings.len(), 2);
        assert_eq!(back.drawings[0].pixel(3, 3), [200, 20, 20, 255]);
        let p = back.drawings[1].pixel(1, 1);
        assert!(p[3] == 128 && p[2].abs_diff(100) <= 1, "premultiplied colour kept: {p:?}");
    }

    #[test]
    fn writing_asks_for_one_drawing_at_a_time_in_order() {
        let mut asked = Vec::new();
        let cells = [FileCell::Key(0), FileCell::Key(1), FileCell::Key(2)];
        let bytes = write("Run", &cells, (8, 6), 3, |i| {
            asked.push(i);
            Ok(img(8, 6, [i as u8 * 50, 0, 0, 255]))
        })
        .unwrap();
        assert_eq!(asked, [0, 1, 2], "each drawing is made once, in order, when it is written");
        let back = from_bytes(&bytes).unwrap();
        assert_eq!(back.drawings[2].pixel(0, 0), [100, 0, 0, 255]);
        // a failing or wrongly sized drawing stops the writing at once
        let mut n = 0;
        assert!(
            write("Run", &cells, (8, 6), 3, |i| {
                n += 1;
                if i == 1 { Ok(img(4, 4, [0; 4])) } else { Ok(img(8, 6, [0; 4])) }
            })
            .is_err()
        );
        assert_eq!(n, 2, "nothing is asked for after the bad drawing");
        assert!(write("Run", &cells, (8, 6), MAX_DRAWINGS + 1, |_| Ok(img(8, 6, [0; 4]))).is_err());
    }

    #[test]
    fn mixed_sizes_are_refused_when_writing() {
        let mut f = sample();
        f.drawings[1] = img(4, 4, [0, 0, 0, 255]);
        assert!(to_bytes(&f).is_err());
    }

    fn rewrite(f: &DrawingFile, edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let bytes = to_bytes(f).unwrap();
        let mut zip = craft_zip::Archive::open(&bytes, craft_zip::Limits { max_entries: 100, max_total_bytes: 1 << 30 }).unwrap();
        let mut head: serde_json::Value = serde_json::from_slice(&zip.read("item.json", 1 << 20).unwrap().unwrap()).unwrap();
        edit(&mut head);
        let mut w = craft_zip::Writer::new();
        w.add("mimetype", MIMETYPE.as_bytes(), craft_zip::Compression::Stored).unwrap();
        w.add("item.json", &serde_json::to_vec(&head).unwrap(), craft_zip::Compression::Deflated).unwrap();
        for i in 0..f.drawings.len() {
            let b = zip.read(&entry(i), 1 << 30).unwrap().unwrap();
            w.add(&entry(i), &b, craft_zip::Compression::Stored).unwrap();
        }
        w.finish().unwrap()
    }

    #[test]
    fn hostile_files_are_refused_or_repaired() {
        let f = sample();
        // not a drawing file at all, empty, truncated
        assert!(from_bytes(b"").is_err());
        assert!(from_bytes(b"PK\x03\x04 nonsense").is_err());
        let good = to_bytes(&f).unwrap();
        assert!(from_bytes(&good[..good.len() / 2]).is_err());
        // more drawings than allowed, a declared size the pictures do not have, a huge size
        assert!(matches!(from_bytes(&rewrite(&f, |h| h["drawings"] = 100_000.into())), Err(InteropError::Limit(_))));
        assert!(from_bytes(&rewrite(&f, |h| h["drawings"] = 3.into())).is_err(), "a listed drawing is missing");
        assert!(from_bytes(&rewrite(&f, |h| h["width"] = 4.into())).is_err(), "a picture larger than declared");
        assert!(matches!(from_bytes(&rewrite(&f, |h| h["width"] = 1_000_000.into())), Err(InteropError::Limit(_))));
        assert!(from_bytes(&rewrite(&f, |h| h["version"] = 99.into())).is_err());
        assert!(from_bytes(&rewrite(&f, |h| h["format"] = "other".into())).is_err());
        // cells: keys past the drawings and orphan holds become blanks, long lists are cut
        let back = from_bytes(&rewrite(&f, |h| {
            h["cells"] = serde_json::json!(["hold", {"key": 7}, "hold", {"key": 1}, "blank"]);
            h["name"] = "\u{7}".repeat(300).into();
        }))
        .unwrap();
        assert_eq!(back.cells, vec![FileCell::Blank, FileCell::Blank, FileCell::Blank, FileCell::Key(1)]);
        assert_eq!(back.name, "Drawing");
        let long = from_bytes(&rewrite(&f, |h| h["cells"] = serde_json::json!(vec![serde_json::json!({"key": 0}); MAX_ITEM_CELLS * 2]))).unwrap();
        assert_eq!(long.cells.len(), MAX_ITEM_CELLS);
        assert!(
            from_bytes(&rewrite(&f, |h| h["cells"] = serde_json::json!(vec![serde_json::json!("hold"); MAX_ITEM_CELLS * 5]))).is_err(),
            "far too many cells"
        );
    }
}
