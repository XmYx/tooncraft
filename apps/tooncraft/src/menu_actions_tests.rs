//! Tests of the right-click menu command bodies (`menu_actions`): each changes what it says, as
//! one undo step, and refuses with a reason when it cannot run.

use tooncraft_doc::palette::{Palette, Swatch};
use tooncraft_doc::{BlendMode, Cell};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::dock::Pane;
use crate::menu_actions as act;

fn app() -> ToonApp {
    ToonApp::with_context(&egui::Context::default(), Cli::default()).without_autosave()
}

const RED: [u8; 3] = [200, 30, 20];

/// A drawing on layer 0, frame 0, with a red 4×4 block at (8, 4).
fn red_block(app: &mut ToonApp) {
    assert!(app.run(Command::NewDrawing { layer: 0, frame: 0 }));
    let rgba: Vec<u8> = (0..16).flat_map(|_| [RED[0], RED[1], RED[2], 255]).collect();
    assert!(app.run(Command::PastePixels { layer: 0, frame: 0, x: 8.0, y: 4.0, width: 4, height: 4, rgba }));
    app.layer = 0;
    app.frame = 0;
    app.selection = None;
}

/// The red pixels' bounding box on layer 0, frame 0.
fn red_box(app: &ToonApp) -> (u32, u32, u32, u32) {
    let m = tooncraft_engine::select::colour_mask(&app.engine, 0, 0, RED, 2).expect("red pixels");
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for y in 0..m.height {
        for x in 0..m.width {
            if m.get(x, y) {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    (x0, y0, x1, y1)
}

#[test]
fn flip_and_rotate_act_on_the_whole_drawing_without_a_selection() {
    let mut a = app();
    red_block(&mut a);
    let w = a.engine.project.width();
    let steps = a.engine.undo_len();
    act::flip(&mut a, true);
    assert_eq!(a.engine.undo_len(), steps + 1);
    let (x0, _, x1, _) = red_box(&a);
    assert_eq!((x0, x1), (w - 12, w - 9));
    a.undo();
    assert_eq!(red_box(&a), (8, 4, 11, 7));
    assert!(a.selection.is_none());
    // a quarter turn about the canvas centre: (dx, dy) from the centre goes to (-dy, dx)
    let (cx, cy) = (a.engine.project.width() / 2, a.engine.project.height() / 2);
    let rgba: Vec<u8> = (0..16).flat_map(|_| [RED[0], RED[1], RED[2], 255]).collect();
    assert!(a.run(Command::ClearDrawing { layer: 0, frame: 0 }));
    assert!(a.run(Command::PastePixels { layer: 0, frame: 0, x: (cx + 10) as f32, y: (cy - 2) as f32, width: 4, height: 4, rgba }));
    a.selection = None;
    let steps = a.engine.undo_len();
    act::rotate_90(&mut a, true);
    assert_eq!(a.engine.undo_len(), steps + 1);
    let (x0, y0, x1, y1) = red_box(&a);
    assert!(x0 + 3 >= cx && x1 <= cx + 3 && y0 + 1 >= cy + 10 && y1 <= cy + 14, "turned block at {x0},{y0} to {x1},{y1}");
}

#[test]
fn flip_refuses_an_empty_frame_and_a_locked_layer() {
    let mut a = app();
    assert!(act::can_transform(&a).is_err());
    red_block(&mut a);
    assert!(a.run(Command::SetLayer { layer: 0, visible: None, locked: Some(true), opacity: None }));
    let steps = a.engine.undo_len();
    assert!(act::can_transform(&a).is_err());
    act::flip(&mut a, false);
    assert_eq!(a.engine.undo_len(), steps);
    assert_eq!(red_box(&a), (8, 4, 11, 7));
}

#[test]
fn fill_and_stroke_selection_are_one_step_each() {
    let mut a = app();
    red_block(&mut a);
    assert!(act::can_paint_selection(&a).is_err());
    a.color = [20, 30, 200, 255];
    crate::selection_tools::set_rings(&mut a, vec![vec![(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)]]);
    let steps = a.engine.undo_len();
    act::fill_selection(&mut a);
    assert_eq!(a.engine.undo_len(), steps + 1);
    assert_eq!(a.engine.undo_label(), Some(tl!("Fill selection")));
    let blue = tooncraft_engine::select::colour_mask(&a.engine, 0, 0, [20, 30, 200], 2).expect("filled");
    assert!((90..=110).contains(&blue.count()), "about 10×10 pixels, got {}", blue.count());
    assert!(!blue.get(40, 40));
    // two rings, one step
    crate::selection_tools::set_rings(
        &mut a,
        vec![vec![(2.0, 30.0), (12.0, 30.0), (12.0, 40.0), (2.0, 40.0)], vec![(40.0, 2.0), (50.0, 2.0), (50.0, 12.0), (40.0, 12.0)]],
    );
    a.color = [10, 160, 40, 255];
    act::stroke_selection(&mut a);
    assert_eq!(a.engine.undo_len(), steps + 2);
    assert_eq!(a.engine.undo_label(), Some(tl!("Stroke selection")));
    let green = tooncraft_engine::select::colour_mask(&a.engine, 0, 0, [10, 160, 40], 40).expect("stroked");
    assert!(green.get(2, 35) || green.get(3, 35), "the first ring's left side");
    assert!(green.get(45, 2) || green.get(45, 3), "the second ring's top side");
}

#[test]
fn paste_reversed_plays_the_copied_frames_backwards_and_keeps_the_clipboard() {
    let mut a = app();
    assert!(act::can_paste_cells(&a).is_err());
    assert!(a.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(a.run(Command::SetExposure { layer: 0, frame: 0, length: 2 }));
    assert!(a.run(Command::NewDrawing { layer: 0, frame: 2 }));
    let (d1, d2) = (a.engine.project.drawing_at(0, 0).expect("1"), a.engine.project.drawing_at(0, 2).expect("2"));
    (a.layer, a.frame, a.sel_anchor) = (0, 2, Some((0, 0)));
    a.copy_cells();
    let copied = a.cell_clipboard.clone();
    (a.frame, a.sel_anchor) = (4, None);
    act::paste_reversed(&mut a);
    let cells: Vec<Cell> = a.engine.project.layers[0].cells[4..7].to_vec();
    assert_eq!(cells, vec![Cell::Key(d2), Cell::Key(d1), Cell::Hold]);
    assert_eq!(a.cell_clipboard, copied);
}

#[test]
fn ruler_items_set_and_clear_the_loop_and_the_onion_range() {
    let mut a = app();
    assert!(act::has_loop(&a).is_err());
    a.frame = 1;
    act::set_loop_at(&mut a, 3, true);
    act::set_loop_at(&mut a, 6, false);
    assert_eq!(a.tl.range, Some((3, 6)));
    assert_eq!(a.frame, 1, "the playhead stays");
    act::set_loop_at(&mut a, 9_999, false);
    assert_eq!(a.tl.range, Some((3, a.engine.project.length() - 1)));
    act::clear_loop(&mut a);
    assert!(a.tl.range.is_none());
    act::onion_range(&mut a, 5);
    let o = a.engine.project.onion;
    assert!(o.enabled && o.before == 5 && o.after == 5);
    act::onion_keys_only(&mut a, true);
    assert!(a.engine.project.onion.keys_only);
    act::go_to_frame(&mut a, 4);
    assert_eq!(a.frame, 3);
    act::go_to_frame(&mut a, -20);
    assert_eq!(a.frame, 0);
}

#[test]
fn layer_items_change_onion_opacity_blend_and_peg() {
    let mut a = app();
    assert!(act::layer_in_onion(&a, 0));
    act::set_layer_in_onion(&mut a, 0, false);
    assert!(!act::layer_in_onion(&a, 0));
    act::set_layer_in_onion(&mut a, 0, true);
    assert!(act::layer_in_onion(&a, 0));
    assert!(a.engine.project.layers[0].onion.is_none(), "back to the shot's settings");
    act::set_layer_opacity(&mut a, 0, 50);
    assert!((a.engine.project.layers[0].opacity - 0.5).abs() < 1e-6);
    act::set_layer_opacity(&mut a, 0, 255);
    assert!((a.engine.project.layers[0].opacity - 1.0).abs() < 1e-6);
    act::set_layer_blend(&mut a, 0, BlendMode::Multiply);
    assert_eq!(a.engine.project.layers[0].blend, BlendMode::Multiply);
    let steps = a.engine.undo_len();
    act::add_peg(&mut a, 0);
    assert_eq!(a.engine.undo_len(), steps + 1);
    assert!(a.engine.project.layers[0].peg.is_some());
    assert!(act::can_add_peg(&a, 0).is_err());
    act::add_peg(&mut a, 0);
    assert_eq!(a.engine.undo_len(), steps + 1);
    assert!(act::can_add_peg(&a, 99).is_err());
}

#[test]
fn swatch_items_duplicate_and_select_by_colour() {
    let mut a = app();
    red_block(&mut a);
    a.colour.current = 0;
    assert!(act::can_edit_swatch(&a).is_err());
    let mut p = Palette::new("Shot");
    p.swatches.push(Swatch { name: "Red".into(), rgb: RED });
    assert!(a.run(Command::SetPalettes { palettes: vec![p] }));
    a.colour.current = 1;
    act::duplicate_swatch(&mut a, 0);
    let s = &a.engine.project.palettes[0].swatches;
    assert_eq!(s.len(), 2);
    assert_eq!(s[1], s[0]);
    act::duplicate_swatch(&mut a, 9);
    assert_eq!(a.engine.project.palettes[0].swatches.len(), 2);
    act::select_colour(&mut a, RED);
    assert_eq!(a.tool, Tool::Select);
    let m = crate::selection_tools::mask(&a).expect("selected");
    assert!(m.get(9, 5) && !m.get(20, 20));
    a.selection = None;
    act::select_colour(&mut a, [0, 255, 0]);
    assert!(a.selection.is_none());
}

#[test]
fn place_as_new_layer_is_one_step() {
    let mut a = app();
    assert!(a.run(Command::NewDrawing { layer: 0, frame: 0 }));
    assert!(a.run(Command::SaveToLibrary { layer: 0, from: 0, to: 0, name: "Blink".into() }));
    let layers = a.engine.project.layers.len();
    let steps = a.engine.undo_len();
    (a.layer, a.frame) = (0, 2);
    act::place_as_new_layer(&mut a, 0);
    assert_eq!(a.engine.project.layers.len(), layers + 1);
    assert_eq!(a.layer, 1);
    assert_eq!(a.engine.project.layers[1].name, "Blink");
    assert!(a.engine.project.drawing_at(1, 2).is_some());
    assert_eq!(a.engine.undo_len(), steps + 1);
    a.undo();
    assert_eq!(a.engine.project.layers.len(), layers);
    act::place_as_new_layer(&mut a, 42);
    assert_eq!(a.engine.project.layers.len(), layers);
    assert!(act::can_show_item_in_folder().is_err());
}

#[test]
fn tool_and_panel_items_reset_settings_and_places() {
    let mut a = app();
    let kind = Tool::Ink.brush().expect("ink is a brush");
    a.brush_mut(kind).size = 77.0;
    act::reset_tool(&mut a, Tool::Ink);
    assert_eq!(a.brush_mut(kind).size, tooncraft_brush::Brush::preset(kind).size);
    assert!(act::can_reset_tool(Tool::Hand).is_err());
    act::edit_tool_shortcut(&mut a, Tool::Ink);
    assert!(a.show_shortcuts);
    assert_eq!(a.keys.filter, Tool::Ink.label());
    assert!(a.workspace.close(Pane::Onion));
    act::reset_panel(&mut a, Pane::Onion);
    assert!(a.workspace.is_open(Pane::Onion));
    let home = crate::dock::Workspace::default().layout.location(&Pane::Onion).ok();
    assert_eq!(a.workspace.layout.location(&Pane::Onion).ok(), home);
}
