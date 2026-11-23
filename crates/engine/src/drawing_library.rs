//! Commands of the shot's drawing library (`tooncraft_doc::library`): save a run of cells as an
//! item, put an item on a layer (copied or linked), rename, move and remove items. Each is one undo
//! step.

use std::collections::BTreeMap;

use tooncraft_doc::library::{self, Item};
use tooncraft_doc::{Cell, DrawingId};

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

/// Run `edit`; on success push one step labelled `label` that brings back the library and layers
/// as they were before (and removes the drawings `edit` created), on failure undo it right away.
fn step(e: &mut Engine, label: &str, edit: impl FnOnce(&mut Engine, &mut Vec<Part>) -> Result<()>) -> Result<()> {
    let mut parts = vec![Part::Library(e.project.library.clone()), Part::Layers(e.project.layers.clone())];
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
    step(e, "Add to library", |e, parts| {
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
    step(e, "Insert from library", |e, parts| {
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
    step(e, "Rename library item", |e, _| {
        e.project.library[item].name = name;
        Ok(())
    })
}

/// Remove an item; its drawings stay in the shot until Purge Unused (or while a linked layer uses
/// them).
pub(crate) fn remove(e: &mut Engine, item: usize) -> Result<()> {
    item_index(e, item)?;
    step(e, "Remove library item", |e, _| {
        e.project.library.remove(item);
        Ok(())
    })
}

/// Move an item to another place in the list.
pub(crate) fn move_item(e: &mut Engine, item: usize, to: usize) -> Result<()> {
    item_index(e, item)?;
    let to = to.min(e.project.library.len() - 1);
    step(e, "Move library item", |e, _| {
        let i = e.project.library.remove(item);
        e.project.library.insert(to, i);
        Ok(())
    })
}
