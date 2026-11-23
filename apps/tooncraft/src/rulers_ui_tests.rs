//! UI tests for the stroke rulers: the Rulers toggle and its menu (from the keyboard too), a
//! freehand stroke that follows a straight ruler, typed handle positions, and a handle dragged
//! on the canvas without painting. The accessibility check is on.

use egui::{Key, Modifiers, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use tooncraft_doc::rulers::Ruler;

use crate::app::ToonApp;
use crate::args::Cli;

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

/// Screen position of a canvas pixel.
fn screen(h: &Harness<'static, ToonApp>, x: f32, y: f32) -> Pos2 {
    let app = h.state();
    let size = vec2(app.engine.project.width() as f32, app.engine.project.height() as f32);
    app.view.to_screen(app.workspace.canvas_rect, size, pos2(x, y))
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

/// Canvas y of every painted pixel on layer 0, frame 0.
fn painted_ys(h: &Harness<'static, ToonApp>) -> Vec<u32> {
    let app = h.state();
    let Some(s) = app.engine.project.drawing_at(0, 0).and_then(|d| app.engine.project.drawing(d)) else { return Vec::new() };
    let t = tooncraft_raster::TILE;
    s.tiles()
        .flat_map(|(i, tile)| {
            let (_, ty) = s.tile_coords(i);
            tile.bytes().as_chunks::<4>().0.iter().enumerate().filter(|(_, p)| p[3] > 0).map(move |(k, _)| ty * t + k as u32 / t).collect::<Vec<_>>()
        })
        .collect()
}

/// Press Tab until the widget called `label` has the keyboard focus; false if it never does.
fn tab_to(h: &mut Harness<'static, ToonApp>, label: &str) -> bool {
    for _ in 0..80 {
        if h.query_by_label(label).is_some_and(|n| n.is_focused()) {
            return true;
        }
        h.key_press(Key::Tab);
        h.run();
    }
    false
}

#[test]
fn the_rulers_menu_works_from_the_keyboard() {
    let mut h = harness();
    // the canvas bar's Rulers toggle, reached with Tab
    assert!(tab_to(&mut h, "Rulers"), "Tab reaches the Rulers toggle");
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().engine.project.rulers.on, "Space turns the rulers on");
    assert!(h.state().engine.project.rulers.grid, "with nothing else to follow, the grid");
    assert!(h.get_by_label("Rulers").is_focused(), "focus stays on the toggle");
    // Shift+F10 opens its menu; Tab into it and Space chooses an item
    h.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    h.run();
    assert!(tab_to(&mut h, "Add Straight Ruler"), "Tab reaches the menu items");
    h.key_press(Key::Space);
    h.run();
    let r = &h.state().engine.project.rulers;
    assert_eq!(r.list.len(), 1);
    assert!(matches!(r.list[0], Ruler::Line { .. }));
    // the handle fields take the arrow keys
    let Some(Ruler::Line { a: before, .. }) = h.state().engine.project.rulers.list.first().cloned() else { panic!("a straight ruler") };
    assert!(tab_to(&mut h, "Point 1 X"), "Tab reaches the typed handle fields");
    for _ in 0..5 {
        h.key_press(Key::ArrowUp);
        h.run();
    }
    h.key_press(Key::Enter);
    h.run();
    assert!(
        matches!(h.state().engine.project.rulers.list.first(), Some(Ruler::Line { a, .. }) if a.0 > before.0),
        "moved by keys: {:?}",
        h.state().engine.project.rulers.list
    );
    assert!(tab_to(&mut h, "Remove All Rulers"));
    h.key_press(Key::Space);
    h.run();
    assert!(h.state().engine.project.rulers.list.is_empty());
}

#[test]
fn a_wobbly_stroke_near_a_straight_ruler_runs_along_it() {
    let mut h = harness();
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let y = (hh * 0.5).round();
    h.state_mut().run(tooncraft_engine::Command::SetRulers {
        rulers: tooncraft_doc::rulers::Rulers { on: true, list: vec![Ruler::Line { a: (0.0, y), b: (w, y) }], reach: 80.0, ..Default::default() },
    });
    h.run();
    let pts: Vec<Pos2> = (0..24).map(|i| screen(&h, w * 0.3 + i as f32 * 20.0, y - 30.0 + (i % 3) as f32 * 30.0)).collect();
    drag_through(&mut h, &pts);
    let ys = painted_ys(&h);
    assert!(!ys.is_empty(), "the stroke painted");
    let size = h.state().brush().size;
    let (lo, hi) = (ys.iter().min().copied().unwrap_or(0) as f32, ys.iter().max().copied().unwrap_or(0) as f32);
    assert!(lo >= y - size && hi <= y + size, "paint stays on the ruler: rows {lo}…{hi} around {y} (brush {size})");
}

#[test]
fn dragging_a_ruler_handle_moves_it_without_painting() {
    let mut h = harness();
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    h.state_mut().run(tooncraft_engine::Command::SetRulers {
        rulers: tooncraft_doc::rulers::Rulers { on: true, list: vec![Ruler::Line { a: (w * 0.25, hh * 0.5), b: (w * 0.75, hh * 0.5) }], ..Default::default() },
    });
    h.run();
    let from = screen(&h, w * 0.25, hh * 0.5);
    let to = screen(&h, w * 0.25, hh * 0.25);
    drag_through(&mut h, &[from, from + vec2(0.0, -10.0), to]);
    assert!(painted_ys(&h).is_empty(), "moving a handle is not a stroke");
    let Ruler::Line { a, .. } = h.state().engine.project.rulers.list[0] else { panic!("still a straight ruler") };
    assert!((a.1 - hh * 0.25).abs() < 3.0, "the handle followed the pointer: {a:?}");
    assert_eq!(h.state().engine.undo_len(), 0, "a setting, not an undo step");
}

#[test]
fn escape_during_a_handle_drag_puts_the_ruler_back() {
    let mut h = harness();
    let (w, hh) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    let start = Ruler::Line { a: (w * 0.25, hh * 0.5), b: (w * 0.75, hh * 0.5) };
    h.state_mut()
        .run(tooncraft_engine::Command::SetRulers { rulers: tooncraft_doc::rulers::Rulers { on: true, list: vec![start.clone()], ..Default::default() } });
    h.run();
    let from = screen(&h, w * 0.25, hh * 0.5);
    h.drag_at(from);
    h.run();
    for k in 1..=5 {
        h.hover_at(from + vec2(0.0, -20.0 * k as f32));
        h.run();
    }
    assert_ne!(h.state().engine.project.rulers.list[0], start, "the handle follows while dragging");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().engine.project.rulers.list[0], start, "Escape puts it back");
    h.drop_at(from + vec2(0.0, -120.0));
    h.run();
    assert_eq!(h.state().engine.project.rulers.list[0], start, "letting go after Escape changes nothing");
    assert!(painted_ys(&h).is_empty(), "and paints nothing");
}
