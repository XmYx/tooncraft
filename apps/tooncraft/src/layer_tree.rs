//! Layer folders as a tree (nested folders with show, lock, fold, opacity and a parent), the
//! current layer's clipping switch, and the colour-label filter shared by the layer list and the
//! timeline.

use egui::vec2;
use tooncraft_doc::{Layer, Project};
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// Layer-list state that is not part of the project.
#[derive(Default)]
pub struct LayerUi {
    /// Only list layers carrying this label colour (on the layer or one of its folders).
    pub filter: Option<[u8; 3]>,
}

/// `Outer › Inner › name`.
pub fn path_name(p: &Project, layer: &Layer) -> String {
    let mut parts: Vec<String> = p.folder_chain(layer.folder.as_deref()).iter().rev().map(|f| f.name.clone()).collect();
    parts.push(layer.name.clone());
    parts.join(" › ")
}

fn has_label(p: &Project, layer: &Layer, c: [u8; 3]) -> bool {
    layer.color == Some(c) || p.folder_chain(layer.folder.as_deref()).iter().any(|f| f.color == Some(c))
}

/// Whether the layer list and the timeline show layer `li` (folded folders and the colour filter
/// hide layers; the current layer always shows).
pub fn listed(app: &ToonApp, li: usize) -> bool {
    let p = &app.engine.project;
    let Some(layer) = p.layers.get(li) else { return false };
    li == app.layer || (!p.is_collapsed(layer) && app.layer_ui.filter.is_none_or(|c| has_label(p, layer, c)))
}

/// Indices of the listed layers, top of the stack first (timeline rows).
pub fn rows(app: &ToonApp) -> Vec<usize> {
    (0..app.engine.project.layers.len()).rev().filter(|&li| listed(app, li)).collect()
}

/// Label colours in use, for the filter menu.
fn labels(p: &Project) -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = Vec::new();
    for c in p.layers.iter().filter_map(|l| l.color).chain(p.folders.iter().filter_map(|f| f.color)) {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

fn swatch(ui: &mut egui::Ui, c: [u8; 3]) {
    let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
}

/// The current layer's clipping switch, the label filter and the folder tree.
pub fn folders(app: &mut ToonApp, ui: &mut egui::Ui) {
    let li = app.layer;
    if let Some(layer) = app.engine.project.layers.get(li) {
        let mut clip = layer.clip;
        if ui
            .add_enabled(li > 0, egui::Checkbox::new(&mut clip, tl!("Clip to the layer below")))
            .on_hover_text(tl!("Show this layer only where the layer below has paint"))
            .changed()
        {
            app.run(Command::SetLayerClip { layer: li, clip });
        }
    }
    let labels = labels(&app.engine.project);
    ui.horizontal(|ui| {
        let l = ui.label(tl!("Show layers labelled"));
        let mut filter = app.layer_ui.filter;
        egui::ComboBox::from_id_salt("layer-label-filter")
            .selected_text(match filter {
                None => tl!("Any colour").to_string(),
                Some(c) => format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut filter, None, tl!("Any colour"));
                for c in &labels {
                    ui.horizontal(|ui| {
                        swatch(ui, *c);
                        ui.selectable_value(&mut filter, Some(*c), format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
                    });
                }
            })
            .response
            .labelled_by(l.id);
        app.layer_ui.filter = filter;
    });
    ui.horizontal(|ui| {
        if ui.button(tl!("New Folder")).on_hover_text(tl!("Put the current layer in a new folder inside its folder")).clicked() {
            let p = &app.engine.project;
            let parent = p.layers.get(li).and_then(|l| l.folder.clone());
            let mut n = p.folders.len() + 1;
            while p.folder(&format!("Folder {n}")).is_some() {
                n += 1;
            }
            app.run(Command::AddFolder { name: tlf!("Folder {n}", n = n), parent, layers: vec![li] });
        }
    });
    // the tree: each folder after its parent, indented by depth
    let p = &app.engine.project;
    let mut order: Vec<(tooncraft_doc::Folder, usize)> = Vec::new();
    fn walk(p: &Project, parent: Option<&str>, depth: usize, out: &mut Vec<(tooncraft_doc::Folder, usize)>) {
        if depth > tooncraft_doc::MAX_FOLDER_DEPTH {
            return;
        }
        for f in p.folders.iter().filter(|f| f.parent.as_deref() == parent) {
            out.push((f.clone(), depth));
            walk(p, Some(&f.name), depth + 1, out);
        }
    }
    walk(p, None, 0, &mut order);
    let names: Vec<String> = p.folders.iter().map(|f| f.name.clone()).collect();
    let mut cmds = Vec::new();
    for (f, depth) in order {
        let indent = depth as f32 * 14.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let mut c = f.color.unwrap_or([150, 150, 150]);
            if crate::color::swatch_edit(ui, &format!("folder-colour-{}", f.name), &tlf!("Label colour of {name}", name = f.name), &mut c) {
                cmds.push(Command::SetFolderColor { name: f.name.clone(), color: Some(c) });
            }
            let mut vis = f.visible;
            if ui.checkbox(&mut vis, tlf!("Folder {name}", name = f.name)).changed() {
                cmds.push(Command::SetFolder { name: f.name.clone(), visible: Some(vis), opacity: None, collapsed: None });
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(indent + 18.0);
            let mut locked = f.locked;
            if ui.checkbox(&mut locked, tlf!("Lock {name}", name = f.name)).changed() {
                cmds.push(Command::SetFolderLock { name: f.name.clone(), locked });
            }
            let mut col = f.collapsed;
            if ui.checkbox(&mut col, tlf!("Fold {name}", name = f.name)).changed() {
                cmds.push(Command::SetFolder { name: f.name.clone(), visible: None, opacity: None, collapsed: Some(col) });
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(indent + 18.0);
            let mut o = f.opacity;
            let l = ui.label(tl!("Opacity"));
            if ui.add(egui::DragValue::new(&mut o).range(0.0..=1.0).speed(0.01).max_decimals(2)).labelled_by(l.id).changed() {
                cmds.push(Command::SetFolder { name: f.name.clone(), visible: None, opacity: Some(o), collapsed: None });
            }
            let l = ui.label(tl!("In"));
            let mut parent = f.parent.clone();
            egui::ComboBox::from_id_salt(("folder-parent", &f.name))
                .selected_text(parent.clone().unwrap_or_else(|| tl!("Top").to_string()))
                .width(90.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut parent, None, tl!("Top"));
                    for n in &names {
                        if *n != f.name && !app.engine.project.folder_would_loop(&f.name, n) {
                            ui.selectable_value(&mut parent, Some(n.clone()), n);
                        }
                    }
                })
                .response
                .labelled_by(l.id);
            if parent != f.parent {
                cmds.push(Command::SetFolderParent { name: f.name.clone(), parent });
            }
        });
    }
    for c in cmds {
        app.run(c);
    }
}
