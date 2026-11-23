//! Canvas tools: the field guide and safe areas over the canvas, the Canvas Size dialog (resize
//! with an anchor, or crop to the camera frame) and saved workspaces (named panel layouts, kept
//! per user next to the shortcuts).

use std::path::PathBuf;

use craft_ui::docking::Layout;
use egui::{Color32, Pos2, Stroke, vec2};
use tooncraft_doc::guides;
use tooncraft_engine::Command;

use crate::app::ToonApp;
use crate::dock::{Pane, Workspace};

/// Most saved workspaces.
const MAX_WORKSPACES: usize = 64;

pub struct CanvasUi {
    pub field_guide: bool,
    pub safe_areas: bool,
    /// The Canvas Size dialog: width, height, anchor (column, row: 0 … 2).
    pub resize: Option<(u32, u32, (u8, u8))>,
    /// Saved workspaces by name.
    pub workspaces: Vec<(String, Layout<Pane>)>,
    pub new_workspace: String,
    /// Where workspaces are kept (`None`: not saved, as in tests).
    store: Option<PathBuf>,
}

impl CanvasUi {
    /// With `persist`, workspaces are read from and written to the user's config folder.
    pub fn new(persist: bool) -> Self {
        let store = if persist { crate::keys::overrides_path().and_then(|p| p.parent().map(|d| d.join("workspaces.json"))) } else { None };
        let workspaces = store.as_ref().and_then(|p| std::fs::read(p).ok()).map(|b| parse(&b)).unwrap_or_default();
        Self { field_guide: false, safe_areas: false, resize: None, workspaces, new_workspace: String::new(), store }
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.store else { return Ok(()) };
        let map: serde_json::Map<String, serde_json::Value> =
            self.workspaces.iter().filter_map(|(n, l)| serde_json::to_value(l).ok().map(|v| (n.clone(), v))).collect();
        let bytes = serde_json::to_vec_pretty(&map).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        craft_atomic::atomic_write(path, &bytes).map_err(|e| e.to_string())
    }

    /// Save the current layout under `name` (replacing one of that name).
    pub fn save_workspace(&mut self, name: &str, layout: &Layout<Pane>) -> Result<(), String> {
        let name: String = name.trim().chars().take(80).collect();
        if name.is_empty() {
            return Err(tl!("Give the workspace a name").to_string());
        }
        let full = self.workspaces.len() >= MAX_WORKSPACES;
        match self.workspaces.iter_mut().find(|(n, _)| *n == name) {
            Some(w) => w.1 = layout.clone(),
            None if !full => self.workspaces.push((name, layout.clone())),
            None => return Err(tl!("Too many saved workspaces").to_string()),
        }
        self.save()
    }

    pub fn delete_workspace(&mut self, name: &str) -> Result<(), String> {
        self.workspaces.retain(|(n, _)| n != name);
        self.save()
    }
}

/// Saved workspaces from a file; anything unreadable is skipped (the file is not trusted).
fn parse(bytes: &[u8]) -> Vec<(String, Layout<Pane>)> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(bytes) else { return Vec::new() };
    map.into_iter()
        .take(MAX_WORKSPACES)
        .filter_map(|(name, v)| serde_json::from_value::<Layout<Pane>>(v).ok().map(|l| (name.chars().take(80).collect(), l)))
        .collect()
}

/// A layout with a tall timeline and the timing panels in front, for timing work.
fn timing_layout() -> Layout<Pane> {
    use craft_ui::docking::Node;
    use craft_ui::layout::{SplitAxis, SplitSize};
    let tabs = |p: &[Pane]| Node::Tabs { panels: p.to_vec(), active: 0 };
    let centre = Node::Split {
        axis: SplitAxis::Vertical,
        size: SplitSize::Ratio(0.55),
        first: Box::new(tabs(&[Pane::Canvas])),
        second: Box::new(tabs(&[Pane::Timeline])),
    };
    let right = Node::Split {
        axis: SplitAxis::Vertical,
        size: SplitSize::Ratio(0.45),
        first: Box::new(Node::Tabs { panels: vec![Pane::Timing, Pane::Pegs, Pane::Camera], active: 0 }),
        second: Box::new(tabs(&[Pane::Layers, Pane::Onion, Pane::Effects, Pane::Colour, Pane::Brush])),
    };
    let root = Node::Split { axis: SplitAxis::Horizontal, size: SplitSize::FixedSecond(320.0), first: Box::new(centre), second: Box::new(right) };
    Layout { root: Some(root), floating: Vec::new() }
}

