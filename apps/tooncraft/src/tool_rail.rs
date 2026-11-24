//! The tool rail (44 px), down the left side. Tools come in groups. A group with more than one
//! variant has a corner marker; press and hold it, right-click it, or press the Menu key or
//! Shift+F10 on it to open its flyout, where each variant shows its icon, name and key. The
//! button keeps the variant used last (remembered in the preferences). Below the groups, after a
//! gap, come the view tools and the current and previous colour with a swap button.
//!
//! The groups and variants are one table (`GROUPS`); the canvas, the tool options bar and the
//! keys read the same `Tool` values.

use std::collections::BTreeMap;

use egui::{Color32, Sense, Stroke, vec2};

use crate::app::{SelectMode, Tool, ToonApp};
use crate::icons::Icon;

/// The rail's width.
pub const WIDTH: f32 = 44.0;
/// A rail button.
const BUTTON: egui::Vec2 = vec2(34.0, 32.0);
/// Press this long on a group to open its flyout.
const HOLD_SECONDS: f64 = 0.45;

/// One entry of a group: a tool, a way of selecting, or the selection transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Tool(Tool),
    Select(SelectMode),
    /// The select tool with only its handles live: move, scale and turn the selection.
    TransformSelection,
}

impl Variant {
    /// The stable id kept in the preferences.
    pub fn id(self) -> &'static str {
        match self {
            Variant::Tool(t) => t.id(),
            Variant::Select(SelectMode::Lasso) => "lasso",
            Variant::Select(SelectMode::Rectangle) => "rectangle",
            Variant::Select(SelectMode::Ellipse) => "ellipse",
            Variant::Select(SelectMode::Wand) => "wand",
            Variant::TransformSelection => "transform_selection",
        }
    }

    pub fn label(self) -> String {
        match self {
            Variant::Tool(t) => t.label(),
            Variant::Select(SelectMode::Lasso) => tl!("Lasso select").to_string(),
            Variant::Select(SelectMode::Rectangle) => tl!("Rectangle select").to_string(),
            Variant::Select(SelectMode::Ellipse) => tl!("Ellipse select").to_string(),
            Variant::Select(SelectMode::Wand) => tl!("Magic wand").to_string(),
            Variant::TransformSelection => tl!("Transform selection").to_string(),
        }
    }

    /// The tool this variant uses.
    pub fn tool(self) -> Tool {
        match self {
            Variant::Tool(t) => t,
            Variant::Select(_) | Variant::TransformSelection => Tool::Select,
        }
    }

    /// The shortcut id that picks this variant's tool.
    pub fn key_id(self) -> String {
        format!("tool.{}", self.tool().id())
    }

    pub fn is_active(self, app: &ToonApp) -> bool {
        match self {
            Variant::Tool(t) => app.tool == t,
            Variant::Select(m) => app.tool == Tool::Select && app.select_mode == m && !app.rail.transform_only,
            Variant::TransformSelection => app.tool == Tool::Select && app.rail.transform_only,
        }
    }

    pub fn apply(self, app: &mut ToonApp) {
        app.tool = self.tool();
        app.rail.transform_only = self == Variant::TransformSelection;
        if let Variant::Select(m) = self {
            app.select_mode = m;
        }
    }

    /// The variant's glyph.
    pub fn icon(self) -> Icon {
        match self {
            Variant::Tool(t) => Icon::from(t),
            Variant::Select(SelectMode::Lasso) => Icon::SelectLasso,
            Variant::Select(SelectMode::Rectangle) => Icon::SelectRect,
            Variant::Select(SelectMode::Ellipse) => Icon::SelectEllipse,
            Variant::Select(SelectMode::Wand) => Icon::SelectWand,
            Variant::TransformSelection => Icon::SelectTransform,
        }
    }

    /// Paint the variant's glyph (filled while it is the tool in use).
    pub fn paint(self, painter: &egui::Painter, rect: egui::Rect, color: Color32, filled: bool) {
        let side = rect.width().min(rect.height()).min(crate::theme::metrics::ICON * 1.25);
        crate::icons::paint(painter, egui::Rect::from_center_size(rect.center(), vec2(side, side)), self.icon(), color, filled);
    }
}

/// A group of variants sharing one rail button.
pub struct Group {
    pub id: &'static str,
    pub variants: &'static [Variant],
}

impl Group {
    pub fn label(&self) -> String {
        match self.id {
            "select" => tl!("Select"),
            "transform" => tl!("Transform"),
            "draw" => tl!("Draw"),
            "smudge" => tl!("Smudge"),
            "erase" => tl!("Erase"),
            "fill" => tl!("Fill"),
            "shapes" => tl!("Shapes"),
            "text" => tl!("Text"),
            "picker" => tl!("Picker"),
            _ => tl!("View"),
        }
        .to_string()
    }
}

