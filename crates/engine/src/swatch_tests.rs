//! Tests of editing a palette swatch: the paint in its colour follows it, one undo step.

use crate::{Command, Engine, EngineError};
use tooncraft_doc::Project;
use tooncraft_doc::palette::Palette;

const SKIN: [u8; 3] = [250, 210, 190];
const NEW: [u8; 3] = [240, 180, 150];

fn fill(e: &mut Engine, layer: usize, frame: u32, x: u32, c: [u8; 3], alpha: u8) {
    e.apply(Command::Fill { layer, frame, x, y: 4, color: [c[0], c[1], c[2], alpha], options: Default::default(), reference: Default::default() }).unwrap();
}

/// Two layers of two drawings each, all filled with the skin colour; a palette with that swatch.
fn engine() -> Engine {
    let mut e = Engine::new(Project::new("Swatch", 32, 32, 12, 4).unwrap());
    e.apply(Command::AddLayer { name: "Two".into(), index: Some(1) }).unwrap();
    for l in 0..2 {
        for f in [0, 2] {
            e.apply(Command::NewDrawing { layer: l, frame: f }).unwrap();
        }
    }
    fill(&mut e, 0, 0, 4, SKIN, 255);
    fill(&mut e, 0, 2, 4, SKIN, 128);
    fill(&mut e, 1, 0, 4, SKIN, 255);
    fill(&mut e, 1, 2, 4, [10, 10, 10], 255);
    let mut p = Palette::new("Characters");
    p.push("skin", SKIN);
    p.push("shadow", [180, 140, 120]);
    e.apply(Command::SetPalettes { palettes: vec![p] }).unwrap();
    e
}

fn rgb_at(e: &Engine, layer: usize, frame: u32) -> [u8; 4] {
    let d = e.project.drawing_at(layer, frame).unwrap();
    tooncraft_raster::unpremultiply(e.project.drawing(d).unwrap().pixel(4, 4))
}

#[test]
fn changing_a_swatch_recolours_its_paint_everywhere_in_one_step() {
    let mut e = engine();
    let before = e.project.clone();
    let n = e.undo_len();
    e.apply(Command::EditSwatch { palette: 0, swatch: 0, rgb: NEW, name: Some("warm skin".into()), update_paint: true }).unwrap();
    assert_eq!(e.undo_len(), n + 1);
    assert_eq!(e.undo_label(), Some("Change swatch"));
    assert_eq!(e.project.palettes[0].swatches[0].rgb, NEW);
    assert_eq!(e.project.palettes[0].swatches[0].name, "warm skin");
    assert_eq!(rgb_at(&e, 0, 0)[..3], NEW);
    assert_eq!(rgb_at(&e, 1, 0)[..3], NEW, "on the other layer too");
    let half = rgb_at(&e, 0, 2);
    assert!(half[3].abs_diff(128) <= 1, "opacity is kept: {half:?}");
    assert!(half[..3].iter().zip(NEW).all(|(a, b)| a.abs_diff(b) <= 2), "half-transparent paint is recoloured: {half:?}");
    assert_eq!(rgb_at(&e, 1, 2)[..3], [10, 10, 10], "other colours stay");
    e.undo().unwrap();
    assert!(e.project == before, "one undo brings back the swatch and every drawing");
    e.redo().unwrap();
    assert_eq!(rgb_at(&e, 1, 0)[..3], NEW);
}

#[test]
fn without_update_only_the_swatch_changes_and_locked_layers_are_left_alone() {
    let mut e = engine();
    e.apply(Command::EditSwatch { palette: 0, swatch: 0, rgb: NEW, name: None, update_paint: false }).unwrap();
    assert_eq!(e.project.palettes[0].swatches[0].name, "skin", "no name given: the name stays");
    assert_eq!(rgb_at(&e, 0, 0)[..3], SKIN);
    e.undo().unwrap();
    e.apply(Command::SetLayer { layer: 1, visible: None, locked: Some(true), opacity: None }).unwrap();
    e.apply(Command::EditSwatch { palette: 0, swatch: 0, rgb: NEW, name: None, update_paint: true }).unwrap();
    assert_eq!(rgb_at(&e, 0, 0)[..3], NEW);
    assert_eq!(rgb_at(&e, 1, 0)[..3], SKIN, "a locked layer keeps its paint");
}

#[test]
fn a_missing_swatch_is_an_error_and_changes_nothing() {
    let mut e = engine();
    let n = e.undo_len();
    assert!(matches!(e.apply(Command::EditSwatch { palette: 0, swatch: 9, rgb: NEW, name: None, update_paint: true }), Err(EngineError::NoSwatch(0, 9))));
    assert!(e.apply(Command::EditSwatch { palette: 3, swatch: 0, rgb: NEW, name: None, update_paint: true }).is_err());
    assert_eq!(e.undo_len(), n);
    assert_eq!(rgb_at(&e, 0, 0)[..3], SKIN);
}

#[test]
fn edit_swatch_reads_from_a_command_file_line() {
    let line = r#"{"cmd":"edit_swatch","palette":0,"swatch":1,"rgb":[1,2,3]}"#;
    let c: Command = serde_json::from_str(line).unwrap();
    assert_eq!(c, Command::EditSwatch { palette: 0, swatch: 1, rgb: [1, 2, 3], name: None, update_paint: true });
    let mut e = engine();
    e.apply(c).unwrap();
    assert_eq!(e.project.palettes[0].swatches[1].rgb, [1, 2, 3]);
}
