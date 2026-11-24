//! UI tests for the selection tools: ellipse and magic wand, add / subtract / intersect with the
//! option and with Shift and Alt, invert, grow and shrink, skew, free distort and resampling,
//! and that the other rings follow a transform. The accessibility check is on.

use egui::{Key, Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_engine::Command;
use tooncraft_raster::select::Combine;

use crate::app::{SelectMode, ToonApp};
use crate::args::Cli;

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1000.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.run();
    h.key_press(Key::S);
    h.run();
    h
}

fn canvas(h: &Harness<'static, ToonApp>) -> egui::Rect {
    h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect()
}

fn drag(h: &mut Harness<'static, ToonApp>, a: Pos2, b: Pos2, mods: Modifiers) {
    // held keys, as the window system reports them
    h.event(egui::Event::ModifiersChanged(mods));
    h.event(egui::Event::PointerMoved(a));
    h.run();
    h.event(egui::Event::PointerButton { pos: a, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
    h.run();
    for i in 1..=8 {
        h.event(egui::Event::PointerMoved(a + (b - a) * (i as f32 / 8.0)));
        h.run();
    }
    h.event(egui::Event::PointerButton { pos: b, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run();
}

fn count(h: &Harness<'static, ToonApp>) -> usize {
    crate::selection_tools::mask(h.state()).map_or(0, |m| m.count())
}

fn sq(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

#[test]
fn combine_modes_and_modifiers() {
    let mut h = harness();
    {
        let app = h.state_mut();
        crate::selection_tools::set_rings(app, vec![sq(100.0, 100.0, 300.0, 300.0)]);
        crate::selection_tools::add_ring(app, sq(150.0, 150.0, 250.0, 250.0), Combine::Subtract);
    }
    assert_eq!(count(&h), 200 * 200 - 100 * 100, "a hole");
    assert_eq!(crate::selection_tools::all_rings(h.state()).len(), 2);
    crate::selection_tools::add_ring(h.state_mut(), sq(400.0, 100.0, 450.0, 150.0), Combine::Add);
    assert_eq!(count(&h), 200 * 200 - 100 * 100 + 50 * 50);
    crate::selection_tools::add_ring(h.state_mut(), sq(0.0, 0.0, 120.0, 120.0), Combine::Intersect);
    assert_eq!(count(&h), 20 * 20);
    // the option buttons and Shift while dragging
    h.get(By::new().role(egui::Role::Button).label("Add Area")).click();
    h.run();
    assert_eq!(h.state().sel.combine, Combine::Add);
    h.get(By::new().role(egui::Role::Button).label("New Area")).click();
    h.run();
    h.state_mut().select_mode = SelectMode::Rectangle;
    crate::selection_tools::set_rings(h.state_mut(), Vec::new());
    let c = canvas(&h).center();
    drag(&mut h, c - vec2(100.0, 60.0), c - vec2(20.0, 0.0), Modifiers::NONE);
    let one = count(&h);
    assert!(one > 0);
    drag(&mut h, c + vec2(20.0, 0.0), c + vec2(100.0, 60.0), Modifiers::SHIFT);
    assert!(count(&h) > one + one / 2, "Shift added a second area");
    assert_eq!(crate::selection_tools::all_rings(h.state()).len(), 2);
}

#[test]
fn ellipse_wand_invert_grow_shrink() {
    let mut h = harness();
    h.get_by_label("Ellipse").click();
    h.run();
    assert_eq!(h.state().select_mode, SelectMode::Ellipse);
    let c = canvas(&h).center();
    drag(&mut h, c - vec2(80.0, 80.0), c + vec2(80.0, 80.0), Modifiers::NONE);
    let e = count(&h);
    assert!(e > 0);
    let poly = h.state().selection.clone().unwrap_or_default();
    assert!(poly.len() >= 40, "an ellipse outline, not a box");
    // grow, shrink, invert
    let before = count(&h);
    h.get_by_label("Grow").click();
    h.run();
    assert!(count(&h) > before);
    h.get_by_label("Shrink").click();
    h.run();
    h.get_by_label("Shrink").click();
    h.run();
    assert!(count(&h) < before);
    let small = count(&h);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::I);
    h.run();
    let (w, hh) = (h.state().engine.project.width() as usize, h.state().engine.project.height() as usize);
    assert_eq!(count(&h), w * hh - small, "Ctrl+Shift+I inverts");
    // the magic wand picks a painted area
    h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [200, 30, 30, 255], options: Default::default(), reference: Default::default() });
    h.get_by_label("Magic Wand").click();
    h.run();
    h.get(By::new().role(egui::Role::Slider).label("Tolerance"));
    crate::selection_tools::wand(h.state_mut(), 10.0, 10.0, Combine::Replace);
    assert_eq!(count(&h), w * hh, "the fill covers the drawing");
}

#[test]
fn skew_distort_resample_and_rings_follow_a_move() {
    let mut h = harness();
    h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [20, 20, 20, 255], options: Default::default(), reference: Default::default() });
    {
        let app = h.state_mut();
        crate::selection_tools::set_rings(app, vec![sq(100.0, 100.0, 300.0, 300.0)]);
        crate::selection_tools::add_ring(app, sq(150.0, 150.0, 250.0, 250.0), Combine::Subtract);
    }
    h.run();
    h.get(By::new().role(egui::Role::ComboBox).label("Resampling"));
    // a move carries the hole along
    h.state_mut().transform_selection(10.0, 0.0, (1.0, 1.0), 0.0);
    let rings = crate::selection_tools::all_rings(h.state());
    assert_eq!(rings.len(), 2);
    assert!(rings[1].iter().any(|p| (p.0 - 160.0).abs() < 0.01), "the hole moved by 10");
    // skew
    h.state_mut().sel.skew = (20.0, 0.0);
    h.get_by_label("Apply Skew").click();
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Transform"));
    // distort: handles, then apply
    h.get_by_label("Distort").click();
    h.run();
    assert!(h.state().sel.distort.is_some());
    if let Some(q) = h.state_mut().sel.distort.as_mut() {
        q[1].0 -= 40.0;
    }
    h.get_by_label("Apply Distort").click();
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Distort"));
    assert!(h.state().sel.distort.is_none());
}