/// Ink and paint: a big canvas, colour and layers on the right, a short timeline.
fn ink_paint_layout() -> Layout<Pane> {
    use craft_ui::docking::Node;
    use craft_ui::layout::{SplitAxis, SplitSize};
    let tabs = |p: &[Pane]| Node::Tabs { panels: p.to_vec(), active: 0 };
    let centre = Node::Split {
        axis: SplitAxis::Vertical,
        size: SplitSize::FixedSecond(150.0),
        first: Box::new(tabs(&[Pane::Canvas])),
        second: Box::new(tabs(&[Pane::Timeline])),
    };
    let right = Node::Split {
        axis: SplitAxis::Vertical,
        size: SplitSize::Ratio(0.5),
        first: Box::new(tabs(&[Pane::Colour])),
        second: Box::new(tabs(&[Pane::Layers, Pane::Brush, Pane::Onion])),
    };
    let root = Node::Split { axis: SplitAxis::Horizontal, size: SplitSize::FixedSecond(300.0), first: Box::new(centre), second: Box::new(right) };
    Layout { root: Some(root), floating: Vec::new() }
}

/// The exposure sheet down the right side, as on paper, next to the canvas.
fn sheet_layout() -> Layout<Pane> {
    use craft_ui::docking::Node;
    use craft_ui::layout::{SplitAxis, SplitSize};
    let tabs = |p: &[Pane]| Node::Tabs { panels: p.to_vec(), active: 0 };
    let right = Node::Split {
        axis: SplitAxis::Vertical,
        size: SplitSize::Ratio(0.7),
        first: Box::new(tabs(&[Pane::Timeline])),
        second: Box::new(tabs(&[Pane::Layers, Pane::Colour, Pane::Brush, Pane::Onion])),
    };
    let root =
        Node::Split { axis: SplitAxis::Horizontal, size: SplitSize::FixedSecond(420.0), first: Box::new(tabs(&[Pane::Canvas])), second: Box::new(right) };
    Layout { root: Some(root), floating: Vec::new() }
}

