//! The selection beyond one outline: several rings (holes, separate parts), made by adding,
//! subtracting or intersecting (the tool option, or Shift / Alt / both while dragging), the
//! ellipse and magic wand modes, invert, grow and shrink, skew, free distort with four corner
//! handles, and the resampling choice. The main ring stays in `ToonApp::selection` (its box
//! is the transform's), the others here; the masks are combined in `tooncraft-raster` and traced
//! back into rings, so every selection command takes them as polygons.

use tooncraft_engine::Command;
use tooncraft_raster::fill::Mask;
use tooncraft_raster::select::{self, Combine, Resample};

use crate::app::ToonApp;

#[derive(Clone, Debug, Default)]
pub struct State {
    /// The rings after the main one, valid while `main` is the current main ring.
    rings: Vec<Vec<(f32, f32)>>,
    main: Vec<(f32, f32)>,
    /// How a new lasso, rectangle, ellipse or wand area combines with the selection.
    pub combine: Combine,
    pub resample: Resample,
    pub wand_tolerance: u8,
    /// What the magic wand looks at.
    pub wand_reference: tooncraft_engine::FillReference,
    /// Pixels for Grow and Shrink.
    pub grow_by: i32,
    /// Skew, degrees, for Apply Skew.
    pub skew: (f32, f32),
    /// Free distort in progress: the corners the selection's box goes to.
    pub distort: Option<[(f32, f32); 4]>,
    /// The corner being dragged.
    pub dragging: Option<usize>,
    /// Magic wand: only the area touching the click (off: that colour everywhere).
    pub contiguous: bool,
    /// Typed box and angle (x, y, width, height, degrees), and the outline they were read from.
    pub typed: [f32; 5],
    typed_for: Vec<(f32, f32)>,
    /// Width and height change together.
    pub keep_proportions: bool,
    /// Soft edge of the selection, pixels (0: hard): moving, erasing, copying and filling blend
    /// over this distance.
    pub feather: f32,
}

impl State {
    pub fn new() -> Self {
        Self {
            wand_tolerance: 32,
            grow_by: 4,
            contiguous: true,
            keep_proportions: true,
            wand_reference: tooncraft_engine::FillReference::Layer,
            ..Self::default()
        }
    }
}

/// The selection's other rings (empty when there are none or they belong to an older outline).
pub fn extra_rings(app: &ToonApp) -> Vec<Vec<(f32, f32)>> {
    match &app.selection {
        Some(main) if *main == app.sel.main => app.sel.rings.clone(),
        _ => Vec::new(),
    }
}

/// Every ring of the selection, main first.
pub fn all_rings(app: &ToonApp) -> Vec<Vec<(f32, f32)>> {
    let mut out = Vec::new();
    if let Some(m) = &app.selection {
        out.push(m.clone());
        out.extend(extra_rings(app));
    }
    out
}

/// Make `rings` the selection (the largest is the main one); empty deselects.
pub fn set_rings(app: &mut ToonApp, mut rings: Vec<Vec<(f32, f32)>>) {
    rings.retain(|r| r.len() >= 3);
    let area = |r: &Vec<(f32, f32)>| {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in r {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
        (x1 - x0) * (y1 - y0)
    };
    if let Some(i) = (0..rings.len()).max_by(|a, b| area(&rings[*a]).total_cmp(&area(&rings[*b]))) {
        let main = rings.remove(i);
        app.selection = Some(main.clone());
        app.sel.main = main;
        app.sel.rings = rings;
    } else {
        app.selection = None;
        app.sel.rings.clear();
        app.sel.main.clear();
    }
    app.sel.distort = None;
}

/// The current selection as a mask with its feather (soft values at the edge), for copying.
pub fn soft_mask(app: &ToonApp) -> Option<Mask> {
    let main = app.selection.as_ref()?;
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    tooncraft_engine::select::soft_mask(w, h, main, &extra_rings(app), app.sel.feather)
}

/// The current selection as a mask (hard-edged: the outline the tools combine and trace).
pub fn mask(app: &ToonApp) -> Option<Mask> {
    let main = app.selection.as_ref()?;
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    tooncraft_engine::select::selection_mask(w, h, main, &extra_rings(app))
}

/// Combine `new` with the selection by `op` and make the result the selection.
pub fn apply_mask(app: &mut ToonApp, new: &Mask, op: Combine) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let current = if op == Combine::Replace { None } else { mask(app) };
    let out = match (op, &current) {
        (Combine::Replace, _) | (Combine::Add, None) => Some(new.clone()),
        (Combine::Subtract | Combine::Intersect, None) => None,
        (op, Some(c)) => select::combine(Some(c), new, op, w, h),
    };
    match out.and_then(|m| select::trace(&m)) {
        Some(rings) => set_rings(app, rings),
        None if out_is_empty(op, &current) => set_rings(app, Vec::new()),
        None => app.set_status(tl!("The selection is too detailed to keep; nothing changed")),
    }
}