/// The rail, top to bottom. The view group sits after a spacer, at the bottom.
pub const GROUPS: [Group; 10] = [
    Group {
        id: "select",
        variants: &[
            Variant::Select(SelectMode::Lasso),
            Variant::Select(SelectMode::Rectangle),
            Variant::Select(SelectMode::Ellipse),
            Variant::Select(SelectMode::Wand),
        ],
    },
    Group { id: "transform", variants: &[Variant::TransformSelection, Variant::Tool(Tool::Transform)] },
    Group {
        id: "draw",
        variants: &[
            Variant::Tool(Tool::Pencil),
            Variant::Tool(Tool::Ink),
            Variant::Tool(Tool::Airbrush),
            Variant::Tool(Tool::Chalk),
            Variant::Tool(Tool::Watercolour),
        ],
    },
    Group { id: "smudge", variants: &[Variant::Tool(Tool::Smudge)] },
    Group { id: "erase", variants: &[Variant::Tool(Tool::Eraser)] },
    Group { id: "fill", variants: &[Variant::Tool(Tool::Fill), Variant::Tool(Tool::Gradient)] },
    Group { id: "shapes", variants: &[Variant::Tool(Tool::Shape)] },
    Group { id: "text", variants: &[Variant::Tool(Tool::Text)] },
    Group { id: "picker", variants: &[Variant::Tool(Tool::Picker)] },
    Group { id: "view", variants: &[Variant::Tool(Tool::Hand), Variant::Tool(Tool::Zoom), Variant::Tool(Tool::RotateView)] },
];

/// The index of the view group (laid out at the bottom).
const VIEW: usize = 9;

/// The rail's state that is not a preference.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The select tool moves, scales and turns the selection only (Transform selection).
    pub transform_only: bool,
}

/// Remembered variants from a preferences file: only known groups and their own variants (the
/// file is not trusted).
pub fn sanitized_memory(m: BTreeMap<String, String>) -> BTreeMap<String, String> {
    m.into_iter().filter(|(g, v)| GROUPS.iter().any(|group| group.id == g && group.variants.iter().any(|x| x.id() == v))).collect()
}

/// The group holding the current tool, if any.
pub fn active_group(app: &ToonApp) -> Option<usize> {
    GROUPS.iter().position(|g| g.variants.iter().any(|v| v.is_active(app)))
}

/// The variant a group's button shows: the active one when it is in the group, else the one used
/// there last, else the first.
pub fn shown(app: &ToonApp, group: usize) -> Variant {
    let Some(g) = GROUPS.get(group) else { return Variant::Tool(Tool::Pencil) };
    if let Some(v) = g.variants.iter().find(|v| v.is_active(app)) {
        return *v;
    }
    let last = app.preferences.prefs.tool_rail.get(g.id);
    g.variants.iter().find(|v| Some(v.id()) == last.map(String::as_str)).or(g.variants.first()).copied().unwrap_or(Variant::Tool(Tool::Pencil))
}

/// Pick `v` from group `group`: use it and remember it for the group.
pub fn pick(app: &mut ToonApp, group: usize, v: Variant) {
    v.apply(app);
    remember(app, group, v);
}

fn remember(app: &mut ToonApp, group: usize, v: Variant) {
    let Some(g) = GROUPS.get(group) else { return };
    if app.preferences.prefs.tool_rail.get(g.id).map(String::as_str) != Some(v.id()) {
        app.preferences.prefs.tool_rail.insert(g.id.to_string(), v.id().to_string());
        crate::preferences::save_now(app);
    }
}

/// The id of a group's flyout popup.
pub fn flyout_id(group: usize) -> egui::Id {
    egui::Id::unique("tooncraft-rail-flyout").with(group)
}

/// The id of a group's rail button.
pub fn button_id(group: usize) -> egui::Id {
    egui::Id::unique("tooncraft-rail-button").with(group)
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    // a tool picked by its key counts as that group's last-used variant
    if let Some(g) = active_group(app) {
        let v = shown(app, g);
        if GROUPS[g].variants.len() > 1 {
            remember(app, g, v);
        }
    }
    ui.add_space(4.0);
    let full = ui.max_rect();
    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        for g in 0..VIEW {
            group_button(app, ui, g);
            // thin gaps between the selection, drawing and fill families
            if matches!(GROUPS[g].id, "transform" | "erase" | "picker") {
                ui.add_space(6.0);
            }
        }
        // the view group and the colour chips sit at the bottom
        let chips_h = 64.0;
        let bottom = full.bottom() - chips_h - BUTTON.y - 16.0;
        let gap = bottom - ui.cursor().top();
        if gap > 0.0 {
            ui.add_space(gap);
        }
        group_button(app, ui, VIEW);
        ui.add_space(8.0);
        colour_chips(app, ui);
    });
}