/// View menu entries (guides, canvas size, workspaces).
pub fn view_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.checkbox(&mut app.canvas_ui.field_guide, tl!("Field Guide"));
    ui.checkbox(&mut app.canvas_ui.safe_areas, tl!("Safe Areas"));
    if ui.button(tl!("Canvas Size…")).clicked() {
        app.canvas_ui.resize = Some((app.engine.project.width(), app.engine.project.height(), (1, 1)));
    }
    ui.menu_button(tl!("Workspaces"), |ui| {
        if ui.button(tl!("Drawing (standard)")).clicked() {
            app.workspace.layout = Workspace::default().layout;
        }
        if ui.button(tl!("Timing")).clicked() {
            app.workspace.layout = timing_layout();
        }
        if ui.button(tl!("Ink and Paint")).clicked() {
            app.workspace.layout = ink_paint_layout();
        }
        if ui.button(tl!("Exposure Sheet")).on_hover_text(tl!("The sheet down the right side, layers as columns")).clicked() {
            app.workspace.layout = sheet_layout();
            app.tl.vertical = true;
        }
        let saved: Vec<String> = app.canvas_ui.workspaces.iter().map(|(n, _)| n.clone()).collect();
        if !saved.is_empty() {
            ui.separator();
        }
        for name in saved {
            ui.horizontal(|ui| {
                if ui.button(&name).clicked()
                    && let Some((_, l)) = app.canvas_ui.workspaces.iter().find(|(n, _)| *n == name)
                {
                    app.workspace.layout = l.clone();
                }
                if ui.small_button(tlf!("Delete {name}", name = name)).clicked()
                    && let Err(e) = app.canvas_ui.delete_workspace(&name)
                {
                    app.set_status(e);
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let l = ui.label(tl!("Name"));
            ui.add(egui::TextEdit::singleline(&mut app.canvas_ui.new_workspace).desired_width(110.0)).labelled_by(l.id);
            if ui.button(tl!("Save Workspace")).clicked() {
                let (name, layout) = (app.canvas_ui.new_workspace.clone(), app.workspace.layout.clone());
                match app.canvas_ui.save_workspace(&name, &layout) {
                    Ok(()) => {
                        app.set_status(tlf!("Saved workspace {name}", name = name.trim()));
                        app.canvas_ui.new_workspace.clear();
                    }
                    Err(e) => app.set_status(e),
                }
            }
        });
    });
}

fn anchor_name(col: u8, row: u8) -> String {
    match (col, row) {
        (0, 0) => tl!("Keep the top left in place"),
        (1, 0) => tl!("Keep the top in place"),
        (2, 0) => tl!("Keep the top right in place"),
        (0, 1) => tl!("Keep the left side in place"),
        (2, 1) => tl!("Keep the right side in place"),
        (0, 2) => tl!("Keep the bottom left in place"),
        (1, 2) => tl!("Keep the bottom in place"),
        (2, 2) => tl!("Keep the bottom right in place"),
        _ => tl!("Keep the centre in place"),
    }
    .to_string()
}

/// The Canvas Size dialog.
pub fn dialog(app: &mut ToonApp, ctx: &egui::Context) {
    let Some((mut w, mut h, mut anchor)) = app.canvas_ui.resize else { return };
    let mut open = true;
    let mut apply = None;
    egui::Window::new(tl!("Canvas Size")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
        egui::Grid::new("canvas-size").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
            let l = ui.label(tl!("Width"));
            ui.add(egui::DragValue::new(&mut w).range(16..=tooncraft_raster::MAX_SIDE).suffix(" px")).labelled_by(l.id);
            ui.end_row();
            let l = ui.label(tl!("Height"));
            ui.add(egui::DragValue::new(&mut h).range(16..=tooncraft_raster::MAX_SIDE).suffix(" px")).labelled_by(l.id);
            ui.end_row();
        });
        ui.label(tl!("Keep in place"));
        egui::Grid::new("canvas-anchor").show(ui, |ui| {
            for row in 0..3u8 {
                for col in 0..3u8 {
                    let name = anchor_name(col, row);
                    let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::click());
                    let on = anchor == (col, row);
                    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, name.clone()));
                    let c = if on { ui.visuals().selection.stroke.color } else { ui.visuals().weak_text_color() };
                    ui.painter().rect_stroke(r.shrink(2.0), 2.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
                    if on {
                        ui.painter().rect_filled(r.shrink(5.0), 1.0, c);
                    }
                    if resp.on_hover_text(&name).clicked() {
                        anchor = (col, row);
                    }
                }
                ui.end_row();
            }
        });
        ui.label(tl!("Drawings are moved, never resampled; paint outside the new canvas is cut (undo brings it back)."));
        ui.horizontal(|ui| {
            if ui.button(tl!("Resize")).clicked() {
                apply = Some(Command::ResizeCanvas { width: w, height: h, anchor: (anchor.0 as f32 / 2.0, anchor.1 as f32 / 2.0) });
            }
            let cam = &app.engine.project.camera;
            if ui
                .add_enabled(!cam.keys.is_empty(), egui::Button::new(tl!("Crop to Camera")))
                .on_hover_text(tl!("Crop to the camera frame at the current frame (no rotation)"))
                .clicked()
            {
                let (pw, ph) = (app.engine.project.width(), app.engine.project.height());
                let k = cam.at(app.frame, pw, ph);
                let z = k.zoom.max(0.05);
                let (cw, ch) = ((pw as f32 / z).round().max(16.0) as u32, (ph as f32 / z).round().max(16.0) as u32);
                apply =
                    Some(Command::CropCanvas { x: (k.x - cw as f32 / 2.0).round() as i64, y: (k.y - ch as f32 / 2.0).round() as i64, width: cw, height: ch });
            }
        });
    });
    app.canvas_ui.resize = if open && apply.is_none() { Some((w, h, anchor)) } else { None };
    if let Some(cmd) = apply
        && app.run(cmd)
    {
        app.view_fitted = false;
        app.textures.clear();
    }
}

