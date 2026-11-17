//! A BoardCraft board or animatic as the starting layout of a shot: every panel becomes a
//! drawing on a locked reference layer, held for the panel's duration and marked as a key, and
//! the panels' camera keys become the shot's camera keys.

use tooncraft_doc::{Affine, CameraKey, Cell, DocError, Mark};
use tooncraft_interop::boards::Board;
use tooncraft_raster::{RgbaImage, Surface};

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Place a board picture on the shot's canvas: scaled to fit, centred. Returns the map from
/// board pixels to canvas pixels.
fn placement(board: &Board, w: u32, h: u32) -> Affine {
    let s = (w as f32 / board.width as f32).min(h as f32 / board.height as f32);
    let (ox, oy) = ((w as f32 - board.width as f32 * s) / 2.0, (h as f32 - board.height as f32 * s) / 2.0);
    Affine { a: s, b: 0.0, c: 0.0, d: s, tx: ox, ty: oy }
}

pub(crate) fn import(e: &mut Engine, path: &str, frame: u32, camera: bool) -> Result<()> {
    let meta = std::fs::metadata(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    if meta.len() > tooncraft_interop::MAX_IMPORT_FILE * 4 {
        return Err(EngineError::Import(format!("{path}: the board is larger than 2 GB")));
    }
    let bytes = std::fs::read(path).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    let board = tooncraft_interop::boards::read(&bytes).map_err(|err| EngineError::Import(format!("{path}: {err}")))?;
    if board.panels.is_empty() {
        return Err(EngineError::Import(format!("{path}: the board has no panels")));
    }
    let end = frame as u64 + board.frames();
    if end > tooncraft_doc::MAX_FRAMES as u64 {
        return Err(DocError::TooLong.into());
    }
    let (w, h) = (e.project.width(), e.project.height());
    let place = placement(&board, w, h);
    // render every panel first: a bad board changes nothing
    let mut surfaces = Vec::with_capacity(board.panels.len());
    for p in &board.panels {
        let mut src = Surface::new(board.width, board.height)?;
        src.load_image(&p.image)?;
        let mut img = RgbaImage::transparent(w, h);
        src.composite_affine(&mut img, (0, 0), &place, 1.0, None);
        let mut s = Surface::new(w, h)?;
        s.load_image(&img)?;
        surfaces.push(s);
    }
    let parts = vec![Part::Whole(Box::new(e.project.clone()))];
    if (e.project.length() as u64) < end {
        e.project.set_length(end as u32)?;
    }
    let li = e.project.add_layer(&format!("Boards: {}", if board.name.is_empty() { "board" } else { &board.name }), 0)?;
    let mut f = frame;
    let s = place.a;
    for (panel, surface) in board.panels.iter().zip(surfaces) {
        let id = e.project.add_drawing(Some(surface))?;
        e.project.marks.insert(id, Mark::Key);
        let layer = e.project.layer_mut(li)?;
        layer.cells[f as usize] = Cell::Key(id);
        for k in 1..panel.duration {
            if let Some(c) = layer.cells.get_mut((f + k) as usize) {
                *c = Cell::Hold;
            }
        }
        if camera {
            for k in &panel.camera {
                let (x, y) = place.apply(k.center.0 * board.width as f32, k.center.1 * board.height as f32);
                // board zoom 1 frames the whole board, which covers `s × board` of the canvas
                let fit = (board.width as f32 * s / w as f32).max(board.height as f32 * s / h as f32);
                let key = CameraKey { frame: f + k.frame, x, y, zoom: k.zoom / fit.max(1e-3), angle: k.rotation };
                e.project.camera.set(key.sanitized(w, h));
            }
        }
        f += panel.duration;
    }
    let layer = e.project.layer_mut(li)?;
    layer.locked = true;
    layer.opacity = 0.5;
    if camera && board.panels.iter().any(|p| !p.camera.is_empty()) {
        e.project.camera.enabled = true;
    }
    e.push(Entry { label: "Import board".into(), parts });
    Ok(())
}
