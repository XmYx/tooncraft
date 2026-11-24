//! UI tests for linked colour styles: a fill made with a colour chosen from a shot palette
//! swatch is linked to it, so editing the swatch recolours that fill and not other paint of the
//! same colour. The accessibility check is on.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use tooncraft_doc::palette::Palette;
use tooncraft_engine::Command;

use crate::app::{Tool, ToonApp};
use crate::args::Cli;

const SKIN: [u8; 3] = [250, 210, 190];

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
    h
}

fn rgb(h: &Harness<'static, ToonApp>, x: i64, y: i64) -> [u8; 4] {
    let app = h.state();
    let p = &app.engine.project;
    p.drawing_at(app.layer, app.frame).and_then(|d| p.drawing(d)).map(|s| tooncraft_raster::unpremultiply(s.pixel(x, y))).unwrap_or([0; 4])
}

#[test]
fn a_fill_with_a_swatch_colour_follows_the_swatch_and_other_paint_does_not() {
    let mut h = harness();
    let (layer, frame) = (h.state().layer, h.state().frame);
    let (w, ht) = (h.state().engine.project.width() as f32, h.state().engine.project.height() as f32);
    // a black wall down the middle makes two areas
    let black = [0, 0, 0, 255];
    let wall = vec![(w / 2.0 - 4.0, 0.0), (w / 2.0 + 4.0, 0.0), (w / 2.0 + 4.0, ht), (w / 2.0 - 4.0, ht)];
    assert!(h.state_mut().run(Command::Gradient {
        layer,
        frame,
        kind: Default::default(),
        from: (0.0, 0.0),
        to: (1.0, 0.0),
        start: black,
        end: black,
        area: Some(wall),
        area_rings: Vec::new(),
        feather: 0.0,
        stops: Vec::new()
    }));
    let mut p = Palette::new("Characters");
    p.push("skin", SKIN);
    assert!(h.state_mut().run(Command::SetPalettes { palettes: vec![p] }));
    h.state_mut().colour.current = 1;
    h.run();
    let swatch = By::new().role(egui::Role::Button).label("Swatch skin, #FAD2BE");
    h.get(swatch.clone()).scroll_to_me();
    h.run();
    h.get(swatch).click();
    h.run();
    assert_eq!(crate::colour_tools::linked_swatch(h.state()), Some((0, 0)), "the colour came from the swatch");
    // the paint bucket on the left half: linked
    h.state_mut().tool = Tool::Fill;
    h.run();
    let rect = h.get(By::new().role(egui::Role::Canvas).label_contains("Canvas")).rect();
    let size = vec2(w, ht);
    let at = h.state().view.to_screen(rect, size, egui::pos2(w / 4.0, ht / 2.0));
    h.hover_at(at);
    h.run();
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
    assert_eq!(rgb(&h, (w / 4.0) as i64, (ht / 2.0) as i64), [SKIN[0], SKIN[1], SKIN[2], 255]);
    let links = &h.state().engine.project.palettes[0].swatches[0].links;
    assert_eq!(links.len(), 1, "the fill is linked to the swatch");
    assert!(links[0].runs.len() <= ht as usize, "one run per row of the filled half: {}", links[0].runs.len());
    // the right half in the same colour, not from the swatch
    let c = [SKIN[0], SKIN[1], SKIN[2], 255];
    assert!(h.state_mut().run(Command::Fill {
        layer,
        frame,
        x: (w * 0.75) as u32,
        y: (ht / 2.0) as u32,
        color: c,
        options: Default::default(),
        reference: Default::default()
    }));
    // the swatch changes with "update paint": only the left half follows
    let new = [120, 60, 40];
    assert!(h.state_mut().run(Command::EditSwatch { palette: 0, swatch: 0, rgb: new, name: None, update_paint: true }));
    assert_eq!(rgb(&h, (w / 4.0) as i64, (ht / 2.0) as i64), [new[0], new[1], new[2], 255], "linked paint follows");
    assert_eq!(rgb(&h, (w * 0.75) as i64, (ht / 2.0) as i64), c, "unlinked paint of the same colour stays");
    // after choosing another colour, fills are no longer linked
    h.state_mut().color = [1, 2, 3, 255];
    assert_eq!(crate::colour_tools::linked_swatch(h.state()), None);
}
