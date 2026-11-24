//! The Colour panel's palettes, hue wheel, harmonies and layer recolouring. Palettes are kept with
//! the shot (`Project::palettes`); "Basic" is the built-in one. Palette files: GIMP `.gpl` (open
//! and save) and Photoshop `.aco` (open), through `tooncraft-interop`.

use egui::{Color32, Mesh, Sense, Stroke, Ui, ecolor::Hsva, pos2, vec2};
use tooncraft_doc::palette::{Palette, Stop, Swatch, stop_colour};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Which palette the panel shows: 0 is the built-in Basic one, n is the shot's palette n − 1.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub current: usize,
    pub rename: Option<String>,
    pub harmony: Harmony,
    pub recolour_from: [u8; 3],
    pub recolour_tolerance: u8,
    pub wheel: bool,
    /// The floating picker window is open.
    pub window: bool,
    /// The swatch being edited: (shot palette index, swatch index, name, colour, update the paint).
    pub edit: Option<SwatchEdit>,
    /// The reference viewer window.
    pub viewer: crate::reference_viewer::Viewer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SwatchEdit {
    pub palette: usize,
    pub swatch: usize,
    pub name: String,
    pub rgb: [u8; 3],
    pub update_paint: bool,
}

impl State {
    pub fn new() -> Self {
        Self { recolour_from: [0, 0, 0], recolour_tolerance: 24, ..Self::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Harmony {
    #[default]
    Complementary,
    Analogous,
    Triadic,
    SplitComplementary,
    Tetradic,
}

fn harmony_name(h: Harmony) -> String {
    match h {
        Harmony::Complementary => tl!("Opposite"),
        Harmony::Analogous => tl!("Neighbours"),
        Harmony::Triadic => tl!("Three-way"),
        Harmony::SplitComplementary => tl!("Split opposite"),
        Harmony::Tetradic => tl!("Four-way"),
    }
    .to_string()
}

/// The hue turns (fractions of a full turn) of a harmony around a base colour.
pub fn harmony_turns(h: Harmony) -> &'static [f32] {
    match h {
        Harmony::Complementary => &[0.0, 0.5],
        Harmony::Analogous => &[-1.0 / 12.0, 0.0, 1.0 / 12.0],
        Harmony::Triadic => &[0.0, 1.0 / 3.0, 2.0 / 3.0],
        Harmony::SplitComplementary => &[0.0, 5.0 / 12.0, 7.0 / 12.0],
        Harmony::Tetradic => &[0.0, 0.25, 0.5, 0.75],
    }
}

/// The colours of harmony `h` around `rgb` (same saturation and brightness).
pub fn harmony(rgb: [u8; 3], h: Harmony) -> Vec<[u8; 3]> {
    let base = Hsva::from(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    harmony_turns(h)
        .iter()
        .map(|t| {
            let c = Color32::from(Hsva::new((base.h + t).rem_euclid(1.0), base.s, base.v, 1.0));
            [c.r(), c.g(), c.b()]
        })
        .collect()
}

fn basic(app: &ToonApp) -> Palette {
    let mut p = Palette::new(tl!("Basic"));
    for s in &app.swatches {
        p.push("", *s);
    }
    p
}

/// The palette shown (a copy).
pub fn shown(app: &ToonApp) -> Palette {
    match app.colour.current.checked_sub(1).and_then(|i| app.engine.project.palettes.get(i)) {
        Some(p) => p.clone(),
        None => basic(app),
    }
}

/// Save the shot's palettes with `f` applied to them.
fn edit(app: &mut ToonApp, f: impl FnOnce(&mut Vec<Palette>)) {
    let mut v = app.engine.project.palettes.clone();
    f(&mut v);
    app.run(Command::SetPalettes { palettes: v });
}

/// Add a colour to the palette shown; on Basic, a new shot palette is made for it first.
pub fn add_colour(app: &mut ToonApp, rgb: [u8; 3]) {
    add_rgba(app, [rgb[0], rgb[1], rgb[2], 255]);
}

/// Add a colour with its opacity to the palette shown (on Basic, a new shot palette first).
pub fn add_rgba(app: &mut ToonApp, rgba: [u8; 4]) {
    add_to_shown(app, |p| p.push_rgba("", rgba));
}

/// Add a gradient swatch to the palette shown (on Basic, a new shot palette first).
pub fn add_gradient(app: &mut ToonApp, stops: Vec<Stop>) {
    if add_to_shown(app, |p| p.push_gradient("", stops)) {
        app.set_status(tl!("Gradient added to the palette"));
    }
}

/// Run `f` on the palette shown and keep the change when it says it made one; Basic gets a new
/// shot palette first. False when nothing was added.
fn add_to_shown(app: &mut ToonApp, f: impl FnOnce(&mut Palette) -> bool) -> bool {
    if app.colour.current == 0 {
        if app.engine.project.palettes.len() >= tooncraft_doc::palette::MAX_PALETTES {
            app.set_status(tl!("The shot has as many palettes as it can hold"));
            return false;
        }
        let name = tl!("Shot colours").to_string();
        edit(app, |v| v.push(Palette::new(&name)));
        app.colour.current = app.engine.project.palettes.len();
    }
    let i = app.colour.current - 1;
    let mut v = app.engine.project.palettes.clone();
    let added = v.get_mut(i).is_some_and(f);
    if added {
        app.run(Command::SetPalettes { palettes: v });
    } else {
        app.set_status(tl!("Could not add to the palette: it is full"));
    }
    added
}

fn swatch_button(ui: &mut Ui, swatch: &Swatch, label: String) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
    if swatch.is_gradient() {
        paint_stops(ui.painter(), rect, &swatch.gradient);
    } else {
        checker(ui.painter(), rect);
        let [r, g, b, a] = swatch.rgba();
        ui.painter().rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(r, g, b, a));
    }
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, Color32::from_gray(120)), egui::StrokeKind::Inside);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
    resp
}

