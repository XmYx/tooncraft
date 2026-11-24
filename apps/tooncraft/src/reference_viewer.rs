//! The reference viewer: a floating window that shows a picture next to the canvas (not in the
//! shot) to look at while drawing, with zoom, pan and an eyedropper. A click on the picture, or
//! Enter with the keyboard cross, makes the colour under it the current colour.

use egui::{Color32, Rect, Sense, Stroke, Vec2, pos2, vec2};
use tooncraft_raster::RgbaImage;

use crate::app::ToonApp;

/// Largest picture side the viewer opens.
pub const MAX_SIDE: u32 = 8192;

#[derive(Clone, Default)]
pub struct Viewer {
    pub open: bool,
    /// The picture's name and pixels (premultiplied).
    pub image: Option<(String, RgbaImage)>,
    texture: Option<egui::TextureHandle>,
    /// 1: the picture fits the window.
    pub zoom: f32,
    pub pan: Vec2,
    /// The keyboard cross, in picture pixels.
    pub cursor: (u32, u32),
}

// by hand: the texture handle has no Debug
impl std::fmt::Debug for Viewer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viewer")
            .field("open", &self.open)
            .field("image", &self.image.as_ref().map(|(n, i)| (n, i.width, i.height)))
            .field("zoom", &self.zoom)
            .finish()
    }
}

/// Open the picture at `path` in the viewer (the window opens too).
pub fn load(app: &mut ToonApp, path: &std::path::Path) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match read(path) {
        Ok(img) => {
            let v = &mut app.colour.viewer;
            v.cursor = (img.width / 2, img.height / 2);
            v.image = Some((name.clone(), img));
            v.texture = None;
            v.zoom = 1.0;
            v.pan = Vec2::ZERO;
            v.open = true;
            app.set_status(tlf!("Reference {name} open: click it to pick a colour", name = name));
        }
        Err(e) => app.set_status(tlf!("Could not open the reference: {error}", error = e)),
    }
}

fn read(path: &std::path::Path) -> Result<RgbaImage, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > tooncraft_interop::MAX_IMPORT_FILE {
        return Err(tlf!("the file is larger than {mb} MB", mb = tooncraft_interop::MAX_IMPORT_FILE >> 20).to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    tooncraft_interop::decode_png(&bytes, MAX_SIDE).map_err(|e| e.to_string())
}

/// Make the colour at picture pixel (x, y) the current colour (its opacity stays).
pub fn pick(app: &mut ToonApp, x: u32, y: u32) {
    let Some((_, img)) = &app.colour.viewer.image else { return };
    if x >= img.width || y >= img.height {
        return;
    }
    let p = tooncraft_raster::unpremultiply(img.pixel(x, y));
    crate::colour_tools::set_rgb(app, [p[0], p[1], p[2]]);
    app.set_status(tlf!("Picked #{hex} from the reference", hex = format!("{:02X}{:02X}{:02X}", p[0], p[1], p[2])));
}

/// Where the picture sits in `area` at the viewer's zoom and pan.
fn placed(v: &Viewer, area: Rect, w: u32, h: u32) -> Rect {
    let fit = (area.width() / w.max(1) as f32).min(area.height() / h.max(1) as f32);
    let scale = fit * v.zoom.clamp(0.1, 64.0);
    Rect::from_center_size(area.center() + v.pan, vec2(w as f32 * scale, h as f32 * scale))
}

/// The window (shown while open).
pub fn window(app: &mut ToonApp, ctx: &egui::Context) {
    if !app.colour.viewer.open {
        return;
    }
    let mut open = true;
    egui::Window::new(tl!("Reference Viewer")).id(egui::Id::unique("reference-viewer-window")).open(&mut open).default_size(vec2(420.0, 360.0)).show(
        ctx,
        |ui| {
            ui.horizontal(|ui| {
                if ui.button(tl!("Open Image…")).clicked()
                    && let Some(f) = rfd::FileDialog::new().add_filter(tl!("Images"), &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"]).pick_file()
                {
                    load(app, &f);
                }
                let v = &mut app.colour.viewer;
                if ui.button(tl!("Fit")).clicked() {
                    v.zoom = 1.0;
                    v.pan = Vec2::ZERO;
                }
                let l = ui.label(tl!("Zoom"));
                ui.add(egui::Slider::new(&mut v.zoom, 0.1..=64.0).logarithmic(true).suffix("×")).labelled_by(l.id);
            });
            picture(app, ui);
        },
    );
    if !open {
        app.colour.viewer.open = false;
    }
}

fn picture(app: &mut ToonApp, ui: &mut egui::Ui) {
    let size = ui.available_size().max(vec2(120.0, 120.0));
    let (area, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let Some((name, img)) = app.colour.viewer.image.clone() else {
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, tl!("No reference picture: open one").to_string()));
        ui.painter().text(
            area.center(),
            egui::Align2::CENTER_CENTER,
            tl!("Open a picture to draw from"),
            egui::FontId::proportional(14.0),
            ui.visuals().weak_text_color(),
        );
        return;
    };
    let label = tlf!("Reference picture {name}: click, or arrows and Enter, to pick a colour", name = name).to_string();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, label.clone()));
    let v = &mut app.colour.viewer;
    let tex = v
        .texture
        .get_or_insert_with(|| {
            let c = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.data);
            ui.ctx().load_texture("reference-viewer", c, egui::TextureOptions::NEAREST)
        })
        .clone();
    if resp.dragged() {
        v.pan += resp.drag_delta();
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            v.zoom = (v.zoom * (1.0 + scroll * 0.002)).clamp(0.1, 64.0);
        }
    }
    let rect = placed(v, area, img.width, img.height);
    let to_px = |p: egui::Pos2| -> Option<(u32, u32)> {
        let x = ((p.x - rect.min.x) / rect.width() * img.width as f32).floor();
        let y = ((p.y - rect.min.y) / rect.height() * img.height as f32).floor();
        (x >= 0.0 && y >= 0.0 && x < img.width as f32 && y < img.height as f32).then_some((x as u32, y as u32))
    };
    let mut picked = None;
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos().and_then(to_px)
    {
        resp.request_focus();
        v.cursor = p;
        picked = Some(p);
    }
    if resp.has_focus() {
        let step = if ui.input(|i| i.modifiers.shift) { 10 } else { 1 };
        ui.input(|i| {
            let (mut x, mut y) = v.cursor;
            if i.key_pressed(egui::Key::ArrowLeft) {
                x = x.saturating_sub(step);
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                x = (x + step).min(img.width - 1);
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                y = y.saturating_sub(step);
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                y = (y + step).min(img.height - 1);
            }
            v.cursor = (x, y);
            if i.key_pressed(egui::Key::Enter) {
                picked = Some((x, y));
            }
        });
    }
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, ui.visuals().extreme_bg_color);
    painter.image(tex.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    if resp.has_focus() {
        let c = rect.min + vec2((v.cursor.0 as f32 + 0.5) / img.width as f32 * rect.width(), (v.cursor.1 as f32 + 0.5) / img.height as f32 * rect.height());
        let st = Stroke::new(1.5, ui.visuals().selection.stroke.color);
        painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], st);
        painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], st);
        painter.rect_stroke(area, 0.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
    }
    if let Some((x, y)) = picked {
        pick(app, x, y);
    }
}
