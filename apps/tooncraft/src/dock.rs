//! The docked workspace (craft-ui docking): the canvas in the middle, the timeline under it, and
//! tabbed groups for colour, brush, layers and onion skin on the right. Panels can be moved,
//! tabbed together, floated and closed; the Window menu lists every panel and puts a closed one
//! back where it was (View ▸ Reset Panels restores the whole layout). The canvas stays put.

use std::collections::HashMap;

use craft_ui::docking::{Action, DockArea, DockStyle, Layout, Location, Node, Permissions};
use craft_ui::layout::{SplitAxis, SplitSize};

use crate::app::ToonApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Pane {
    Canvas,
    Timeline,
    Colour,
    Brush,
    Layers,
    Onion,
    Camera,
    Pegs,
    Library,
    Effects,
    Sound,
    LipSync,
    Timing,
    History,
    Drawings,
    Curves,
    SecondView,
    Navigator,
}

impl Pane {
    /// Every panel, in the Window menu's order.
    pub const ALL: [Pane; 18] = [
        Pane::Canvas,
        Pane::Timeline,
        Pane::Colour,
        Pane::Brush,
        Pane::Library,
        Pane::Drawings,
        Pane::Layers,
        Pane::Onion,
        Pane::Camera,
        Pane::Pegs,
        Pane::Effects,
        Pane::Sound,
        Pane::LipSync,
        Pane::Timing,
        Pane::History,
        Pane::Curves,
        Pane::SecondView,
        Pane::Navigator,
    ];

    /// The panel's title (from the panel registry).
    pub fn label(self) -> String {
        crate::panel_views::view(self).title()
    }

    /// The panel a `--panel` name means (the canvas for an unknown name).
    pub fn from_name(name: &str) -> Pane {
        crate::panel_views::by_name(name).unwrap_or(Pane::Canvas)
    }

    pub fn permissions(self) -> Permissions {
        // the canvas is the one panel that cannot be closed
        let closable = self != Pane::Canvas;
        match self {
            Pane::Canvas => Permissions { movable: false, floatable: false, closable, accepts_tabs: false, accepts_splits: true },
            _ => Permissions { closable, ..Permissions::default() },
        }
    }
}

pub struct Workspace {
    pub layout: Layout<Pane>,
    /// The canvas viewport last frame (keyboard zoom anchors on it).
    pub canvas_rect: egui::Rect,
    raise: Option<Pane>,
    /// Where each closed panel was, so the Window menu puts it back there.
    closed: HashMap<Pane, Location<Pane>>,
}

fn tabs(panels: &[Pane], active: usize) -> Node<Pane> {
    Node::Tabs { panels: panels.to_vec(), active }
}

impl Default for Workspace {
    fn default() -> Self {
        let centre = Node::Split {
            axis: SplitAxis::Vertical,
            size: SplitSize::FixedSecond(236.0),
            first: Box::new(tabs(&[Pane::Canvas], 0)),
            second: Box::new(tabs(&[Pane::Timeline, Pane::Curves], 0)),
        };
        let right = Node::Split {
            axis: SplitAxis::Vertical,
            size: SplitSize::Ratio(0.5),
            first: Box::new(tabs(&[Pane::Colour, Pane::Brush, Pane::Library, Pane::Drawings, Pane::SecondView], 0)),
            second: Box::new(tabs(
                &[Pane::Layers, Pane::Navigator, Pane::Onion, Pane::Camera, Pane::Pegs, Pane::Effects, Pane::Sound, Pane::LipSync, Pane::Timing, Pane::History],
                0,
            )),
        };
        let root = Node::Split { axis: SplitAxis::Horizontal, size: SplitSize::FixedSecond(300.0), first: Box::new(centre), second: Box::new(right) };
        Self {
            layout: Layout { root: Some(root), floating: Vec::new() },
            canvas_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            raise: None,
            closed: HashMap::new(),
        }
    }
}

impl Workspace {
    /// Is the panel in the layout (docked or floating)?
    pub fn is_open(&self, pane: Pane) -> bool {
        self.layout.contains(&pane)
    }

    /// Close a panel, remembering where it was. The canvas stays.
    pub fn close(&mut self, pane: Pane) -> bool {
        if !self.is_open(pane) || !pane.permissions().closable {
            return false;
        }
        let at = self.layout.location(&pane).ok();
        match self.layout.apply_with_permissions(Action::Close { panel: pane }, |p| p.permissions()) {
            Ok(_) => {
                if let Some(at) = at {
                    self.closed.insert(pane, at);
                }
                true
            }
            Err(e) => {
                log::warn!("panel close refused: {e:?}");
                false
            }
        }
    }

