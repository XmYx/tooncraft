//! Our own colour picker: a saturation/value field and a hue strip (click or drag), with
//! hue/saturation/value number fields as the keyboard alternative.

use egui::{Color32, Mesh, Pos2, Rect, Sense, Stroke, ecolor::Hsva, pos2, vec2};

fn to_rgb(h: Hsva) -> [u8; 3] {
    let c = Color32::from(h);
    [c.r(), c.g(), c.b()]
}

/// Edit a straight RGB colour. Returns true when it changed.
pub fn picker(ui: &mut egui::Ui, id: &str, rgb: &mut [u8; 3]) -> bool {
    let key = egui::Id::unique(("hsv", id));
    // keep hue while saturation or value is zero
    let mut hsv: Hsva =
        ui.data(|d| d.get_temp::<Hsva>(key)).filter(|h| to_rgb(*h) == *rgb).unwrap_or_else(|| Hsva::from(Color32::from_rgb(rgb[0], rgb[1], rgb[2])));
    let before = hsv;
    let w = ui.available_width().clamp(120.0, 260.0);
    let (sv_rect, sv) = ui.allocate_exact_size(vec2(w, w * 0.62), Sense::click_and_drag());
    let mut mesh = Mesh::default();
    let n = 12;
    for j in 0..=n {
        for i in 0..=n {
            let (s, v) = (i as f32 / n as f32, 1.0 - j as f32 / n as f32);
            let p = pos2(sv_rect.left() + s * sv_rect.width(), sv_rect.top() + (1.0 - v) * sv_rect.height());
            mesh.colored_vertex(p, Color32::from(Hsva::new(hsv.h, s, v, 1.0)));
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = (j * (n + 1) + i) as u32;
            let b = a + n as u32 + 1;
            mesh.add_triangle(a, a + 1, b);
            mesh.add_triangle(a + 1, b + 1, b);
        }
    }
    ui.painter().add(mesh);
    let mark = pos2(sv_rect.left() + hsv.s * sv_rect.width(), sv_rect.top() + (1.0 - hsv.v) * sv_rect.height());
    ui.painter().circle_stroke(mark, 5.0, Stroke::new(1.5, if hsv.v > 0.5 { Color32::BLACK } else { Color32::WHITE }));
    sv.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, tl!("Saturation and brightness")));
    if let Some(p) = sv.interact_pointer_pos().filter(|_| sv.dragged() || sv.clicked()) {
        hsv.s = ((p.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
        hsv.v = 1.0 - ((p.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
    }
    let (h_rect, hr) = ui.allocate_exact_size(vec2(w, 14.0), Sense::click_and_drag());
    let mut hm = Mesh::default();
    for i in 0..=24 {
        let t = i as f32 / 24.0;
        let x = h_rect.left() + t * h_rect.width();
        let c = Color32::from(Hsva::new(t, 1.0, 1.0, 1.0));
        hm.colored_vertex(pos2(x, h_rect.top()), c);
        hm.colored_vertex(pos2(x, h_rect.bottom()), c);
        if i > 0 {
            let k = (i * 2) as u32;
            hm.add_triangle(k - 2, k - 1, k);
            hm.add_triangle(k - 1, k + 1, k);
        }
    }
    ui.painter().add(hm);
    let hx = h_rect.left() + hsv.h * h_rect.width();
    ui.painter().rect_stroke(
        Rect::from_center_size(Pos2::new(hx, h_rect.center().y), vec2(5.0, 16.0)),
        1.0,
        Stroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Middle,
    );
    hr.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Slider, true, tl!("Hue")));
    if let Some(p) = hr.interact_pointer_pos().filter(|_| hr.dragged() || hr.clicked()) {
        hsv.h = ((p.x - h_rect.left()) / h_rect.width()).clamp(0.0, 0.9999);
    }
    ui.horizontal(|ui| {
        let mut deg = hsv.h * 360.0;
        let l = ui.label(tl!("Hue"));
        if ui.add(egui::DragValue::new(&mut deg).range(0.0..=359.0).max_decimals(0)).labelled_by(l.id).changed() {
            hsv.h = deg / 360.0;
        }
        let mut s = hsv.s * 100.0;
        let l = ui.label(tl!("Saturation"));
        if ui.add(egui::DragValue::new(&mut s).range(0.0..=100.0).max_decimals(0)).labelled_by(l.id).changed() {
            hsv.s = s / 100.0;
        }
        let mut v = hsv.v * 100.0;
        let l = ui.label(tl!("Brightness"));
        if ui.add(egui::DragValue::new(&mut v).range(0.0..=100.0).max_decimals(0)).labelled_by(l.id).changed() {
            hsv.v = v / 100.0;
        }
    });
    ui.data_mut(|d| d.insert_temp(key, hsv));
    if hsv != before {
        *rgb = to_rgb(hsv);
        return true;
    }
    false
}

/// A colour swatch that opens our picker inline below it. Returns true on change.
pub fn swatch_edit(ui: &mut egui::Ui, id: &str, label: &str, rgb: &mut [u8; 3]) -> bool {
    let open_key = egui::Id::unique(("swatch-open", id));
    let open = ui.data(|d| d.get_temp::<bool>(open_key).unwrap_or(false));
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 18.0), Sense::click());
    ui.painter().rect(rect, 3.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]), Stroke::new(1.0, Color32::from_gray(130)), egui::StrokeKind::Inside);
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 3.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
    }
    let l = label.to_string();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, l.clone()));
    if resp.on_hover_text(label).clicked() {
        ui.data_mut(|d| d.insert_temp(open_key, !open));
    }
    if open {
        let mut changed = false;
        egui::Window::new(label)
            .id(egui::Id::unique(("swatch-win", id)))
            .collapsible(false)
            .resizable(false)
            .default_pos(rect.left_bottom() + vec2(0.0, 4.0))
            .show(ui.ctx(), |ui| {
                changed = picker(ui, id, rgb);
                if ui.button(tl!("Done")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    ui.data_mut(|d| d.insert_temp(open_key, false));
                }
            });
        return changed;
    }
    false
}