/// A light and dark checkerboard, so see-through colours show as such.
fn checker(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 3.0, Color32::from_gray(230));
    // square cells, a quarter of the short side, clipped to the rectangle
    let s = (rect.width().min(rect.height()) / 4.0).max(2.0);
    let (cols, rows) = ((rect.width() / s).ceil() as u32, (rect.height() / s).ceil() as u32);
    for y in 0..rows.min(64) {
        for x in 0..cols.min(256) {
            if (x + y) % 2 == 1 {
                let cell = egui::Rect::from_min_size(rect.min + vec2(x as f32 * s, y as f32 * s), vec2(s, s)).intersect(rect);
                painter.rect_filled(cell, 0.0, Color32::from_gray(160));
            }
        }
    }
}

/// `stops` left to right across `rect`, over a checkerboard.
fn paint_stops(painter: &egui::Painter, rect: egui::Rect, stops: &[Stop]) {
    checker(painter, rect);
    let mut mesh = Mesh::default();
    let colour = |t: f32| {
        let [r, g, b, a] = stop_colour(stops, t);
        Color32::from_rgba_unmultiplied(r, g, b, a)
    };
    // the stops plus both ends, in order
    let mut at: Vec<f32> = std::iter::once(0.0).chain(stops.iter().map(|s| s.at)).chain(std::iter::once(1.0)).collect();
    at.sort_by(f32::total_cmp);
    for t in at {
        let x = rect.left() + rect.width() * t;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(x, rect.top()), colour(t));
        mesh.colored_vertex(pos2(x, rect.bottom()), colour(t));
        if base >= 2 {
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base, base + 1);
        }
    }
    painter.add(mesh);
}

/// A gradient preview strip with an accessible name.
pub fn gradient_strip(ui: &mut Ui, stops: &[Stop], size: egui::Vec2, label: String) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    paint_stops(ui.painter(), rect, stops);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label.clone()));
    resp
}

/// What a swatch is called for screen readers.
fn swatch_label(s: &Swatch) -> String {
    if s.is_gradient() {
        return if s.name.is_empty() {
            tlf!("Gradient swatch, {n} stops", n = s.gradient.len())
        } else {
            tlf!("Gradient swatch {name}, {n} stops", name = s.name, n = s.gradient.len())
        }
        .to_string();
    }
    let hex = hex3(s.rgb);
    let base = if s.name.is_empty() { tlf!("Swatch #{hex}", hex = hex) } else { tlf!("Swatch {name}, #{hex}", name = s.name, hex = hex) }.to_string();
    if s.alpha == 255 {
        base
    } else {
        let pct = (s.alpha as f32 / 255.0 * 100.0).round() as u32;
        tlf!("{swatch}, {pct} % opaque", swatch = base, pct = pct).to_string()
    }
}

