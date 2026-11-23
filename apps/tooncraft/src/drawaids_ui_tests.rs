//! UI tests for the drawing aids. They cover polylines placed by clicks and finished with Enter,
//! filled shapes, lasso fill, the gradient tool, symmetry and hold to straighten. Tools are chosen
//! with their keys, and the accessibility check is on.

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::symmetry::{Symmetry, SymmetryMode};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;
use crate::draw_aids::{ShapeFill, ShapeMode};

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1600.0, 940.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h
}

fn canvas(h: &Harness<'static, ToonApp>) -> egui::Rect {
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect()
}

fn click_at(h: &mut Harness<'static, ToonApp>, at: Pos2) {
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run();
}

fn drag_through(h: &mut Harness<'static, ToonApp>, points: &[Pos2]) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else { return };
    h.drag_at(*first);
    h.run();
    for p in &points[1..] {
        h.hover_at(*p);
        h.run();
    }
    h.drop_at(*last);
    h.run();
}

/// Canvas x of every painted pixel on layer 0, frame 0.
fn painted_xs(h: &Harness<'static, ToonApp>) -> Vec<u32> {
    let app = h.state();
    let Some(s) = app.engine.project.drawing_at(0, 0).and_then(|d| app.engine.project.drawing(d)) else { return Vec::new() };
    let t = tooncraft_raster::TILE;
    s.tiles()
        .flat_map(|(i, tile)| {
            let (tx, _) = s.tile_coords(i);
            tile.bytes().as_chunks::<4>().0.iter().enumerate().filter(|(_, p)| p[3] > 0).map(move |(k, _)| tx * t + k as u32 % t).collect::<Vec<_>>()
        })
        .collect()
}

fn painted(h: &Harness<'static, ToonApp>) -> usize {
    painted_xs(h).len()
}

#[test]
fn a_polyline_is_placed_by_clicks_backspace_removes_a_point_and_enter_finishes() {
    let mut h = harness();
    h.key_press(Key::U);
    h.run();
    assert_eq!(h.state().tool, Tool::Shape);
    h.get_by_label("Polyline").click();
    h.run();
    assert_eq!(h.state().aids.shape_mode, ShapeMode::Polyline);
    let c = canvas(&h).center();
    for d in [vec2(-120.0, -60.0), vec2(0.0, 60.0), vec2(120.0, -60.0)] {
        click_at(&mut h, c + d);
    }
    assert_eq!(h.state().aids.path.len(), 3);
    h.key_press(Key::Backspace);
    h.run();
    assert_eq!(h.state().aids.path.len(), 2, "Backspace removes the last point, not the frame");
    click_at(&mut h, c + vec2(140.0, 40.0));
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().aids.path.is_empty());
    assert!(!h.state().playback.playing, "Enter finished the line instead of starting playback");
    assert_eq!(h.state().engine.undo_label(), Some("Shape"));
    assert!(painted(&h) > 200);
    // Escape drops the points without drawing
    let before = h.state().engine.undo_len();
    click_at(&mut h, c);
    assert_eq!(h.state().aids.path.len(), 1);
    h.key_press(Key::Escape);
    h.run();
    assert!(h.state().aids.path.is_empty());
    assert_eq!(h.state().engine.undo_len(), before);
}

#[test]
fn a_filled_rectangle_is_one_undo_step() {
    let mut h = harness();
    h.key_press(Key::U);
    h.run();
    h.get_by_label("Rectangle").click();
    h.run();
    h.get_by_label("Filled").click();
    h.run();
    assert_eq!(h.state().aids.shape_fill, ShapeFill::Filled);
    let c = canvas(&h).center();
    drag_through(&mut h, &[c - vec2(60.0, 40.0), c, c + vec2(60.0, 40.0)]);
    assert_eq!(h.state().engine.undo_label(), Some("Shape"));
    let n = painted(&h);
    assert!(n > 2000, "the inside is filled ({n} pixels)");
    h.state_mut().undo();
    assert_eq!(painted(&h), 0);
}

#[test]
fn lasso_fill_and_the_gradient_tool() {
    let mut h = harness();
    h.key_press(Key::F);
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Lasso fill")).click();
    h.run();
    assert!(h.state().aids.lasso_fill);
    let c = canvas(&h).center();
    let lasso: Vec<Pos2> = (0..=24)
        .map(|k| {
            let a = k as f32 / 24.0 * std::f32::consts::TAU;
            c + vec2(a.cos(), a.sin()) * 80.0
        })
        .collect();
    drag_through(&mut h, &lasso);
    assert_eq!(h.state().engine.undo_label(), Some("Shape"));
    assert!(painted(&h) > 1000);
    h.key_press(Key::G);
    h.run();
    assert_eq!(h.state().tool, Tool::Gradient);
    h.get(By::new().role(egui::Role::CheckBox).label("Fade to transparent"));
    drag_through(&mut h, &[c - vec2(200.0, 0.0), c, c + vec2(200.0, 0.0)]);
    assert_eq!(h.state().engine.undo_label(), Some("Gradient"));
}

#[test]
fn symmetry_is_offered_with_the_brushes_and_mirrors_a_stroke() {
    let mut h = harness();
    h.key_press(Key::N);
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Symmetry"));
    h.get(By::new().role(egui::Role::CheckBox).label("Hold to straighten"));
    h.state_mut().run(Command::SetSymmetry { symmetry: Symmetry { mode: SymmetryMode::Vertical, ..Symmetry::default() } });
    h.state_mut().aids.hold_straighten = false;
    h.run();
    let c = canvas(&h).center();
    drag_through(&mut h, &[c + vec2(60.0, -50.0), c + vec2(80.0, 0.0), c + vec2(60.0, 50.0)]);
    let half = h.state().engine.project.width() / 2;
    let xs = painted_xs(&h);
    let left = xs.iter().filter(|x| **x < half).count();
    let right = xs.len() - left;
    assert!(left > 0 && right > 0, "both halves are painted ({left} / {right})");
    assert_eq!(h.state().engine.undo_len(), 1, "the mirrored stroke is one undo step");
}

#[test]
fn holding_still_turns_a_rough_line_into_a_straight_one() {
    let mut h = harness();
    h.key_press(Key::N);
    h.run();
    assert!(h.state().aids.hold_straighten);
    let c = canvas(&h).center();
    h.drag_at(c - vec2(150.0, 0.0));
    h.run();
    for i in 1..=20 {
        let x = -150.0 + i as f32 * 15.0;
        h.hover_at(c + vec2(x, if i % 2 == 0 { 2.0 } else { -2.0 }));
        h.run();
    }
    h.run();
    std::thread::sleep(std::time::Duration::from_millis(600));
    h.run();
    h.run();
    h.drop_at(c + vec2(150.0, 0.0));
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Shape"), "the stroke was replaced by a line");
    assert_eq!(h.state().engine.undo_len(), 1);
}
