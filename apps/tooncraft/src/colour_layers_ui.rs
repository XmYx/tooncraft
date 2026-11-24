//! Line and colour art in the app: the layer right-click menu adds a colour layer below a line
//! layer (it follows the line layer's exposures, `tooncraft_doc::colour_art`), shows which line
//! layer a colour layer follows, and stops following. Every item is a menu button, so the menu
//! works from the keyboard (Shift+F10 or the Menu key on a layer row).

use egui::Ui;
use tooncraft_engine::Command;

use crate::app::ToonApp;

/// The colour-layer items of the layer menu for layer `li`.
pub fn menu_items(app: &mut ToonApp, ui: &mut Ui, li: usize) {
    let p = &app.engine.project;
    let Some(l) = p.layers.get(li) else { return };
    match p.line_layer_of(li).and_then(|i| p.layers.get(i)) {
        Some(line) => {
            let name = line.name.clone();
            ui.label(tlf!("Colour of {layer}", layer = name));
            if ui
                .button(tl!("Stop Following the Line Layer"))
                .on_hover_text(tl!("Keep this layer's drawings, but stop following the line layer's timing"))
                .clicked()
            {
                app.run(Command::SetColourOf { layer: li, line: None });
                ui.close();
            }
        }
        None if l.colour_of.is_none() => {
            let has = p.colour_layer_of(li).is_some();
            let hint = if has {
                tl!("This layer already has a colour layer; another one follows it too")
            } else {
                tl!("A layer below for colour that follows this layer's drawings and timing")
            };
            if ui.button(tl!("Add Colour Layer")).on_hover_text(hint).clicked() {
                if app.run(Command::AddColourLayer { layer: li }) {
                    // the new layer is at `li`, the line layer moved up one
                    app.layer = li;
                    app.set_status(tl!("Colour layer added: paint under the lines, it follows their timing"));
                }
                ui.close();
            }
        }
        None => {}
    }
}