/// Use a swatch: a gradient becomes the gradient tool's stops and the tool is chosen; a
/// see-through colour brings its opacity; an opaque one keeps the current opacity.
pub fn use_swatch(app: &mut ToonApp, s: &Swatch) {
    if s.is_gradient() {
        app.aids.gradient_stops = s.gradient.clone();
        app.tool = crate::app::Tool::Gradient;
        app.set_status(tl!("Gradient tool: drag to paint the swatch's gradient"));
    } else if s.alpha < 255 {
        app.color = s.rgba();
    } else {
        set_rgb(app, s.rgb);
    }
}

fn hex3(c: [u8; 3]) -> String {
    format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// The palettes section: choose, show, add, name, delete, open and save files.
pub fn palettes(app: &mut ToonApp, ui: &mut Ui) {
    let names: Vec<String> = std::iter::once(tl!("Basic").to_string()).chain(app.engine.project.palettes.iter().map(|p| p.name.clone())).collect();
    app.colour.current = app.colour.current.min(names.len() - 1);
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("Palette"));
        egui::ComboBox::from_id_salt("palette-choice")
            .selected_text(names[app.colour.current].clone())
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut app.colour.current, i, n);
                }
            })
            .response
            .labelled_by(l.id);
        if ui.button(tl!("New Palette")).clicked() && app.engine.project.palettes.len() < tooncraft_doc::palette::MAX_PALETTES {
            let name = tlf!("Palette {n}", n = app.engine.project.palettes.len() + 1);
            edit(app, |v| v.push(Palette::new(&name)));
            app.colour.current = app.engine.project.palettes.len();
        }
        let own = app.colour.current > 0;
        if ui.add_enabled(own, egui::Button::new(tl!("Rename"))).clicked() {
            app.colour.rename = Some(names[app.colour.current].clone());
        }
        if ui.add_enabled(own, egui::Button::new(tl!("Delete Palette"))).clicked() {
            let i = app.colour.current - 1;
            edit(app, |v| {
                if i < v.len() {
                    v.remove(i);
                }
            });
            app.colour.current = 0;
        }
        if ui.button(tl!("Open Palette…")).on_hover_text(tl!("GIMP palette (.gpl) or Photoshop swatches (.aco)")).clicked()
            && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Palettes"), &["gpl", "aco"]).pick_file()
        {
            open_file(app, &f);
        }
        if ui.button(tl!("Save Palette…")).on_hover_text(tl!("As a GIMP palette (.gpl)")).clicked() {
            let p = shown(app);
            if let Some(f) = rfd::FileDialog::new().add_filter("GPL", &["gpl"]).set_file_name(format!("{}.gpl", p.name)).save_file() {
                save_file(app, &p, &f);
            }
        }
    });
    if let Some(mut n) = app.colour.rename.take() {
        let mut keep = true;
        ui.horizontal(|ui| {
            let l = ui.label(tl!("Name"));
            let r = ui.add(egui::TextEdit::singleline(&mut n).desired_width(160.0)).labelled_by(l.id);
            r.request_focus();
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) || ui.button(tl!("OK")).clicked() {
                let i = app.colour.current.saturating_sub(1);
                let name = n.clone();
                edit(app, |v| {
                    if let Some(p) = v.get_mut(i) {
                        p.name = name;
                    }
                });
                keep = false;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                keep = false;
            }
        });
        if keep {
            app.colour.rename = Some(n);
        }
    }
    let p = shown(app);
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for (i, s) in p.swatches.iter().enumerate() {
            let resp = swatch_button(ui, s, swatch_label(s));
            let resp = if s.name.is_empty() { resp } else { resp.on_hover_text(&s.name) };
            if resp.clicked() {
                chosen = Some(s.clone());
            }
            // right-click, or the Menu key / Shift+F10 on the focused swatch
            let (index, rgb) = (i, s.rgb);
            let launch = if i == 0 { "swatch" } else { "" };
            crate::context_menus::attach(ui, &resp, launch, |ui| crate::context_menus::swatch(app, ui, index, rgb));
        }
    });
    if let Some(s) = chosen {
        use_swatch(app, &s);
    }
    if ui.button(tl!("Add Current Colour")).on_hover_text(tl!("With its opacity, to the palette shown (on Basic, a new shot palette)")).clicked() {
        let c = app.color;
        add_rgba(app, c);
    }
    swatch_editor(app, ui);
}

