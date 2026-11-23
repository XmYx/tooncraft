//! Tests of the cell commands behind the timeline's right-click menu: swapping a drawing for a
//! whole exposure and filling empty frames.

use crate::{Command, Engine};
use tooncraft_doc::{Cell, DrawingId, Project};

fn engine() -> Engine {
    Engine::new(Project::new("Cells", 64, 64, 12, 8).unwrap())
}

#[test]
fn swap_replaces_the_whole_exposure_and_none_empties_it() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 0 }).unwrap();
    e.apply(Command::SetExposure { layer: 0, frame: 0, length: 3 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 3 }).unwrap();
    let a = e.project.drawing_at(0, 0).unwrap();
    let b = e.project.drawing_at(0, 3).unwrap();
    assert_ne!(a, b);
    e.apply(Command::SwapDrawing { layer: 0, frame: 1, drawing: Some(b) }).unwrap();
    for f in 0..3 {
        assert_eq!(e.project.drawing_at(0, f), Some(b), "frame {f} shows the swapped drawing");
    }
    assert_eq!(e.undo_label(), Some("Swap drawing"));
    e.apply(Command::SwapDrawing { layer: 0, frame: 2, drawing: None }).unwrap();
    assert!((0..3).all(|f| e.project.drawing_at(0, f).is_none()));
    assert_eq!(e.project.drawing_at(0, 3), Some(b));
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.project.drawing_at(0, 1), Some(a));
    // a drawing that does not exist, or a frame past the end, changes nothing
    let before = e.project.layers[0].cells.clone();
    assert!(e.apply(Command::SwapDrawing { layer: 0, frame: 0, drawing: Some(DrawingId(9_999)) }).is_err());
    assert!(e.apply(Command::SwapDrawing { layer: 0, frame: 99, drawing: Some(a) }).is_err());
    assert_eq!(e.project.layers[0].cells, before);
}

#[test]
fn fill_empty_holds_each_drawing_until_the_next() {
    let mut e = engine();
    e.apply(Command::NewDrawing { layer: 0, frame: 2 }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 5 }).unwrap();
    e.apply(Command::FillEmpty { layer: 0 }).unwrap();
    let cells = &e.project.layers[0].cells;
    assert_eq!(cells[0], Cell::Blank, "frames before the first drawing stay blank");
    assert!(matches!(cells[2], Cell::Key(_)));
    assert_eq!(cells[3], Cell::Hold);
    assert_eq!(cells[4], Cell::Hold);
    assert!(matches!(cells[5], Cell::Key(_)));
    assert_eq!(cells[7], Cell::Hold);
    assert_eq!(e.project.drawing_at(0, 4), e.project.drawing_at(0, 2));
}

#[test]
fn fill_modes_empty_only_and_repaint() {
    use tooncraft_raster::fill::{FillMode, FillOptions};
    let mut e = Engine::new(Project::new("Fill", 64, 64, 12, 2).unwrap());
    let opts = |mode| FillOptions { mode, gap: 0, ..Default::default() };
    let red = [200, 20, 20, 255];
    let blue = [20, 20, 200, 255];
    // empty only on an empty canvas fills
    e.apply(Command::Fill { layer: 0, frame: 0, x: 10, y: 10, color: red, options: opts(FillMode::EmptyOnly), reference: Default::default() }).unwrap();
    assert_eq!(e.undo_label(), Some("Fill"));
    let n = e.undo_len();
    // empty only on paint does nothing
    e.apply(Command::Fill { layer: 0, frame: 0, x: 10, y: 10, color: blue, options: opts(FillMode::EmptyOnly), reference: Default::default() }).unwrap();
    assert_eq!(e.undo_len(), n);
    assert_eq!(e.pick_color(0, 10, 10)[..3], red[..3]);
    // repaint recolours paint
    e.apply(Command::Fill { layer: 0, frame: 0, x: 10, y: 10, color: blue, options: opts(FillMode::Repaint), reference: Default::default() }).unwrap();
    assert_eq!(e.pick_color(0, 10, 10)[..3], blue[..3]);
    // repaint on an empty drawing (frame 1 is a new drawing) does nothing
    e.apply(Command::NewDrawing { layer: 0, frame: 1 }).unwrap();
    let n = e.undo_len();
    e.apply(Command::Fill { layer: 0, frame: 1, x: 5, y: 5, color: red, options: opts(FillMode::Repaint), reference: Default::default() }).unwrap();
    assert_eq!(e.undo_len(), n);
}