/// One group's button, with its flyout.
fn group_button(app: &mut ToonApp, ui: &mut egui::Ui, g: usize) {
    let v = shown(app, g);
    let group = &GROUPS[g];
    let many = group.variants.len() > 1;
    let active = v.is_active(app);
    let (rect, _) = ui.allocate_exact_size(BUTTON, Sense::hover());
    let resp = ui.interact(rect, button_id(g), Sense::click());
    let pal = crate::theme::palette(app.light_theme);
    let bg = if active {
        pal.accent.gamma_multiply(0.30)
    } else if resp.hovered() {
        pal.raised
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect(rect, 4.0, bg, if active { Stroke::new(1.0, pal.accent) } else { Stroke::NONE }, egui::StrokeKind::Inside);
    let fg = if active { pal.accent } else { pal.weak };
    v.paint(ui.painter(), rect, fg, active);
    if many {
        // the corner marker: this button holds more tools
        let c = rect.right_bottom() - vec2(3.0, 3.0);
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)], fg, Stroke::NONE));
    }
    if resp.has_focus() {
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
    }
    let key = app.keys.text(&v.key_id());
    let name = format!("{} ({key})", v.label());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, name.clone()));
    let tip = if many { tlf!("{tool}; hold or right-click for the other {group} tools", tool = name, group = group.label().to_lowercase()) } else { name };
    let resp = resp.on_hover_text(tip);
    if resp.clicked() {
        pick(app, g, v);
    }
    // open the flyout: hold, right-click, or the Menu key / Shift+F10 on the focused button
    let held = many && resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.press_start_time().is_some_and(|s| i.time - s > HOLD_SECONDS));
    let keyboard = resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Menu) || (i.modifiers.shift && i.key_pressed(egui::Key::F10)));
    let right_click = resp.secondary_clicked() || keyboard;
    let open = (right_click || held).then_some(egui::SetOpenCommand::Bool(true));
    if right_click {
        ui.ctx().data_mut(|d| d.insert_temp(flyout_id(g).with("full"), true));
    } else if held {
        ui.ctx().data_mut(|d| d.insert_temp(flyout_id(g).with("full"), false));
    }
    egui::Popup::from_response(&resp)
        .id(flyout_id(g))
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .open_memory(open)
        .align(egui::RectAlign::RIGHT_START)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| flyout(app, ui, g));
}

/// A group's flyout: every variant with its icon, name and key. From a right-click (or the
/// keyboard) it also offers Reset Tool Settings and Shortcut….
fn flyout(app: &mut ToonApp, ui: &mut egui::Ui, g: usize) {
    let pal = crate::theme::palette(app.light_theme);
    ui.set_min_width(190.0);
    let mut first = None;
    let mut any_focused = false;
    for v in GROUPS[g].variants {
        let on = v.is_active(app);
        let key = app.keys.text(&v.key_id());
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        first.get_or_insert(resp.id);
        any_focused |= resp.has_focus();
        if resp.hovered() || resp.has_focus() || on {
            ui.painter().rect_filled(rect, 3.0, if on { pal.accent.gamma_multiply(0.25) } else { pal.raised });
        }
        let icon = egui::Rect::from_min_size(rect.left_top() + vec2(4.0, 3.0), vec2(20.0, 20.0));
        v.paint(ui.painter(), icon, if on { pal.accent } else { pal.text }, on);
        let font = egui::TextStyle::Button.resolve(ui.style());
        ui.painter().text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, v.label(), font.clone(), pal.text);
        ui.painter().text(rect.right_center() - vec2(6.0, 0.0), egui::Align2::RIGHT_CENTER, &key, font, pal.weak);
        if resp.has_focus() {
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
        }
        let name = format!("{} ({key})", v.label());
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, name.clone()));
        if resp.clicked() {
            pick(app, g, *v);
            ui.close();
        }
    }
    // opened from the keyboard, the focus moves into the flyout
    if let Some(id) = first
        && !any_focused
        && ui.ctx().memory(|m| m.has_focus(button_id(g)))
    {
        ui.ctx().memory_mut(|m| m.request_focus(id));
    }
    let full = ui.ctx().data(|d| d.get_temp::<bool>(flyout_id(g).with("full"))).unwrap_or(false);
    if !full {
        return;
    }
    ui.separator();
    let t = shown(app, g).tool();
    crate::context_menus::tool_button_extras(app, ui, t);
}