/// Set the current colour, keeping its opacity.
pub fn set_rgb(app: &mut ToonApp, rgb: [u8; 3]) {
    app.color = [rgb[0], rgb[1], rgb[2], app.color[3]];
}

/// The current colour's own opacity (brushes and fills paint with it, times the brush opacity).
pub fn opacity(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Colour opacity"));
        let mut pct = (app.color[3] as f32 / 255.0 * 100.0).round();
        if ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).suffix(" %").max_decimals(0)).labelled_by(l.id).changed() {
            app.color[3] = (pct.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8;
        }
    });
}

/// The swatch editor under the palette: name, colour, and whether the paint in the old colour
/// follows. Apply runs `EditSwatch` (one undo step).
fn swatch_editor(app: &mut ToonApp, ui: &mut Ui) {
    let Some(mut ed) = app.colour.edit.take() else { return };
    let mut keep = true;
    ui.group(|ui| {
        ui.label(tl!("Edit swatch"));
        ui.horizontal(|ui| {
            let l = ui.label(tl!("Swatch name"));
            ui.add(egui::TextEdit::singleline(&mut ed.name).desired_width(160.0)).labelled_by(l.id);
        });
        crate::color::picker(ui, "swatch-edit", &mut ed.rgb);
        ui.checkbox(&mut ed.update_paint, tl!("Update paint in this colour"))
            .on_hover_text(tl!("Every unlocked layer: paint in the old colour takes the new one"));
        ui.horizontal(|ui| {
            if ui.button(tl!("Apply")).clicked() {
                let cmd =
                    Command::EditSwatch { palette: ed.palette, swatch: ed.swatch, rgb: ed.rgb, name: Some(ed.name.clone()), update_paint: ed.update_paint };
                if app.run(cmd) {
                    app.set_status(tl!("Swatch changed"));
                }
                keep = false;
            }
            if ui.button(tl!("Cancel")).clicked() {
                keep = false;
            }
        });
    });
    if keep {
        app.colour.edit = Some(ed);
    }
}

/// The floating colour picker: the picker, the colour's opacity and the palette shown.
pub fn picker_window(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.colour.window {
        return;
    }
    let mut open = true;
    egui::Window::new(tl!("Colour Picker")).id(egui::Id::unique("colour-picker-window")).open(&mut open).resizable(false).default_width(260.0).show(
        ctx,
        |ui| {
            let mut rgb = [app.color[0], app.color[1], app.color[2]];
            if crate::color::picker(ui, "window-colour", &mut rgb) {
                set_rgb(app, rgb);
            }
            opacity(app, ui);
            ui.separator();
            let p = shown(app);
            ui.label(tlf!("Palette: {name}", name = p.name));
            let mut chosen = None;
            ui.horizontal_wrapped(|ui| {
                for s in &p.swatches {
                    if swatch_button(ui, s, swatch_label(s)).clicked() {
                        chosen = Some(s.clone());
                    }
                }
            });
            if let Some(s) = chosen {
                use_swatch(app, &s);
            }
        },
    );
    if !open {
        app.colour.window = false;
    }
}

/// Add the palette in `path` to the shot and show it.
pub fn open_file(app: &mut ToonApp, path: &std::path::Path) {
    match tooncraft_interop::palette::read_file(path) {
        Ok(p) => {
            let n = p.swatches.len();
            let name = p.name.clone();
            if app.engine.project.palettes.len() >= tooncraft_doc::palette::MAX_PALETTES {
                app.set_status(tl!("The shot has as many palettes as it can hold"));
                return;
            }
            edit(app, |v| v.push(p));
            app.colour.current = app.engine.project.palettes.len();
            app.set_status(tlf!("Palette {name} added ({n} colours)", name = name, n = n));
        }
        Err(e) => app.set_status(tlf!("Could not open the palette: {error}", error = e)),
    }
}

pub fn save_file(app: &mut ToonApp, p: &Palette, path: &std::path::Path) {
    let text = tooncraft_interop::palette::to_gpl(p);
    match craft_atomic::atomic_write(path, text.as_bytes()) {
        Ok(()) => app.set_status(tlf!("Saved {file}", file = path.display())),
        Err(e) => app.set_status(tlf!("Could not save the palette: {error}", error = e)),
    }
}