/// The field guide and safe areas over the canvas. `screen` maps canvas pixels to the screen.
pub fn draw_guides(app: &ToonApp, painter: &egui::Painter, screen: impl Fn(f32, f32) -> Pos2) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let rect = |r: guides::Rect| vec![screen(r.0, r.1), screen(r.2, r.1), screen(r.2, r.3), screen(r.0, r.3)];
    if app.canvas_ui.field_guide {
        let faint = Color32::from_rgba_unmultiplied(70, 110, 200, 70);
        let (xs, ys) = guides::grid(w, h);
        for x in &xs {
            painter.line_segment([screen(*x, 0.0), screen(*x, h as f32)], Stroke::new(0.5, faint));
        }
        for y in &ys {
            painter.line_segment([screen(0.0, *y), screen(w as f32, *y)], Stroke::new(0.5, faint));
        }
        let strong = Color32::from_rgba_unmultiplied(70, 110, 200, 150);
        for (i, f) in guides::fields(w, h).into_iter().enumerate() {
            if (i + 1) % 2 == 0 {
                painter.add(egui::Shape::closed_line(rect(f), Stroke::new(1.0, strong)));
                let p = screen(f.0, f.1);
                painter.text(p + vec2(3.0, 2.0), egui::Align2::LEFT_TOP, format!("{}", i + 1), egui::FontId::proportional(10.0), strong);
            }
        }
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let arm = w.min(h) as f32 / 40.0;
        painter.line_segment([screen(cx - arm, cy), screen(cx + arm, cy)], Stroke::new(1.5, strong));
        painter.line_segment([screen(cx, cy - arm), screen(cx, cy + arm)], Stroke::new(1.5, strong));
    }
    if app.canvas_ui.safe_areas {
        let c = Color32::from_rgba_unmultiplied(220, 120, 40, 170);
        painter.add(egui::Shape::closed_line(rect(guides::centred(w, h, guides::ACTION_SAFE)), Stroke::new(1.0, c)));
        let mut title = rect(guides::centred(w, h, guides::TITLE_SAFE));
        if let Some(first) = title.first().copied() {
            title.push(first);
        }
        painter.add(egui::Shape::dashed_line(&title, Stroke::new(1.0, c), 6.0, 4.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_round_trip_and_hostile_files_are_skipped() {
        let dir = std::env::temp_dir().join(format!("tooncraft-ws-{}", std::process::id()));
        let mut ui = CanvasUi::new(false);
        ui.store = Some(dir.join("workspaces.json"));
        ui.save_workspace("Timing", &timing_layout()).unwrap();
        ui.save_workspace("Mine", &Workspace::default().layout).unwrap();
        assert!(ui.save_workspace("  ", &timing_layout()).is_err());
        let bytes = std::fs::read(dir.join("workspaces.json")).unwrap();
        let back = parse(&bytes);
        assert_eq!(back.len(), 2);
        assert_eq!(back.iter().find(|(n, _)| n == "Timing").map(|(_, l)| l), Some(&timing_layout()));
        assert!(parse(b"{\"x\": 3, \"y\": {\"root\": 7}}").is_empty());
        assert!(parse(b"\xff\x00garbage").is_empty());
        ui.delete_workspace("Mine").unwrap();
        assert_eq!(parse(&std::fs::read(dir.join("workspaces.json")).unwrap()).len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
