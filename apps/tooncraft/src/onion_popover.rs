//! The onion skin's settings popover (right-click the onion toggle in the canvas header): an
//! opacity slider for each step from 5 before to 5 after, the before and after colours, by
//! drawings or by frames, tinted, original or outlined ghosts, keys only and every layer. Every
//! change goes through `Command::SetOnion`.

use egui::{RichText, Slider, Ui};
use tooncraft_doc::{OnionMode, OnionSkin};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Steps with a slider on each side.
pub const SLIDERS: usize = 5;

/// The opacity each slider shows: the step's opacity inside the reach, 0 outside it.
pub fn slider_values(o: &OnionSkin) -> ([f32; SLIDERS], [f32; SLIDERS]) {
    let side = |before: bool| {
        let reach = if before { o.before } else { o.after } as usize;
        std::array::from_fn(|i| if i < reach { o.offset_opacity(if before { -(i as i32 + 1) } else { i as i32 + 1 }) } else { 0.0 })
    };
    (side(true), side(false))
}

/// The settings after setting the slider `step` (1 = nearest) on one side to `value`: the steps
/// switch to their own opacities (keeping what they showed), and the reach grows to a step
/// turned up or shrinks past the outer steps turned down to 0.
pub fn set_slider(o: OnionSkin, before: bool, step: usize, value: f32) -> OnionSkin {
    let mut o = o;
    if !o.per_step {
        for i in 0..tooncraft_doc::ONION_MAX_STEPS {
            let v = o.step_opacity(i as u32 + 1);
            o.before_steps[i] = v;
            o.after_steps[i] = v;
        }
        o.per_step = true;
    }
    let step = step.clamp(1, tooncraft_doc::ONION_MAX_STEPS);
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
    let (steps, reach) = if before { (&mut o.before_steps, &mut o.before) } else { (&mut o.after_steps, &mut o.after) };
    steps[step - 1] = value;
    let top = (*reach as usize).clamp(step, tooncraft_doc::ONION_MAX_STEPS);
    *reach = (1..=top).rev().find(|&i| steps[i - 1] > 0.001).unwrap_or(0) as u8;
    o.sanitized()
}