fn out_is_empty(op: Combine, current: &Option<Mask>) -> bool {
    matches!(op, Combine::Subtract | Combine::Intersect) || current.is_none()
}

/// The combine mode for a new outline: the option, or Shift (add), Alt (subtract), both
/// (intersect) held.
pub fn combine_for(app: &ToonApp, mods: egui::Modifiers) -> Combine {
    match (mods.shift, mods.alt) {
        (true, true) => Combine::Intersect,
        (true, false) => Combine::Add,
        (false, true) => Combine::Subtract,
        _ => app.sel.combine,
    }
}

/// A new lasso, rectangle or ellipse outline, combined by `op`.
pub fn add_ring(app: &mut ToonApp, ring: Vec<(f32, f32)>, op: Combine) {
    if op == Combine::Replace || app.selection.is_none() && op == Combine::Add {
        set_rings(app, vec![ring]);
        return;
    }
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    if let Some(m) = select::polygon_mask(w, h, &ring) {
        apply_mask(app, &m, op);
    }
}

/// An ellipse inside the box from `a` to `b`, as an outline of 72 points.
pub fn ellipse(a: (f32, f32), b: (f32, f32)) -> Vec<(f32, f32)> {
    let (cx, cy) = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let (rx, ry) = ((b.0 - a.0).abs() * 0.5, (b.1 - a.1).abs() * 0.5);
    (0..72)
        .map(|i| {
            let t = i as f32 / 72.0 * std::f32::consts::TAU;
            (cx + rx * t.cos(), cy + ry * t.sin())
        })
        .collect()
}

/// The magic wand at a canvas point.
pub fn wand(app: &mut ToonApp, x: f32, y: f32, op: Combine) {
    if x < 0.0 || y < 0.0 {
        return;
    }
    let (tol, reference) = (app.sel.wand_tolerance, app.sel.wand_reference);
    let contiguous = app.sel.contiguous;
    match tooncraft_engine::select::wand_looking_at(&app.engine, app.layer, app.frame, (x as u32, y as u32), tol, reference, contiguous) {
        Some(m) => {
            apply_mask(app, &m, op);
            let n = m.count();
            app.set_status(tlf!("Selected {n} pixels", n = n));
        }
        None => app.set_status(tl!("Nothing to select there")),
    }
}

pub fn invert(app: &mut ToonApp) {
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    let current = mask(app);
    match select::invert(current.as_ref(), w, h).and_then(|m| select::trace(&m)) {
        Some(r) => set_rings(app, r),
        None => set_rings(app, Vec::new()),
    }
}

/// Grow (`n` > 0) or shrink the selection by `n` pixels.
pub fn grow(app: &mut ToonApp, n: i32) {
    let Some(m) = mask(app) else { return };
    let (w, h) = (app.engine.project.width(), app.engine.project.height());
    match select::grow(&m, n, w, h).and_then(|m| select::trace(&m)) {
        Some(r) => set_rings(app, r),
        None => set_rings(app, Vec::new()),
    }
}

/// Make `rings` (main first) the selection without reordering them (a transform in progress).
pub fn keep_rings(app: &mut ToonApp, rings: Vec<Vec<(f32, f32)>>) {
    let mut it = rings.into_iter();
    if let Some(main) = it.next() {
        app.selection = Some(main.clone());
        app.sel.main = main;
        app.sel.rings = it.collect();
    }
}

/// Move every ring of the selection by `t` (after a transform of the pixels).
pub fn map_rings(app: &mut ToonApp, f: impl Fn((f32, f32)) -> (f32, f32)) {
    let rings: Vec<Vec<(f32, f32)>> = all_rings(app).into_iter().map(|r| r.into_iter().map(&f).collect()).collect();
    if rings.is_empty() {
        return;
    }
    let mut it = rings.into_iter();
    if let Some(main) = it.next() {
        app.selection = Some(main.clone());
        app.sel.main = main;
        app.sel.rings = it.collect();
    }
}

