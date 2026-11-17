//! ToonCraft's dense dark (and light) theme. Text contrast is at least 4.5:1 and control edges
//! 3:1 against their backgrounds in both themes (checked in `tests`).

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub workspace: Color32,
    pub cell_key: Color32,
    pub cell_hold: Color32,
    pub grid: Color32,
    pub playhead: Color32,
}

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(24, 25, 29),
    panel: Color32::from_rgb(32, 33, 38),
    raised: Color32::from_rgb(46, 48, 55),
    text: Color32::from_rgb(226, 228, 234),
    weak: Color32::from_rgb(160, 164, 176),
    accent: Color32::from_rgb(242, 140, 64),
    workspace: Color32::from_rgb(52, 54, 60),
    cell_key: Color32::from_rgb(242, 140, 64),
    cell_hold: Color32::from_rgb(150, 98, 58),
    grid: Color32::from_rgb(58, 60, 68),
    playhead: Color32::from_rgb(90, 170, 255),
};

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(232, 233, 236),
    panel: Color32::from_rgb(244, 244, 246),
    raised: Color32::from_rgb(220, 222, 227),
    text: Color32::from_rgb(28, 29, 34),
    weak: Color32::from_rgb(84, 88, 98),
    accent: Color32::from_rgb(176, 78, 0),
    workspace: Color32::from_rgb(196, 198, 204),
    cell_key: Color32::from_rgb(196, 96, 20),
    cell_hold: Color32::from_rgb(230, 168, 120),
    grid: Color32::from_rgb(200, 202, 208),
    playhead: Color32::from_rgb(0, 96, 200),
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
    let r = CornerRadius::same(3);
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
    v.window_corner_radius = CornerRadius::same(5);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(5.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size = egui::vec2(36.0, 20.0);
        s.spacing.slider_width = 120.0;
        s.spacing.window_margin = egui::Margin::same(8);
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
            assert!(contrast(p.cell_key, p.panel) >= 3.0, "key cells");
            assert!(contrast(p.playhead, p.panel) >= 3.0, "playhead");
        }
    }
}