/// The popover's contents.
pub fn ui(app: &mut ToonApp, ui: &mut Ui) {
    let pal = crate::theme::palette(app.light_theme);
    let o0 = app.engine.project.onion;
    let mut o = o0;
    ui.label(RichText::new(tl!("Onion skin")).strong());
    let (before, after) = slider_values(&o);
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = 64.0;
        ui.spacing_mut().item_spacing.x = 2.0;
        // furthest before on the left, furthest after on the right
        let cols: Vec<(bool, usize, f32)> = (0..SLIDERS).rev().map(|i| (true, i, before[i])).chain((0..SLIDERS).map(|i| (false, i, after[i]))).collect();
        for (is_before, i, v) in cols {
            ui.vertical(|ui| {
                let mut v = v;
                let tint = if is_before { pal.onion_prev } else { pal.onion_next };
                let name = if is_before { tlf!("Onion opacity, {n} before", n = i + 1) } else { tlf!("Onion opacity, {n} after", n = i + 1) };
                let resp = ui
                    .scope(|ui| {
                        ui.visuals_mut().selection.bg_fill = tint;
                        ui.add(Slider::new(&mut v, 0.0..=1.0).vertical().show_value(false).step_by(0.05))
                    })
                    .inner;
                ui.ctx().accesskit_node_builder(resp.id, |n| n.set_label(name.clone()));
                let resp = resp.on_hover_text(name);
                ui.label(RichText::new(if is_before { format!("−{}", i + 1) } else { format!("+{}", i + 1) }).small().color(pal.weak));
                if resp.changed() {
                    o = set_slider(o, is_before, i + 1, v);
                }
            });
            if is_before && i == 0 {
                ui.add_space(8.0);
            }
        }
    });
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Before"));
        ui.color_edit_button_srgb(&mut o.before_tint).labelled_by(l.id).on_hover_text(tl!("Colour of the drawings before this one"));
        ui.add_space(8.0);
        let l = ui.label(tl!("After"));
        ui.color_edit_button_srgb(&mut o.after_tint).labelled_by(l.id).on_hover_text(tl!("Colour of the drawings after this one"));
    });
    // segmented choices rather than drop-downs: a drop-down inside the popover would close it
    ui.horizontal(|ui| {
        ui.label(tl!("Steps"));
        ui.selectable_value(&mut o.mode, OnionMode::Drawings, tl!("By drawings"));
        ui.selectable_value(&mut o.mode, OnionMode::Frames, tl!("By frames"));
    });
    ui.horizontal(|ui| {
        ui.label(tl!("Ghosts"));
        if ui.selectable_label(!o.outline && o.tinted, tl!("Tinted")).clicked() {
            (o.outline, o.tinted) = (false, true);
        }
        if ui.selectable_label(!o.outline && !o.tinted, tl!("Original colours")).clicked() {
            (o.outline, o.tinted) = (false, false);
        }
        if ui.selectable_label(o.outline, tl!("Outlines")).clicked() {
            (o.outline, o.tinted) = (true, true);
        }
    });
    ui.checkbox(&mut o.keys_only, tl!("Keys only")).on_hover_text(tl!("Stepping by drawings, show only drawings marked as keys"));
    ui.checkbox(&mut o.all_layers, tl!("Onion skin on every visible layer"));
    if o != o0 {
        app.run(Command::SetOnion { onion: o });
    }
    ui.separator();
    if ui.button(tl!("More Onion Settings…")).clicked() {
        app.workspace.raise(crate::dock::Pane::Onion);
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sliders_show_the_reach_and_keep_the_falloff_when_switching_to_steps() {
        let o = OnionSkin { before: 2, after: 3, opacity: 0.5, falloff: 0.5, per_step: false, ..OnionSkin::default() };
        let (b, a) = slider_values(&o);
        assert_eq!(b, [0.5, 0.25, 0.0, 0.0, 0.0]);
        assert_eq!(a, [0.5, 0.25, 0.125, 0.0, 0.0]);
        // moving the nearest after slider keeps the others where they were
        let o2 = set_slider(o, false, 1, 0.8);
        assert!(o2.per_step);
        let (b2, a2) = slider_values(&o2);
        assert_eq!(b2, b);
        assert_eq!(a2, [0.8, 0.25, 0.125, 0.0, 0.0]);
    }

    #[test]
    fn turning_a_step_up_grows_the_reach_and_the_outer_steps_down_shrinks_it() {
        let o = OnionSkin { before: 1, after: 1, ..OnionSkin::default() };
        let o = set_slider(o, true, 4, 0.3);
        assert_eq!(o.before, 4);
        let o = set_slider(o, true, 4, 0.0);
        assert_eq!(o.before, 3, "the steps between keep their opacities");
        let mut o = o;
        for s in (1..=3).rev() {
            o = set_slider(o, true, s, 0.0);
        }
        assert_eq!(o.before, 0);
        // a hole in the middle keeps the reach
        let o = set_slider(OnionSkin { after: 3, ..OnionSkin::default() }, false, 2, 0.0);
        assert_eq!(o.after, 3);
        assert_eq!(slider_values(&o).1[1], 0.0);
    }

    #[test]
    fn hostile_slider_input_stays_in_range() {
        let o = OnionSkin::default();
        for (step, v) in [(0, 2.0), (99, -1.0), (3, f32::NAN), (5, f32::INFINITY)] {
            let r = set_slider(o, step % 2 == 0, step, v);
            assert!(r.before as usize <= tooncraft_doc::ONION_MAX_STEPS && r.after as usize <= tooncraft_doc::ONION_MAX_STEPS);
            assert!(r.before_steps.iter().chain(&r.after_steps).all(|s| (0.0..=1.0).contains(s)));
        }
    }
}
