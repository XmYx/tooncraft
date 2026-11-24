//! ToonCraft's dense dark (and light) theme: graphite neutrals and one warm accent. Colour carries
//! meaning only (the accent for the selection and the active state, red and green for onion before
//! and after, blue for the playhead, label colours for layers). Text contrast is at least 4.5:1 and
//! meaningful marks 3:1 against the surfaces they sit on, in both themes (checked in `tests`).

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Layout metrics on a 4 px grid.
#[allow(dead_code, reason = "the §2 metrics: the UI-pass shell and timeline bars are sized from them")]
pub mod metrics {
    pub const GRID: f32 = 4.0;
    /// Buttons, fields and other controls.
    pub const CONTROL_H: f32 = 22.0;
    /// The tool options bar and the canvas header.
    pub const BAR_H: f32 = 28.0;
    /// The top bar (menus, workspaces, search).
    pub const TOP_BAR_H: f32 = 32.0;
    pub const RADIUS_CONTROL: u8 = 4;
    pub const RADIUS_POPOVER: u8 = 6;
    /// Body text.
    pub const FONT_BODY: f32 = 12.5;
    /// Panel tabs and headers.
    pub const FONT_HEADER: f32 = 11.5;
    /// No text is smaller than this.
    pub const FONT_MIN: f32 = 11.0;
    /// An icon glyph's side (the 16 px icon grid).
    pub const ICON: f32 = 16.0;

    // Checked when the app is built: bars sit on the grid and no text is below the minimum.
    const _: () = assert!(BAR_H % GRID == 0.0 && TOP_BAR_H % GRID == 0.0 && CONTROL_H % 2.0 == 0.0 && ICON % GRID == 0.0);
    const _: () = assert!(FONT_HEADER >= FONT_MIN && FONT_BODY >= FONT_MIN);
}

#[allow(dead_code, reason = "the §2 design tokens: some are read only by the UI-pass shell and timeline code and by the contrast test")]
pub struct Palette {
    /// The workspace behind the paper.
    pub bg: Color32,
    /// Panel bodies.
    pub panel: Color32,
    /// Fields, buttons and headers.
    pub raised: Color32,
    /// Grid gutters and slider tracks.
    pub sunken: Color32,
    /// 1 px separators.
    pub line: Color32,
    pub text: Color32,
    pub weak: Color32,
    /// The selection, the active tool and key cells.
    pub accent: Color32,
    pub workspace: Color32,
    pub cell_key: Color32,
    pub cell_hold: Color32,
    /// Hold bars (the same colour as `cell_hold`).
    pub hold: Color32,
    pub grid: Color32,
    /// The playhead, links and the keyboard focus ring.
    pub playhead: Color32,
    /// Onion ghosts and ruler brackets before the current frame.
    pub onion_prev: Color32,
    /// Onion ghosts and ruler brackets after the current frame.
    pub onion_next: Color32,
    /// Cell marks: key, breakdown and in-between.
    pub mark_key: Color32,
    pub mark_bd: Color32,
    pub mark_ib: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(24, 25, 29),
    panel: Color32::from_rgb(32, 33, 38),
    raised: Color32::from_rgb(46, 48, 55),
    sunken: Color32::from_rgb(20, 21, 25),
    line: Color32::from_rgb(58, 60, 68),
    text: Color32::from_rgb(226, 228, 234),
    weak: Color32::from_rgb(160, 164, 176),
    accent: Color32::from_rgb(242, 140, 64),
    workspace: Color32::from_rgb(52, 54, 60),
    cell_key: Color32::from_rgb(242, 140, 64),
    cell_hold: Color32::from_rgb(150, 98, 58),
    hold: Color32::from_rgb(150, 98, 58),
    grid: Color32::from_rgb(58, 60, 68),
    playhead: Color32::from_rgb(90, 170, 255),
    onion_prev: Color32::from_rgb(224, 86, 74),
    onion_next: Color32::from_rgb(79, 184, 106),
    mark_key: Color32::from_rgb(242, 140, 64),
    mark_bd: Color32::from_rgb(201, 162, 255),
    mark_ib: Color32::from_rgb(143, 163, 184),
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(232, 233, 236),
    panel: Color32::from_rgb(244, 244, 246),
    raised: Color32::from_rgb(220, 222, 227),
    sunken: Color32::from_rgb(210, 212, 217),
    line: Color32::from_rgb(200, 202, 208),
    text: Color32::from_rgb(28, 29, 34),
    weak: Color32::from_rgb(84, 88, 98),
    accent: Color32::from_rgb(176, 78, 0),
    workspace: Color32::from_rgb(196, 198, 204),
    cell_key: Color32::from_rgb(176, 78, 0),
    cell_hold: Color32::from_rgb(230, 168, 120),
    hold: Color32::from_rgb(230, 168, 120),
    grid: Color32::from_rgb(200, 202, 208),
    playhead: Color32::from_rgb(0, 96, 200),
    onion_prev: Color32::from_rgb(192, 58, 48),
    onion_next: Color32::from_rgb(42, 127, 66),
    mark_key: Color32::from_rgb(176, 78, 0),
    mark_bd: Color32::from_rgb(118, 70, 190),
    mark_ib: Color32::from_rgb(78, 96, 118),
};

