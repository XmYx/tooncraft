//! Commands of the shot's drawing library (`tooncraft_doc::library`): save a run of cells as an
//! item, put an item on a layer (copied or linked), rename, move and remove items. Each is one undo
//! step.

use std::collections::BTreeMap;

use tooncraft_doc::library::{self, Item};
use tooncraft_doc::{Cell, DrawingId};

use tooncraft_interop::drawing_file::{self, DrawingFile, FileCell};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

fn item_index(e: &Engine, item: usize) -> Result<()> {
    if item < e.project.library.len() { Ok(()) } else { Err(EngineError::NoLibraryItem(item)) }
}

/// Copy every drawing `cells` shows into a new drawing, and point the cells at the copies.
/// The created ids go into `parts` (so undo removes them).
fn copy_drawings(e: &mut Engine, cells: &mut [Cell], parts: &mut Vec<Part>) -> Result<()> {
    let mut map: BTreeMap<DrawingId, DrawingId> = BTreeMap::new();
    for c in cells.iter_mut() {
        let Cell::Key(d) = *c else { continue };
        let id = match map.get(&d) {
            Some(id) => *id,
            None => {
                let surface = e.project.drawing_owned(d)?;
                let id = e.project.add_drawing(surface)?;
                parts.push(Part::Drawing { id, surface: None });
                map.insert(d, id);
                id
            }
        };
        *c = Cell::Key(id);
    }
    Ok(())
}

/// Run `edit`; on success push one step labelled `label` that brings back the library (and the
/// layers, when `layers`) as they were before and removes the drawings `edit` created; on failure
/// undo it right away.
fn step(e: &mut Engine, label: &str, layers: bool, edit: impl FnOnce(&mut Engine, &mut Vec<Part>) -> Result<()>) -> Result<()> {
    let mut parts = vec![Part::Library(e.project.library.clone())];
    if layers {
        parts.push(Part::Layers(e.project.layers.clone()));
    }
    match edit(e, &mut parts) {
        Ok(()) => {
            e.push(Entry { label: label.into(), parts });
            Ok(())
        }
        Err(err) => {
            Entry { label: String::new(), parts }.swap(&mut e.project);
            Err(err)
        }
    }
}

pub(crate) fn save(e: &mut Engine, layer: usize, from: u32, to: u32, name: &str) -> Result<()> {
    if e.project.library.len() >= library::MAX_ITEMS {
        return Err(EngineError::Limit("library items"));
    }
    let src = e.project.layer(layer)?;
    let Some(mut cells) = library::cells_from(&src.cells, from, to) else {
        return Err(EngineError::NoDrawing(from.min(to)));
    };
    let name = library::clean_name(name);
    step(e, "Add to library", false, |e, parts| {
        copy_drawings(e, &mut cells, parts)?;
        e.project.library.push(Item { name, cells });
        Ok(())
    })
}

pub(crate) fn insert(e: &mut Engine, item: usize, layer: usize, frame: u32, linked: bool) -> Result<()> {
    item_index(e, item)?;
    e.check_unlocked(layer)?;
    let len = e.project.length();
    if frame >= len {
        return Err(tooncraft_doc::DocError::NoFrame(frame).into());
    }
    let mut cells = e.project.library[item].cells.clone();
    cells.truncate((len - frame) as usize);
    step(e, "Insert from library", true, |e, parts| {
        if !linked {
            copy_drawings(e, &mut cells, parts)?;
        }
        let end = frame as usize + cells.len();
        // A hold right after the inserted run kept showing the drawing that was there before.
        let after = e.project.drawing_at(layer, end as u32);
        let l = e.project.layer_mut(layer)?;
        if let Some(slot) = l.cells.get_mut(frame as usize..end) {
            slot.copy_from_slice(&cells);
        }
        if let Some(c) = l.cells.get_mut(end)
            && *c == Cell::Hold
        {
            *c = after.map(Cell::Key).unwrap_or(Cell::Blank);
        }
        Ok(())
    })
}

pub(crate) fn rename(e: &mut Engine, item: usize, name: &str) -> Result<()> {
    item_index(e, item)?;
    let name = library::clean_name(name);
    step(e, "Rename library item", false, |e, _| {
        e.project.library[item].name = name;
        Ok(())
    })
}

/// Remove an item; its drawings stay in the shot until Purge Unused (or while a linked layer uses
/// them).
pub(crate) fn remove(e: &mut Engine, item: usize) -> Result<()> {
    item_index(e, item)?;
    step(e, "Remove library item", false, |e, _| {
        e.project.library.remove(item);
        Ok(())
    })
}

