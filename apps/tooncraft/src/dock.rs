//! The docked workspace (craft-ui docking): the canvas in the middle, the timeline under it, and
//! tabbed groups for colour, brush, layers and onion skin on the right. Panels can be moved,
//! tabbed together and floated; none can be closed (View ▸ Reset Panels restores the layout).
//! The canvas stays put.

use craft_ui::docking::{DockArea, DockStyle, Layout, Node, Permissions};
use craft_ui::layout::{SplitAxis, SplitSize};

use crate::app::ToonApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Canvas,
    Timeline,
    Colour,
    Brush,
    Layers,
    Onion,
}

impl Pane {
    pub fn label(self) -> String {
        match self {
            Pane::Canvas => tl!("Canvas"),
            Pane::Timeline => tl!("Timeline"),
            Pane::Colour => tl!("Colour"),
            Pane::Brush => tl!("Brush"),
            Pane::Layers => tl!("Layers"),
            Pane::Onion => tl!("Onion Skin"),
        }
        .to_string()
    }

    pub fn from_name(name: &str) -> Pane {
        match name.to_ascii_lowercase().as_str() {
            "timeline" => Pane::Timeline,
            "colour" | "color" => Pane::Colour,
            "brush" => Pane::Brush,
            "onion" => Pane::Onion,
            "layers" => Pane::Layers,
            _ => Pane::Canvas,
        }
    }

    fn permissions(self) -> Permissions {
        let closable = false;
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
            second: Box::new(tabs(&[Pane::Timeline], 0)),
        };
        let right = Node::Split {
            axis: SplitAxis::Vertical,
            size: SplitSize::Ratio(0.5),
            first: Box::new(tabs(&[Pane::Colour, Pane::Brush], 0)),
            second: Box::new(tabs(&[Pane::Layers, Pane::Onion], 0)),
        };
        let root = Node::Split { axis: SplitAxis::Horizontal, size: SplitSize::FixedSecond(300.0), first: Box::new(centre), second: Box::new(right) };
        Self {
            layout: Layout { root: Some(root), floating: Vec::new() },
            canvas_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            raise: None,
        }
    }
}

impl Workspace {
    /// Bring a panel's tab to the front next frame.
    pub fn raise(&mut self, pane: Pane) {
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
    let mut area = DockArea::new(egui::Id::unique("tooncraft-dock"));
    if let Some(p) = app.workspace.raise.take() {
        area = area.raise_panel(p);
    }
    let out = area.show(
        ui,
        &layout,
        &style,
        |p| p.label(),
        |p| p.permissions(),
        |ui, pane| match pane {
            Pane::Canvas => crate::canvas::show(app, ui),
            Pane::Timeline => crate::timeline::show(app, ui),
            Pane::Colour => crate::panels::colour(app, ui),
            Pane::Brush => crate::panels::brush(app, ui),
            Pane::Layers => crate::panels::layers(app, ui),
            Pane::Onion => crate::panels::onion(app, ui),
        },
    );
    if let Some(e) = out.error {
        log::warn!("panel layout: {e:?}; restoring the default layout");
        app.workspace.layout = Workspace::default().layout;
        return;
    }
    for action in out.actions {
        if let Err(e) = app.workspace.layout.apply_with_permissions(action, |p| p.permissions()) {
            log::warn!("panel move refused: {e:?}");
        }
    }
}