/// The box of an outline: (x, y, width, height).
pub fn bbox(poly: &[(f32, f32)]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in poly {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if x0 > x1 { (0.0, 0.0, 0.0, 0.0) } else { (x0, y0, x1 - x0, y1 - y0) }
}

/// Move, scale and turn the selection so its box becomes the typed one (one undo step).
pub fn apply_typed(app: &mut ToonApp) {
    let Some(main) = app.selection.clone() else { return };
    let (x0, y0, w0, h0) = bbox(&main);
    if w0 <= 0.0 || h0 <= 0.0 {
        return;
    }
    let [x, y, w, h, deg] = app.sel.typed;
    if ![x, y, w, h, deg].iter().all(|v| v.is_finite()) || w <= 0.0 || h <= 0.0 {
        app.set_status(tl!("Width and height must be more than zero"));
        return;
    }
    let (dx, dy) = ((x + w / 2.0) - (x0 + w0 / 2.0), (y + h / 2.0) - (y0 + h0 / 2.0));
    let (sx, sy) = ((w / w0).clamp(0.01, 100.0), (h / h0).clamp(0.01, 100.0));
    app.transform_selection(dx, dy, (sx, sy), deg.to_radians());
    // the fields now describe the new outline (an angle is applied once)
    app.sel.typed_for.clear();
}

/// Start a free distort: the four corners of the selection's box become handles.
pub fn start_distort(app: &mut ToonApp) {
    if let Some(m) = mask(app) {
        let (x0, y0, x1, y1) = (m.x as f32, m.y as f32, (m.x + m.width) as f32, (m.y + m.height) as f32);
        app.sel.distort = Some([(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }
}

/// Apply the distort in progress (one undo step); the selection becomes the new quad.
pub fn apply_distort(app: &mut ToonApp) {
    let (Some(corners), Some(main)) = (app.sel.distort, app.selection.clone()) else { return };
    let (layer, frame, rings, resample) = (app.layer, app.frame, extra_rings(app), app.sel.resample);
    if app.run(Command::DistortSelection { layer, frame, polygon: main, rings, corners, resample, feather: app.sel.feather }) {
        set_rings(app, vec![corners.to_vec()]);
    }
    app.sel.distort = None;
}

/// Skew the selection by the degrees in the options (one undo step).
pub fn apply_skew(app: &mut ToonApp) {
    let Some(main) = app.selection.clone() else { return };
    let (kx, ky) = (app.sel.skew.0.to_radians(), app.sel.skew.1.to_radians());
    let (layer, frame, rings, resample) = (app.layer, app.frame, extra_rings(app), app.sel.resample);
    let cmd = Command::TransformSelection {
        layer,
        frame,
        polygon: main.clone(),
        rings,
        dx: 0.0,
        dy: 0.0,
        scale_x: 1.0,
        scale_y: 1.0,
        angle: 0.0,
        skew_x: kx,
        skew_y: ky,
        resample,
        feather: app.sel.feather,
    };
    if app.run(cmd) {
        let t = tooncraft_raster::select::Affine::about_skewed(tooncraft_engine::select::pivot(&main), 0.0, 0.0, 1.0, 1.0, 0.0, (kx, ky));
        map_rings(app, |p| t.apply(p.0, p.1));
    }
}

fn combine_name(c: Combine) -> String {
    match c {
        Combine::Replace => tl!("New Area"),
        Combine::Add => tl!("Add Area"),
        Combine::Subtract => tl!("Remove Area"),
        Combine::Intersect => tl!("Keep Overlap"),
    }
    .to_string()
}

fn resample_name(r: Resample) -> String {
    match r {
        Resample::Nearest => tl!("Nearest (hard edges)"),
        Resample::Bilinear => tl!("Bilinear (smooth)"),
        Resample::Bicubic => tl!("Bicubic (sharp)"),
    }
    .to_string()
}

/// The select tool's extra options (after the mode buttons).
pub fn options(app: &mut ToonApp, ui: &mut egui::Ui) {
    for c in [Combine::Replace, Combine::Add, Combine::Subtract, Combine::Intersect] {
        ui.selectable_value(&mut app.sel.combine, c, combine_name(c)).on_hover_text(tl!("Or hold Shift to add, Alt to subtract, both to intersect"));
    }
    ui.separator();
    if app.select_mode == crate::app::SelectMode::Wand {
        let l = ui.label(tl!("Tolerance"));
        ui.add(egui::Slider::new(&mut app.sel.wand_tolerance, 0..=255)).labelled_by(l.id);
        crate::layer_fx::reference_picker(ui, "wand-ref", &mut app.sel.wand_reference);
        ui.checkbox(&mut app.sel.contiguous, tl!("Touching only")).on_hover_text(tl!("Off: that colour everywhere in the picture"));
        ui.separator();
    }
    let has = app.selection.is_some();
    // typed box and angle, read from the outline whenever it changes
    if let Some(main) = app.selection.clone() {
        if main != app.sel.typed_for {
            let (x, y, w, h) = bbox(&main);
            app.sel.typed = [x, y, w, h, 0.0];
            app.sel.typed_for = main;
        }
        let ratio = if app.sel.typed[3] > 0.0 { app.sel.typed[2] / app.sel.typed[3] } else { 1.0 };
        let mut t = app.sel.typed;
        for (i, name) in [tl!("X"), tl!("Y"), tl!("Width"), tl!("Height"), tl!("Angle")].iter().enumerate() {
            let l = ui.label(*name);
            let range = if i == 4 {
                -360.0..=360.0
            } else if i >= 2 {
                0.5..=65536.0
            } else {
                -65536.0..=65536.0
            };
            ui.add(egui::DragValue::new(&mut t[i]).range(range).max_decimals(1).suffix(if i == 4 { "°" } else { "" })).labelled_by(l.id);
        }
        if app.sel.keep_proportions && ratio > 0.0 {
            if t[2] != app.sel.typed[2] {
                t[3] = t[2] / ratio;
            } else if t[3] != app.sel.typed[3] {
                t[2] = t[3] * ratio;
            }
        }
        app.sel.typed = t;
        ui.checkbox(&mut app.sel.keep_proportions, tl!("Keep proportions"));
        if ui.button(tl!("Apply Box")).on_hover_text(tl!("Move, scale and turn the selection to these numbers")).clicked() {
            apply_typed(app);
        }
        ui.separator();
    }
    if ui.button(tl!("Invert")).on_hover_text(tl!("Select everything that is not selected (Ctrl+Shift+I)")).clicked() {
        invert(app);
    }
    let l = ui.label(tl!("By"));
    ui.add(egui::DragValue::new(&mut app.sel.grow_by).range(1..=select::MAX_GROW).suffix(tl!(" px"))).labelled_by(l.id);
    if ui.add_enabled(has, egui::Button::new(tl!("Grow"))).clicked() {
        let n = app.sel.grow_by;
        grow(app, n);
    }
    if ui.add_enabled(has, egui::Button::new(tl!("Shrink"))).clicked() {
        let n = app.sel.grow_by;
        grow(app, -n);
    }
    let l = ui.label(tl!("Feather"));
    ui.add(egui::DragValue::new(&mut app.sel.feather).range(0.0..=select::MAX_FEATHER).max_decimals(1).suffix(tl!(" px")))
        .labelled_by(l.id)
        .on_hover_text(tl!("Soft edge: moving, erasing, copying and filling the selection fade out over this many pixels"));
    ui.separator();
    let l = ui.label(tl!("Resampling"));
    egui::ComboBox::from_id_salt("select-resample")
        .selected_text(resample_name(app.sel.resample))
        .show_ui(ui, |ui| {
            for r in [Resample::Nearest, Resample::Bilinear, Resample::Bicubic] {
                ui.selectable_value(&mut app.sel.resample, r, resample_name(r));
            }
        })
        .response
        .labelled_by(l.id);
    let l = ui.label(tl!("Skew"));
    ui.add(egui::DragValue::new(&mut app.sel.skew.0).range(-60.0..=60.0).suffix("°")).labelled_by(l.id).on_hover_text(tl!("Skew along x, degrees"));
    let l2 = ui.label(tl!("and"));
    ui.add(egui::DragValue::new(&mut app.sel.skew.1).range(-60.0..=60.0).suffix("°")).labelled_by(l2.id).on_hover_text(tl!("Skew along y, degrees"));
    if ui.add_enabled(has, egui::Button::new(tl!("Apply Skew"))).clicked() {
        apply_skew(app);
    }
    if app.sel.distort.is_some() {
        if ui.button(tl!("Apply Distort")).clicked() {
            apply_distort(app);
        }
        if ui.button(tl!("Cancel Distort")).clicked() {
            app.sel.distort = None;
        }
    } else if ui.add_enabled(has, egui::Button::new(tl!("Distort"))).on_hover_text(tl!("Drag the four corners freely (perspective), then Apply")).clicked() {
        start_distort(app);
    }
}