    /// Put a closed panel back where it was (or in the first panel group when that place is
    /// gone) and bring it to the front.
    pub fn open(&mut self, pane: Pane) -> bool {
        if !self.is_open(pane) {
            let back = self.closed.remove(&pane).is_some_and(|at| self.layout.restore(pane, &at).is_ok());
            if !back && let Err(e) = self.layout.apply_with_permissions(Action::Open { panel: pane, anchor: None }, |p| p.permissions()) {
                log::warn!("panel open refused: {e:?}");
                return false;
            }
        }
        self.raise(pane);
        true
    }

    /// Put a panel back in its place in the default layout (a closed one opens there) and bring
    /// it to the front. False when the panel cannot move (the canvas) or its place is gone.
    pub fn reset_panel(&mut self, pane: Pane) -> bool {
        let Ok(home) = Workspace::default().layout.location(&pane) else { return self.open(pane) };
        if self.layout.location(&pane).is_ok_and(|at| at == home) {
            self.raise(pane);
            return true;
        }
        if self.is_open(pane) && !self.close(pane) {
            return false;
        }
        self.closed.insert(pane, home);
        self.open(pane)
    }

    /// Bring a panel's tab to the front next frame (opening it first when it was closed).
    pub fn raise(&mut self, pane: Pane) {
        if !self.is_open(pane) {
            self.open(pane);
            return;
        }
        if let Some(root) = self.layout.root.as_mut() {
            activate(root, pane);
        }
        self.raise = Some(pane);
    }
}

fn activate(node: &mut Node<Pane>, pane: Pane) -> bool {
    match node {
        Node::Tabs { panels, active } => match panels.iter().position(|p| *p == pane) {
            Some(i) => {
                *active = i;
                true
            }
            None => false,
        },
        Node::Split { first, second, .. } => activate(first, pane) || activate(second, pane),
        _ => false,
    }
}

pub fn show(app: &mut ToonApp, ui: &mut egui::Ui) {
    let layout = app.workspace.layout.clone();
    let style = DockStyle::from_ui(ui);
    let mut area = DockArea::new(crate::panel_chrome::dock_id());
    if let Some(p) = app.workspace.raise.take() {
        area = area.raise_panel(p);
    }
    let ctx = ui.ctx().clone();
    let (mut out, queued, glyphs_only) = {
        let mut chrome = crate::panel_chrome::Chrome::new(app);
        let out = area.show_content(ui, &layout, &style, &mut chrome);
        (out, std::mem::take(&mut chrome.queued), chrome.glyphs_only())
    };
    out.actions.extend(queued);
    crate::panel_chrome::paint_tab_glyphs(app, &ctx, &glyphs_only);
    if let Some(e) = out.error {
        log::warn!("panel layout: {e:?}; restoring the default layout");
        app.workspace.layout = Workspace::default().layout;
        return;
    }
    for action in out.actions {
        // panels closed from their tab or window: remember where they were
        let closing: Vec<(Pane, Location<Pane>)> = app
            .workspace
            .layout
            .closed_by(&action)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| app.workspace.layout.location(&p).ok().map(|at| (p, at)))
            .collect();
        match app.workspace.layout.apply_with_permissions(action, |p| p.permissions()) {
            Ok(_) => app.workspace.closed.extend(closing),
            Err(e) => log::warn!("panel move refused: {e:?}"),
        }
    }
}

/// The Window menu: the workspaces, then every panel with a tick when it is open; picking one
/// opens or closes it.
pub fn window_menu(app: &mut ToonApp, ui: &mut egui::Ui) {
    ui.menu_button(tl!("Workspace"), |ui| crate::workspaces::menu(app, ui));
    ui.separator();
    for pane in Pane::ALL {
        if pane == Pane::Canvas {
            continue;
        }
        let mut open = app.workspace.is_open(pane);
        if ui.checkbox(&mut open, pane.label()).clicked() {
            if open {
                app.workspace.open(pane);
            } else {
                app.workspace.close(pane);
            }
        }
    }
    ui.separator();
    if ui.button(tl!("Reset Panels")).on_hover_text(tl!("Every panel back in its first place")).clicked() {
        app.workspace = Workspace::default();
        ui.close();
    }
}