#[test]
fn recolour_every_drawing_and_palettes_round_trip() {
    let mut e = Engine::new(Project::new("Recolour", 64, 64, 12, 4).unwrap());
    let red = [200, 20, 20, 255];
    e.apply(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: red, options: Default::default(), reference: Default::default() }).unwrap();
    e.apply(Command::NewDrawing { layer: 0, frame: 2 }).unwrap();
    e.apply(Command::Fill { layer: 0, frame: 2, x: 5, y: 5, color: [205, 25, 15, 255], options: Default::default(), reference: Default::default() }).unwrap();
    let n = e.undo_len();
    e.apply(Command::RecolourLayer { layer: 0, from: [200, 20, 20], to: [20, 200, 20], tolerance: 10 }).unwrap();
    assert_eq!(e.undo_len(), n + 1, "both drawings in one step");
    assert_eq!(e.pick_color(0, 5, 5)[..3], [20, 200, 20]);
    assert_eq!(e.pick_color(2, 5, 5)[..3], [20, 200, 20], "a near colour on another drawing too");
    e.undo().unwrap();
    assert_eq!(e.pick_color(2, 5, 5)[..3], [205, 25, 15]);
    // nothing near: no step
    let n = e.undo_len();
    e.apply(Command::RecolourLayer { layer: 0, from: [0, 0, 255], to: [1, 1, 1], tolerance: 0 }).unwrap();
    assert_eq!(e.undo_len(), n);
    // palettes: kept with the shot, bounded
    let mut p = tooncraft_doc::palette::Palette::new("Main");
    p.push("skin", [250, 210, 190]);
    let many = vec![p.clone(); 500];
    e.apply(Command::SetPalettes { palettes: many }).unwrap();
    assert_eq!(e.project.palettes.len(), tooncraft_doc::palette::MAX_PALETTES);
    e.apply(Command::SetPalettes { palettes: vec![p.clone()] }).unwrap();
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap().palettes, vec![p]);
}

#[test]
fn split_and_duplicate_sound_clips() {
    use std::sync::Arc;
    use tooncraft_doc::sound::Soundtrack;
    let mut e = Engine::new(Project::new("Sound", 64, 64, 10, 40).unwrap());
    e.project.sounds.push(Soundtrack { length: Some(3.0), fade_in: 0.5, fade_out: 0.5, ..Soundtrack::new("a.wav", 5, Arc::new(Vec::new())) });
    // split 1 s in (frame 15 at 10 fps)
    e.apply(Command::SplitSound { index: 0, frame: 15 }).unwrap();
    let s = &e.project.sounds;
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].offset, s[0].length, s[0].fade_out), (5, Some(1.0), 0.0));
    assert_eq!((s[1].offset, s[1].trim_start, s[1].length, s[1].fade_in), (15, 1.0, Some(2.0), 0.0));
    assert_eq!(e.undo_label(), Some("Split sound"));
    // outside the clip: refused, nothing changes
    let n = e.undo_len();
    assert!(e.apply(Command::SplitSound { index: 0, frame: 5 }).is_err());
    assert!(e.apply(Command::SplitSound { index: 1, frame: 99 }).is_err());
    assert!(e.apply(Command::SplitSound { index: 9, frame: 10 }).is_err());
    assert_eq!(e.undo_len(), n);
    // duplicate at frame 30
    e.apply(Command::DuplicateSound { index: 1, offset: 30 }).unwrap();
    assert_eq!(e.project.sounds.len(), 3);
    assert_eq!(e.project.sounds[2].offset, 30);
    assert_eq!(e.project.sounds[2].trim_start, 1.0);
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.project.sounds.len(), 1);
}

#[test]
fn onion_settings_per_layer() {
    use tooncraft_doc::OnionSkin;
    let mut e = Engine::new(Project::new("Onion", 64, 64, 12, 8).unwrap());
    e.apply(Command::AddLayer { name: "Top".into(), index: None }).unwrap();
    let own = OnionSkin { before: 5, after: 0, ..e.project.onion };
    e.apply(Command::SetLayerOnion { layer: 1, onion: Some(own) }).unwrap();
    assert_eq!(e.project.onion_for(1).before, 5);
    assert_eq!(e.project.onion_for(0).before, e.project.onion.before, "the other layer keeps the shot's");
    // the shot's switch still turns every layer's onion skin off
    e.project.onion.enabled = false;
    assert!(!e.project.onion_for(1).enabled);
    // hostile values are clamped, and it round-trips through the file
    e.apply(Command::SetLayerOnion { layer: 1, onion: Some(OnionSkin { before: 200, opacity: f32::NAN, ..own }) }).unwrap();
    assert_eq!(e.project.onion_for(1).before as usize, tooncraft_doc::ONION_MAX_STEPS);
    let bytes = tooncraft_format::to_bytes(&e.project).unwrap();
    assert_eq!(tooncraft_format::from_bytes(&bytes).unwrap().layers[1].onion.map(|o| o.before), Some(tooncraft_doc::ONION_MAX_STEPS as u8));
    e.apply(Command::SetLayerOnion { layer: 1, onion: None }).unwrap();
    assert!(e.project.layers[1].onion.is_none());
}
