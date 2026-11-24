//! The text tool (T): click the canvas (or press Enter on it, at the keyboard pen) to place a
//! text box, type, then Place Text (Ctrl+Enter) paints it into the drawing as one undo step;
//! Escape drops it. Until then the text stays editable and its box shows on the canvas. Font,
//! size, alignment and spacing are in the tool options bar; the colour is the current colour.
//! The painting is `tooncraft_engine::text` (Command::Text).

use std::sync::Arc;

use egui::{Color32, Pos2, Rect, Stroke, pos2, vec2};
use tooncraft_engine::Command;
use tooncraft_engine::text::{TextAlign, TextStyle};

use crate::app::ToonApp;

/// A text box being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Editing {
    /// Canvas position of the anchor (top of the first line).
    pub at: (f32, f32),
    pub text: String,
    /// Give the edit field the keyboard on the next frame.
    pub focus: bool,
    /// The field had the keyboard at the end of the last frame (egui drops focus on Escape
    /// before widgets run, so this is how Escape is known to be meant for the text).
    pub typing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextTool {
    pub style: TextStyle,
    pub editing: Option<Editing>,
}

/// Give the engine the fonts the app ships with: egui's own faces, then any craft-fonts faces
/// the build embedded (borrowed, not copied).
pub fn register_fonts(app: &mut ToonApp) {
    let defs = egui::FontDefinitions::default();
    for (key, name) in [("Ubuntu-Light", "Ubuntu Light"), ("Hack", "Hack Regular")] {
        if let Some(data) = defs.font_data.get(key) {
            app.engine.fonts.add(name, data.font.clone(), data.index);
        }
    }
    for face in crate::i18n::CRAFT_FONTS {
        if !face.is_cjk() {
            app.engine.fonts.add(&face.name(), Arc::new(face.bytes), face.index);
        }
    }
}

/// Start a text box at a canvas point (committing one being typed first).
pub fn start(app: &mut ToonApp, at: (f32, f32)) {
    commit(app);
    app.aids.text.editing = Some(Editing { at, text: String::new(), focus: true, typing: false });
    app.set_status(tl!("Type the text; Ctrl+Enter places it, Escape drops it"));
}

fn style(app: &ToonApp) -> TextStyle {
    TextStyle { color: app.color, ..app.aids.text.style.clone() }
}

/// Paint the text being typed into the drawing (one undo step).
pub fn commit(app: &mut ToonApp) {
    let Some(ed) = app.aids.text.editing.take() else { return };
    if ed.text.trim().is_empty() {
        return;
    }
    let (layer, frame, style) = (app.layer, app.frame, style(app));
    if app.run(Command::Text { layer, frame, text: ed.text, x: ed.at.0, y: ed.at.1, style }) {
        app.set_status(tl!("Text placed"));
    }
}

/// The text tool's canvas input: a click places a text box there.
pub fn input(app: &mut ToonApp, at: Option<(f32, f32)>, resp: &egui::Response) {
    if resp.clicked_by(egui::PointerButton::Primary)
        && let Some(p) = at
    {
        start(app, p);
    }
}