#[test]
fn typed_box_moves_and_scales_the_selection_and_the_wand_can_go_global() {
    let mut h = harness();
    h.state_mut().run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [20, 20, 20, 255], options: Default::default(), reference: Default::default() });
    crate::selection_tools::set_rings(h.state_mut(), vec![sq(100.0, 100.0, 300.0, 200.0)]);
    h.run();
    assert_eq!(h.state().sel.typed, [100.0, 100.0, 200.0, 100.0, 0.0], "the fields read the box");
    // a new place and twice the width; proportions keep the height in step
    {
        let app = h.state_mut();
        app.sel.typed[0] = 400.0;
        app.sel.typed[1] = 50.0;
    }
    h.get(By::new().role(egui::Role::SpinButton).label("Width")).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get(By::new().role(egui::Role::SpinButton).label("Width")).type_text("400");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().sel.typed[3], 200.0, "keep proportions doubled the height too");
    h.get_by_label("Apply Box").click();
    h.run();
    assert_eq!(h.state().engine.undo_label(), Some("Transform"));
    let (x, y, w, hh) = crate::selection_tools::bbox(h.state().selection.as_ref().unwrap());
    assert!((x - 400.0).abs() < 0.5 && (y - 50.0).abs() < 0.5 && (w - 400.0).abs() < 0.5 && (hh - 200.0).abs() < 0.5, "{x} {y} {w} {hh}");
    // the wand: "Touching only" off selects that colour everywhere
    h.state_mut().select_mode = SelectMode::Wand;
    h.run();
    h.get(By::new().role(egui::Role::CheckBox).label("Touching only")).click();
    h.run();
    assert!(!h.state().sel.contiguous);
}

#[test]
fn feather_typed_in_the_options_softens_erase_and_copy() {
    let mut h = harness();
    {
        let app = h.state_mut();
        let (w, hh) = (app.engine.project.width(), app.engine.project.height());
        app.run(Command::Fill { layer: 0, frame: 0, x: 5, y: 5, color: [20, 40, 200, 255], options: Default::default(), reference: Default::default() });
        assert!(w > 400 && hh > 400);
        crate::selection_tools::set_rings(app, vec![sq(200.0, 200.0, 400.0, 400.0)]);
    }
    h.run();
    // type the feather into the Select tool's options, from the keyboard
    h.get(By::new().role(egui::Role::SpinButton).label("Feather")).scroll_to_me();
    h.run();
    h.get(By::new().role(egui::Role::SpinButton).label("Feather")).click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.get(By::new().role(egui::Role::SpinButton).label("Feather")).type_text("8");
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert_eq!(h.state().sel.feather, 8.0);
    // copy carries the soft edge; Delete leaves a fade instead of a hard cut
    h.state_mut().copy_pixels(false);
    let (rgba, cw, _, _) = h.state().pixel_clipboard.clone().unwrap();
    assert!(cw > 200, "the copied box includes the fade outside the outline");
    let alpha = |x: usize, y: usize| rgba[(y * cw as usize + x) * 4 + 3];
    assert!(alpha(0, cw as usize / 2) < 60, "the copy fades out at its edge");
    h.state_mut().delete_selected_pixels();
    h.run();
    let px = |h: &Harness<'static, ToonApp>, x: i64, y: i64| {
        let p = &h.state().engine.project;
        p.drawing_at(0, 0).and_then(|d| p.drawing(d)).map_or([0; 4], |s| s.pixel(x, y))
    };
    assert_eq!(px(&h, 300, 300)[3], 0, "the middle is erased");
    assert_eq!(px(&h, 150, 300)[3], 255, "far outside is kept");
    let edge = px(&h, 200, 300)[3];
    assert!(edge > 60 && edge < 200, "the edge keeps part of the paint, got {edge}");
    // one undo step brings it all back
    h.state_mut().engine.undo().unwrap();
    assert_eq!(px(&h, 300, 300)[3], 255);
}