pub fn palette(light: bool) -> &'static Palette {
    if light { &LIGHT } else { &DARK }
}

pub fn apply(ctx: &egui::Context, light: bool) {
    let p = palette(light);
    let mut v = if light { Visuals::light() } else { Visuals::dark() };
    v.panel_fill = p.panel;
    v.window_fill = p.panel;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.raised;
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = p.accent.gamma_multiply(if light { 0.35 } else { 0.55 });
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.playhead;
    let r = CornerRadius::same(metrics::RADIUS_CONTROL);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_fill = p.panel;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.grid);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.bg_fill = p.raised;
    v.widgets.inactive.weak_bg_fill = p.raised;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.weak.gamma_multiply(0.7));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.window_corner_radius = CornerRadius::same(metrics::RADIUS_POPOVER);
    v.menu_corner_radius = CornerRadius::same(metrics::RADIUS_POPOVER);
    // only popovers and menus are raised; panels stay flat
    let shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(102) };
    v.popup_shadow = shadow;
    v.window_shadow = shadow;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(5.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size = egui::vec2(36.0, 20.0);
        s.spacing.slider_width = 120.0;
        s.spacing.window_margin = egui::Margin::same(8);
        use egui::{FontFamily::Proportional, FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::new(metrics::FONT_BODY, Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(metrics::FONT_BODY, Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(metrics::FONT_MIN, Proportional));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lum(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn contrast_meets_wcag_aa() {
        for p in [&DARK, &LIGHT] {
            for bg in [p.bg, p.panel, p.raised] {
                assert!(contrast(p.text, bg) >= 4.5, "text on {bg:?}");
                assert!(contrast(p.weak, bg) >= 4.5, "weak text on {bg:?}: {}", contrast(p.weak, bg));
            }
            assert!(contrast(p.weak, p.sunken) >= 4.5, "weak text on the sunken gutters: {}", contrast(p.weak, p.sunken));
            assert!(contrast(p.text, p.sunken) >= 4.5, "text on the sunken gutters");
            // marks that carry meaning (WCAG 1.4.11 non-text contrast) on panels and in the grid
            let marks = [
                ("key cells", p.cell_key),
                ("accent", p.accent),
                ("playhead", p.playhead),
                ("onion before", p.onion_prev),
                ("onion after", p.onion_next),
                ("key mark", p.mark_key),
                ("breakdown mark", p.mark_bd),
                ("in-between mark", p.mark_ib),
            ];
            for (name, c) in marks {
                for bg in [p.panel, p.sunken] {
                    assert!(contrast(c, bg) >= 3.0, "{name} on {bg:?}: {}", contrast(c, bg));
                }
            }
            // onion before and after are two different colours
            assert_ne!(p.onion_prev, p.onion_next);
            // separators are visible but quieter than text
            assert!(contrast(p.line, p.panel) > 1.0 && contrast(p.line, p.panel) < contrast(p.weak, p.panel));
        }
    }
}