/// The edit box floating at the text's place on the canvas; `to_screen` maps canvas pixels.
pub fn edit_box(app: &mut ToonApp, ctx: &egui::Context, canvas: Rect, to_screen: &dyn Fn(f32, f32) -> Pos2) {
    let Some(ed) = app.aids.text.editing.as_ref() else { return };
    let room = vec2(260.0, 120.0).min(canvas.size());
    let anchor = to_screen(ed.at.0, ed.at.1).clamp(canvas.min, canvas.max - room);
    let mut place = false;
    let mut drop = false;
    let mut text = ed.text.clone();
    let (focus, was_typing) = (ed.focus, ed.typing);
    let mut typing_now = false;
    egui::Area::new(egui::Id::unique("tooncraft-text-box")).order(egui::Order::Foreground).fixed_pos(anchor + vec2(0.0, 6.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            let l = ui.label(tl!("Text"));
            // Ctrl+Enter places the text, so it must not reach the field as a line break
            // read before the field: it gives up the keyboard on Escape while handling it
            let field = egui::Id::unique("tooncraft-text-field");
            let typing = focus || was_typing || ui.memory(|m| m.has_focus(field));
            let commit_key = typing && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter));
            let escape = typing && ui.input(|i| i.key_pressed(egui::Key::Escape));
            let edit = ui.add(egui::TextEdit::multiline(&mut text).id(field).desired_rows(2).desired_width(240.0)).labelled_by(l.id);
            if focus {
                edit.request_focus();
            }
            typing_now = edit.has_focus() || focus;
            place |= commit_key;
            drop |= escape;
            ui.horizontal(|ui| {
                place |= ui.button(tl!("Place Text")).on_hover_text(tl!("Paint the text into the drawing (Ctrl+Enter)")).clicked();
                drop |= ui.button(tl!("Cancel")).on_hover_text(tl!("Drop the text (Escape)")).clicked();
            });
        });
    });
    if let Some(ed) = app.aids.text.editing.as_mut() {
        ed.text = text;
        ed.focus = false;
        ed.typing = typing_now;
    }
    if drop {
        app.aids.text.editing = None;
        app.set_status(tl!("Text dropped"));
    } else if place {
        commit(app);
    }
}

/// The box the text being typed takes, drawn on the canvas.
pub fn overlay(painter: &egui::Painter, app: &ToonApp, to_screen: &dyn Fn(f32, f32) -> Pos2, accent: Color32) {
    let Some(ed) = &app.aids.text.editing else { return };
    let st = style(app);
    let (x0, y0, x1, y1) = match app.engine.fonts.get(&st.font).and_then(|f| tooncraft_engine::text::layout(f, &ed.text, ed.at, &st).ok()) {
        Some(l) => l.bounds,
        None => (ed.at.0, ed.at.1, ed.at.0, ed.at.1 + st.size),
    };
    let r = Rect::from_two_pos(to_screen(x0, y0), to_screen(x1.max(x0 + 1.0), y1));
    painter.rect_stroke(r.expand(2.0), 0.0, Stroke::new(1.0, accent), egui::StrokeKind::Outside);
    let a = to_screen(ed.at.0, ed.at.1);
    painter.line_segment([a, pos2(a.x, to_screen(ed.at.0, y1).y)], Stroke::new(1.5, accent));
}

fn align_name(a: TextAlign) -> String {
    match a {
        TextAlign::Left => tl!("Align left"),
        TextAlign::Center => tl!("Centre"),
        TextAlign::Right => tl!("Align right"),
    }
    .to_string()
}

/// The text tool's options: font, size, alignment, line and letter spacing.
pub fn options(app: &mut ToonApp, ui: &mut egui::Ui) {
    let names = app.engine.fonts.names();
    let st = &mut app.aids.text.style;
    if names.is_empty() {
        ui.label(tl!("No font is available"));
    } else {
        let current = if st.font.is_empty() { names.first().cloned().unwrap_or_default() } else { st.font.clone() };
        let l = ui.label(tl!("Font"));
        egui::ComboBox::from_id_salt("text-font")
            .selected_text(current.clone())
            .show_ui(ui, |ui| {
                for n in &names {
                    if ui.selectable_label(*n == current, n.as_str()).clicked() {
                        st.font = n.clone();
                    }
                }
            })
            .response
            .labelled_by(l.id);
    }
    let l = ui.label(tl!("Size"));
    ui.add(egui::DragValue::new(&mut st.size).range(tooncraft_engine::text::MIN_SIZE..=tooncraft_engine::text::MAX_SIZE).suffix(" px")).labelled_by(l.id);
    for a in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
        ui.selectable_value(&mut st.align, a, align_name(a));
    }
    let l = ui.label(tl!("Line spacing"));
    ui.add(egui::DragValue::new(&mut st.line_spacing).range(0.5..=4.0).speed(0.05).max_decimals(2)).labelled_by(l.id);
    let l = ui.label(tl!("Letter spacing"));
    ui.add(egui::DragValue::new(&mut st.letter_spacing).range(-50.0..=200.0).speed(0.2).suffix(" px")).labelled_by(l.id);
    ui.label(tl!("Click the canvas to place text; Enter places it at the keyboard pen"));
}
