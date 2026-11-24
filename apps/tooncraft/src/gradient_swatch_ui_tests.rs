//! UI tests for colour swatches with an opacity and gradient swatches: choosing one from the
//! palette, painting a gradient swatch with the gradient tool, the stop editor and adding to the
//! palette. The accessibility check is on.

use egui::{Key, Pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::palette::{Palette, Stop};
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

fn harness() -> Harness<'static, ToonApp> {
    let ctx = egui::Context::default();
    let app = ToonApp::with_context(&ctx, Cli::default()).without_autosave();
    let mut h = Harness::builder().with_size(vec2(1700.0, 1300.0)).with_accessibility_check(true).build_ui_state(
        |ui, app: &mut ToonApp| {
            let ctx = ui.ctx().clone();
            app.tick(&ctx);
            app.draw(ui);
        },
        app,
    );
    h.state_mut().workspace.raise(crate::dock::Pane::Colour);
    h.run();
    h.run();
    h
}

fn click(h: &mut Harness<'static, ToonApp>, by: By<'_>) {
    h.get(by.clone()).scroll_to_me();
    h.run();
    h.get(by).click();
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

fn pixel(h: &Harness<'static, ToonApp>, x: i64, y: i64) -> [u8; 4] {
    let p = &h.state().engine.project;
    p.drawing_at(0, 0).and_then(|d| p.drawing(d)).map(|s| s.pixel(x, y)).unwrap_or([0; 4])
}

#[test]
fn a_see_through_swatch_sets_the_opacity_and_a_gradient_swatch_paints_its_stops() {
    let mut h = harness();
    let mut p = Palette::new("Sky");
    assert!(p.push_rgba("haze", [40, 80, 200, 128]));
    let stops = vec![Stop { at: 0.0, rgba: RED }, Stop { at: 0.5, rgba: GREEN }, Stop { at: 1.0, rgba: BLUE }];
    assert!(p.push_gradient("dawn", stops.clone()));
    let app = h.state_mut();
    assert!(app.run(Command::SetPalettes { palettes: vec![p] }));
    app.colour.current = 1;
    h.run();
    click(&mut h, By::new().role(egui::Role::Button).label("Swatch haze, #2850C8, 50 % opaque"));
    assert_eq!(h.state().color, [40, 80, 200, 128], "the swatch's opacity comes with it");
    click(&mut h, By::new().role(egui::Role::Button).label("Gradient swatch dawn, 3 stops"));
    assert_eq!(h.state().tool, Tool::Gradient, "a gradient swatch chooses the gradient tool");
    assert_eq!(h.state().aids.gradient_stops, stops);
    h.get(By::new().role(egui::Role::Image).label("Gradient preview"));
    let canvas = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let c = canvas.center();
    drag_through(&mut h, &[c - vec2(150.0, 0.0), c, c + vec2(150.0, 0.0)]);
    assert_eq!(h.state().engine.undo_label(), Some("Gradient"));
    let (w, y) = (h.state().engine.project.width() as i64, h.state().engine.project.height() as i64 / 2);
    let (left, right) = (pixel(&h, 0, y), pixel(&h, w - 1, y));
    assert!(left[0] > 200 && left[2] < 40, "the start is red: {left:?}");
    assert!(right[2] > 200 && right[0] < 40, "the end is blue: {right:?}");
    let green = (0..w).map(|x| pixel(&h, x, y)).any(|p| p[1] > 200 && p[0] < 40 && p[2] < 40);
    assert!(green, "the middle stop is painted");
}

#[test]
fn the_stop_editor_builds_a_gradient_and_adds_it_to_the_palette() {
    let mut h = harness();
    h.state_mut().color = [10, 20, 30, 200];
    h.run();
    click(&mut h, By::new().role(egui::Role::Button).label("Add Current Colour"));
    let pal = h.state().engine.project.palettes.clone();
    assert_eq!(pal.len(), 1, "on Basic a shot palette is made");
    assert_eq!(pal[0].swatches[0].rgba(), [10, 20, 30, 200], "the colour keeps its opacity");
    h.key_press(Key::G);
    h.run();
    assert_eq!(h.state().tool, Tool::Gradient);
    click(&mut h, By::new().role(egui::Role::Button).label("Edit Stops"));
    assert_eq!(h.state().aids.gradient_stops.len(), 2, "the two-colour gradient becomes two stops");
    click(&mut h, By::new().role(egui::Role::Button).label("Add Stop"));
    assert_eq!(h.state().aids.gradient_stops.len(), 3);
    assert_eq!(h.state().aids.gradient_stops[1], Stop { at: 0.5, rgba: [10, 20, 30, 200] }, "the new stop sits half way");
    click(&mut h, By::new().role(egui::Role::Button).label("Add Gradient to Palette"));
    let s = &h.state().engine.project.palettes[0].swatches[1];
    assert!(s.is_gradient() && s.gradient.len() == 3, "{s:?}");
    h.get(By::new().role(egui::Role::Button).label("Gradient swatch, 3 stops"));
    click(&mut h, By::new().role(egui::Role::Button).label("Two Colours"));
    assert!(h.state().aids.gradient_stops.is_empty());
    h.get(By::new().role(egui::Role::CheckBox).label("Fade to transparent"));
}