/// The current and previous colour, overlapping; a click on the previous one swaps back.
fn colour_chips(app: &mut ToonApp, ui: &mut egui::Ui) {
    let prev_id = egui::Id::unique("tooncraft-rail-previous-colour");
    let seen_id = egui::Id::unique("tooncraft-rail-seen-colour");
    let cur = app.color;
    let (seen, prev) = ui.ctx().data(|d| (d.get_temp::<[u8; 4]>(seen_id), d.get_temp::<[u8; 4]>(prev_id)));
    let prev = match seen {
        Some(s) if s != cur => Some(s),
        _ => prev,
    };
    ui.ctx().data_mut(|d| {
        d.insert_temp(seen_id, cur);
        if let Some(p) = prev {
            d.insert_temp(prev_id, p);
        }
    });
    let pal = crate::theme::palette(app.light_theme);
    let (area, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
    let hex = |c: [u8; 4]| format!("{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    let edge = Stroke::new(1.0, Color32::from_gray(140));
    // previous: behind, lower right
    let back = egui::Rect::from_min_size(area.min + vec2(14.0, 14.0), vec2(20.0, 20.0));
    if let Some(p) = prev {
        let resp = ui.interact(back, egui::Id::unique("tooncraft-rail-chip-previous"), Sense::click());
        ui.painter().rect(back, 3.0, Color32::from_rgb(p[0], p[1], p[2]), edge, egui::StrokeKind::Inside);
        let name = tlf!("Swap to the previous colour #{hex}", hex = hex(p));
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
        if resp.has_focus() {
            ui.painter().rect_stroke(back, 3.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
        }
        if resp.on_hover_text(tl!("Swap the current and previous colours")).clicked() {
            crate::colour_tools::set_rgb(app, [p[0], p[1], p[2]]);
        }
    }
    // current: in front, upper left
    let front = egui::Rect::from_min_size(area.min + vec2(2.0, 2.0), vec2(22.0, 22.0));
    let resp = ui.interact(front, egui::Id::unique("tooncraft-rail-chip-current"), Sense::click());
    ui.painter().rect(front, 3.0, Color32::from_rgb(cur[0], cur[1], cur[2]), edge, egui::StrokeKind::Inside);
    if resp.has_focus() {
        ui.painter().rect_stroke(front, 3.0, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Outside);
    }
    let name = tlf!("Colour #{hex}", hex = hex(cur));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone()));
    if resp.on_hover_text(tl!("Current colour: open the Colour panel")).clicked() {
        app.workspace.raise(crate::dock::Pane::Colour);
    }
    let why = prev.is_none().then(|| tl!("Pick another colour first").to_string());
    let swap = crate::icons::IconButton::new(Icon::Swap, tl!("Swap colours")).size(20.0).disabled(why);
    if ui.add(swap).clicked()
        && let Some(p) = prev
    {
        crate::colour_tools::set_rgb(app, [p[0], p[1], p[2]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_on_the_rail_once_and_ids_are_unique() {
        let mut tools: Vec<Tool> =
            GROUPS.iter().flat_map(|g| g.variants.iter()).filter_map(|v| if let Variant::Tool(t) = v { Some(*t) } else { None }).collect();
        tools.push(Tool::Select);
        for t in Tool::ALL {
            assert_eq!(tools.iter().filter(|x| **x == t).count(), 1, "{t:?} is on the rail once");
        }
        let mut ids: Vec<&str> = GROUPS.iter().flat_map(|g| g.variants.iter().map(|v| v.id())).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "variant ids are unique");
        let mut groups: Vec<&str> = GROUPS.iter().map(|g| g.id).collect();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(groups.len(), GROUPS.len());
        assert_eq!(GROUPS[VIEW].id, "view");
        for g in &GROUPS {
            assert!(!g.variants.is_empty() && !g.label().is_empty());
            for v in g.variants {
                assert!(!v.label().is_empty());
            }
        }
    }

    #[test]
    fn remembered_variants_from_a_file_are_checked() {
        let mut m = BTreeMap::new();
        m.insert("draw".to_string(), "ink".to_string());
        m.insert("draw2".to_string(), "ink".to_string());
        m.insert("fill".to_string(), "ink".to_string());
        m.insert("select".to_string(), "wand".to_string());
        m.insert("x".repeat(10_000), "y".repeat(10_000));
        let s = sanitized_memory(m);
        assert_eq!(s.len(), 2);
        assert_eq!(s["draw"], "ink");
        assert_eq!(s["select"], "wand");
    }
}