/// Move an item to another place in the list.
pub(crate) fn move_item(e: &mut Engine, item: usize, to: usize) -> Result<()> {
    item_index(e, item)?;
    let to = to.min(e.project.library.len() - 1);
    if to == item {
        return Ok(());
    }
    step(e, "Move library item", false, |e, _| {
        let i = e.project.library.remove(item);
        e.project.library.insert(to, i);
        Ok(())
    })
}

/// Write library item `item` to `path` as a drawing file (atomically; not an undo step). The
/// drawings are read and encoded one at a time (a paged drawing is loaded only for its turn).
pub(crate) fn save_file(e: &mut Engine, item: usize, path: &str) -> Result<()> {
    let p = &e.project;
    let it = p.library.get(item).ok_or(EngineError::NoLibraryItem(item))?;
    let ids = it.drawings();
    let index: BTreeMap<DrawingId, u32> = ids.iter().enumerate().map(|(i, d)| (*d, i as u32)).collect();
    let cells: Vec<FileCell> = it
        .cells
        .iter()
        .map(|c| match c {
            Cell::Key(d) => index.get(d).map(|i| FileCell::Key(*i)).unwrap_or(FileCell::Blank),
            Cell::Hold => FileCell::Hold,
            Cell::Blank => FileCell::Blank,
        })
        .collect();
    let drawing = |i: usize| -> std::result::Result<tooncraft_raster::RgbaImage, tooncraft_interop::InteropError> {
        let bad = |m: String| tooncraft_interop::InteropError::Encode(m);
        let id = ids.get(i).ok_or_else(|| bad(format!("no drawing {i}")))?;
        let s = p.drawing_owned(*id).map_err(|err| bad(err.to_string()))?.ok_or_else(|| bad(format!("drawing {} is missing", id.0)))?;
        Ok(s.to_image())
    };
    let bytes = drawing_file::write(&it.name, &cells, (p.width(), p.height()), ids.len(), drawing).map_err(|err| EngineError::Import(err.to_string()))?;
    craft_atomic::atomic_write(std::path::Path::new(path), &bytes).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    Ok(())
}

/// Add the drawing file at `path` to the shot's library as a new item: each drawing is placed on
/// the canvas as `placement` says (centred). One undo step.
pub(crate) fn open_file(e: &mut Engine, path: &str, placement: crate::import::Placement) -> Result<()> {
    if e.project.library.len() >= library::MAX_ITEMS {
        return Err(EngineError::Limit("library items"));
    }
    let p = std::path::Path::new(path);
    let len = std::fs::metadata(p).map_err(|err| EngineError::Import(format!("{path}: {err}")))?.len();
    if len > drawing_file::MAX_FILE {
        return Err(EngineError::Import(format!("{path} is larger than {} MB", drawing_file::MAX_FILE >> 20)));
    }
    let bytes = std::fs::read(p).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    let f = drawing_file::from_bytes(&bytes).map_err(|err| EngineError::Import(err.to_string()))?;
    add_file(e, f, placement)
}

/// Add a decoded drawing file to the library (see [`open_file`]).
pub(crate) fn add_file(e: &mut Engine, f: DrawingFile, placement: crate::import::Placement) -> Result<()> {
    if e.project.library.len() >= library::MAX_ITEMS {
        return Err(EngineError::Limit("library items"));
    }
    let (w, h) = (e.project.width(), e.project.height());
    step(e, "Add drawing file to library", false, |e, parts| {
        let mut ids = Vec::with_capacity(f.drawings.len());
        for img in &f.drawings {
            let placed = if (img.width, img.height) == (w, h) { img.clone() } else { crate::import::place(img, w, h, placement) };
            let mut s = tooncraft_raster::Surface::new(w, h)?;
            s.load_image(&placed)?;
            let id = e.project.add_drawing(Some(s))?;
            parts.push(Part::Drawing { id, surface: None });
            ids.push(id);
        }
        let cells: Vec<Cell> = f
            .cells
            .iter()
            .map(|c| match c {
                FileCell::Key(i) => ids.get(*i as usize).map(|d| Cell::Key(*d)).unwrap_or(Cell::Blank),
                FileCell::Hold => Cell::Hold,
                FileCell::Blank => Cell::Blank,
            })
            .collect();
        let item = Item { name: f.name, cells }.sanitized(|_| true);
        if item.is_empty() {
            return Err(EngineError::Import("the drawing file shows no drawing".into()));
        }
        e.project.library.push(item);
        Ok(())
    })
}