/// A hue ring: click or drag around it to turn the hue (saturation and brightness stay).
pub fn wheel(ui: &mut Ui, rgb: &mut [u8; 3]) -> bool {
    let size = ui.available_width().clamp(120.0, 200.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click_and_drag());
    let c = rect.center();
    let (r1, r0) = (size * 0.5, size * 0.5 - 16.0);
    let mut mesh = Mesh::default();
    let n = 72;
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let a = t * std::f32::consts::TAU;
        let col = Color32::from(Hsva::new(t, 1.0, 1.0, 1.0));
        mesh.colored_vertex(pos2(c.x + a.cos() * r1, c.y + a.sin() * r1), col);
        mesh.colored_vertex(pos2(c.x + a.cos() * r0, c.y + a.sin() * r0), col);
        if i > 0 {
            let k = (i * 2) as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    ui.painter().add(mesh);
    let mut hsv = Hsva::from(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    let a = hsv.h * std::f32::consts::TAU;
    let rm = (r0 + r1) * 0.5;
    ui.painter().circle_stroke(pos2(c.x + a.cos() * rm, c.y + a.sin() * rm), 6.0, Stroke::new(2.0, Color32::WHITE));
    ui.painter().circle_filled(c, r0 * 0.55, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    let deg = (hsv.h * 360.0).round();
    resp.widget_info(|| egui::WidgetInfo::slider(true, deg as f64, tl!("Hue wheel")));
    if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let t = ((p.y - c.y).atan2(p.x - c.x) / std::f32::consts::TAU).rem_euclid(1.0);
        hsv.h = t;
        if hsv.s < 0.01 {
            hsv.s = 1.0;
        }
        if hsv.v < 0.01 {
            hsv.v = 1.0;
        }
        let col = Color32::from(hsv);
        let out = [col.r(), col.g(), col.b()];
        if out != *rgb {
            *rgb = out;
            return true;
        }
    }
    false
}

/// The harmony row: colours that go with the current one; a click takes one.
pub fn harmonies(app: &mut ToonApp, ui: &mut Ui) {
    ui.horizontal_wrapped(|ui| {
        let l = ui.label(tl!("Harmony"));
        egui::ComboBox::from_id_salt("colour-harmony")
            .selected_text(harmony_name(app.colour.harmony))
            .show_ui(ui, |ui| {
                for h in [Harmony::Complementary, Harmony::Analogous, Harmony::Triadic, Harmony::SplitComplementary, Harmony::Tetradic] {
                    ui.selectable_value(&mut app.colour.harmony, h, harmony_name(h));
                }
            })
            .response
            .labelled_by(l.id);
        let base = [app.color[0], app.color[1], app.color[2]];
        let colours = harmony(base, app.colour.harmony);
        let mut chosen = None;
        for c in &colours {
            if swatch_button(ui, &Swatch::colour("", *c), tlf!("Harmony colour #{hex}", hex = hex3(*c)).to_string()).clicked() {
                chosen = Some(*c);
            }
        }
        if ui.button(tl!("Add All to Palette")).clicked() {
            for c in &colours {
                add_colour(app, *c);
            }
        }
        if let Some(c) = chosen {
            set_rgb(app, [c[0], c[1], c[2]]);
        }
    });
}

/// Recolour the layer: one colour becomes the current colour in every drawing.
pub fn recolour(app: &mut ToonApp, ui: &mut Ui) {
    egui::CollapsingHeader::new(tl!("Recolour Layer")).default_open(false).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            crate::color::swatch_edit(ui, "recolour-from", tl!("Colour to replace"), &mut app.colour.recolour_from);
            ui.label(tl!("becomes the current colour"));
            let l = ui.label(tl!("Tolerance"));
            ui.add(egui::DragValue::new(&mut app.colour.recolour_tolerance).range(0..=255)).labelled_by(l.id);
        });
        if ui.button(tl!("Recolour Every Drawing of the Layer")).on_hover_text(tl!("Antialiased edges keep their softness; one undo step")).clicked() {
            let (layer, from, tolerance) = (app.layer, app.colour.recolour_from, app.colour.recolour_tolerance);
            let to = [app.color[0], app.color[1], app.color[2]];
            let before = app.engine.undo_len();
            if app.run(Command::RecolourLayer { layer, from, to, tolerance }) {
                let s = if app.engine.undo_len() > before { tl!("Recoloured") } else { tl!("No pixels of that colour on the layer") };
                app.set_status(s);
            }
        }
    });
}
