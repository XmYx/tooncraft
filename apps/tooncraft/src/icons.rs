//! ToonCraft's icon family: original line icons drawn in code with egui shapes (no icon files, no
//! traced shapes). Every glyph sits on a 16 px grid with a 1.5 px stroke, round ends and 2 px of
//! padding; the filled variant means "on" or "active" and the outline variant means off.
//!
//! [`paint`] draws an [`Icon`]; [`IconButton`] (or `ui.icon_button(..)` through [`IconUi`]) is the
//! button every toolbar, header and panel uses: it always carries an accessible name and a tooltip
//! with the shortcut.

use egui::{Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Widget, epaint::PathShape, vec2};

use crate::app::Tool;

#[allow(dead_code, reason = "the shared icon family: the UI-pass shell, canvas and menu code draws from all of it; the tests paint every glyph")]
/// Every glyph ToonCraft draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    // tools
    Pencil,
    Ink,
    Airbrush,
    Chalk,
    Eraser,
    Watercolour,
    Smudge,
    Fill,
    Picker,
    Select,
    Shape,
    Gradient,
    Hand,
    Transform,
    Text,
    Zoom,
    RotateView,
    // tool variants
    SelectTransform,
    SelectLasso,
    SelectRect,
    SelectEllipse,
    SelectWand,
    ShapeLine,
    ShapeRect,
    ShapeEllipse,
    GradientLinear,
    GradientRadial,
    // transport
    First,
    PrevDrawing,
    PrevFrame,
    Play,
    Pause,
    NextFrame,
    NextDrawing,
    Last,
    Loop,
    PlayAround,
    // view toggles
    Onion,
    LightTable,
    Grid,
    Mirror,
    Rulers,
    FadeOthers,
    SafeArea,
    // layer toggles
    Eye,
    Lock,
    LockAlpha,
    OnionInclude,
    Label,
    // panels
    Colour,
    Palettes,
    Brush,
    Layers,
    Library,
    History,
    Curves,
    Sound,
    Effects,
    Timing,
    Canvas,
    Timeline,
    Camera,
    LipSync,
    Drawings,
    SecondView,
    Navigator,
    // actions
    Swap,
    Add,
    Duplicate,
    Delete,
    Rename,
    More,
    Search,
    Undo,
    Redo,
    FlipH,
    FlipV,
    Rotate90,
    Cut,
    Copy,
    Paste,
    Peg,
    MergeDown,
    Folder,
    Settings,
    Theme,
    Close,
    ToolPalette,
    Radial,
    // cell marks
    MarkKey,
    MarkBreakdown,
    MarkInbetween,
}

#[allow(dead_code, reason = "the shared icon family: the UI-pass shell, canvas and menu code draws from all of it; the tests paint every glyph")]
impl Icon {
    pub const ALL: [Icon; 92] = [
        Icon::Pencil,
        Icon::Ink,
        Icon::Airbrush,
        Icon::Chalk,
        Icon::Eraser,
        Icon::Watercolour,
        Icon::Smudge,
        Icon::Fill,
        Icon::Picker,
        Icon::Select,
        Icon::Shape,
        Icon::Gradient,
        Icon::Hand,
        Icon::Transform,
        Icon::Text,
        Icon::Zoom,
        Icon::RotateView,
        Icon::SelectTransform,
        Icon::SelectLasso,
        Icon::SelectRect,
        Icon::SelectEllipse,
        Icon::SelectWand,
        Icon::ShapeLine,
        Icon::ShapeRect,
        Icon::ShapeEllipse,
        Icon::GradientLinear,
        Icon::GradientRadial,
        Icon::First,
        Icon::PrevDrawing,
        Icon::PrevFrame,
        Icon::Play,
        Icon::Pause,
        Icon::NextFrame,
        Icon::NextDrawing,
        Icon::Last,
        Icon::Loop,
        Icon::PlayAround,
        Icon::Onion,
        Icon::LightTable,
        Icon::Grid,
        Icon::Mirror,
        Icon::Rulers,
        Icon::FadeOthers,
        Icon::SafeArea,
        Icon::Eye,
        Icon::Lock,
        Icon::LockAlpha,
        Icon::OnionInclude,
        Icon::Label,
        Icon::Colour,
        Icon::Palettes,
        Icon::Brush,
        Icon::Layers,
        Icon::Library,
        Icon::History,
        Icon::Curves,
        Icon::Sound,
        Icon::Effects,
        Icon::Timing,
        Icon::Canvas,
        Icon::Timeline,
        Icon::Camera,
        Icon::LipSync,
        Icon::Drawings,
        Icon::SecondView,
        Icon::Navigator,
        Icon::Swap,
        Icon::Add,
        Icon::Duplicate,
        Icon::Delete,
        Icon::Rename,
        Icon::More,
        Icon::Search,
        Icon::Undo,
        Icon::Redo,
        Icon::FlipH,
        Icon::FlipV,
        Icon::Rotate90,
        Icon::Cut,
        Icon::Copy,
        Icon::Paste,
        Icon::Peg,
        Icon::MergeDown,
        Icon::Folder,
        Icon::Settings,
        Icon::Theme,
        Icon::Close,
        Icon::ToolPalette,
        Icon::Radial,
        Icon::MarkKey,
        Icon::MarkBreakdown,
        Icon::MarkInbetween,
    ];

    /// What the glyph shows, translated: the fallback accessible name of a button that has no
    /// label of its own.
    pub fn label(self) -> String {
        let s = match self {
            Icon::Pencil => tl!("Pencil"),
            Icon::Ink => tl!("Ink pen"),
            Icon::Airbrush => tl!("Airbrush"),
            Icon::Chalk => tl!("Chalk"),
            Icon::Eraser => tl!("Eraser"),
            Icon::Watercolour => tl!("Watercolour"),
            Icon::Smudge => tl!("Smudge"),
            Icon::Fill => tl!("Paint bucket"),
            Icon::Picker => tl!("Colour picker"),
            Icon::Select => tl!("Select"),
            Icon::Shape => tl!("Shapes"),
            Icon::Gradient => tl!("Gradient"),
            Icon::Hand => tl!("Hand"),
            Icon::Transform => tl!("Transform layer"),
            Icon::Text => tl!("Text"),
            Icon::Zoom => tl!("Zoom"),
            Icon::RotateView => tl!("Rotate view"),
            Icon::SelectTransform => tl!("Transform selection"),
            Icon::Canvas => tl!("Canvas"),
            Icon::Timeline => tl!("Timeline"),
            Icon::Camera => tl!("Camera"),
            Icon::LipSync => tl!("Lip sync"),
            Icon::Drawings => tl!("Drawings"),
            Icon::SecondView => tl!("Second view"),
            Icon::Navigator => tl!("Navigator"),
            Icon::Swap => tl!("Swap colours"),
            Icon::SelectLasso => tl!("Lasso"),
            Icon::SelectRect => tl!("Rectangle"),
            Icon::SelectEllipse => tl!("Ellipse"),
            Icon::SelectWand => tl!("Similar pixels"),
            Icon::ShapeLine => tl!("Line"),
            Icon::ShapeRect => tl!("Rectangle"),
            Icon::ShapeEllipse => tl!("Ellipse"),
            Icon::GradientLinear => tl!("Linear"),
            Icon::GradientRadial => tl!("Radial"),
            Icon::First => tl!("First frame"),
            Icon::PrevDrawing => tl!("Previous drawing"),
            Icon::PrevFrame => tl!("Previous frame"),
            Icon::Play => tl!("Play"),
            Icon::Pause => tl!("Pause"),
            Icon::NextFrame => tl!("Next frame"),
            Icon::NextDrawing => tl!("Next drawing"),
            Icon::Last => tl!("Last frame"),
            Icon::Loop => tl!("Loop"),
            Icon::PlayAround => tl!("Play around this frame"),
            Icon::Onion => tl!("Onion skin"),
            Icon::LightTable => tl!("Light table"),
            Icon::Grid => tl!("Grid"),
            Icon::Mirror => tl!("Mirror view"),
            Icon::Rulers => tl!("Rulers"),
            Icon::FadeOthers => tl!("Fade other layers"),
            Icon::SafeArea => tl!("Safe area"),
            Icon::Eye => tl!("Show"),
            Icon::Lock => tl!("Lock"),
            Icon::LockAlpha => tl!("Lock transparency"),
            Icon::OnionInclude => tl!("Include in onion skin"),
            Icon::Label => tl!("Label colour"),
            Icon::Colour => tl!("Colour"),
            Icon::Palettes => tl!("Palettes"),
            Icon::Brush => tl!("Brush"),
            Icon::Layers => tl!("Layers"),
            Icon::Library => tl!("Library"),
            Icon::History => tl!("History"),
            Icon::Curves => tl!("Curves"),
            Icon::Sound => tl!("Sound"),
            Icon::Effects => tl!("Effects"),
            Icon::Timing => tl!("Timing"),
            Icon::Add => tl!("Add"),
            Icon::Duplicate => tl!("Duplicate"),
            Icon::Delete => tl!("Delete"),
            Icon::Rename => tl!("Rename"),
            Icon::More => tl!("More"),
            Icon::Search => tl!("Search"),
            Icon::Undo => tl!("Undo"),
            Icon::Redo => tl!("Redo"),
            Icon::FlipH => tl!("Flip horizontally"),
            Icon::FlipV => tl!("Flip vertically"),
            Icon::Rotate90 => tl!("Rotate 90°"),
            Icon::Cut => tl!("Cut"),
            Icon::Copy => tl!("Copy"),
            Icon::Paste => tl!("Paste"),
            Icon::Peg => tl!("Peg"),
            Icon::MergeDown => tl!("Merge down"),
            Icon::Folder => tl!("Folder"),
            Icon::Settings => tl!("Settings"),
            Icon::Theme => tl!("Light or dark theme"),
            Icon::Close => tl!("Close"),
            Icon::ToolPalette => tl!("Tool palette"),
            Icon::Radial => tl!("Radial menu"),
            Icon::MarkKey => tl!("Key"),
            Icon::MarkBreakdown => tl!("Breakdown"),
            Icon::MarkInbetween => tl!("In-between"),
        };
        s.to_string()
    }
}

impl From<Tool> for Icon {
    fn from(t: Tool) -> Self {
        match t {
            Tool::Pencil => Icon::Pencil,
            Tool::Ink => Icon::Ink,
            Tool::Airbrush => Icon::Airbrush,
            Tool::Chalk => Icon::Chalk,
            Tool::Eraser => Icon::Eraser,
            Tool::Watercolour => Icon::Watercolour,
            Tool::Smudge => Icon::Smudge,
            Tool::Fill => Icon::Fill,
            Tool::Picker => Icon::Picker,
            Tool::Select => Icon::Select,
            Tool::Shape => Icon::Shape,
            Tool::Gradient => Icon::Gradient,
            Tool::Hand => Icon::Hand,
            Tool::Transform => Icon::Transform,
            Tool::Text => Icon::Text,
            Tool::Zoom => Icon::Zoom,
            Tool::RotateView => Icon::RotateView,
        }
    }
}

/// The glyph of a panel's tab.
pub fn for_pane(p: crate::dock::Pane) -> Icon {
    crate::panel_views::view(p).icon
}

/// Draw `icon` centred in `rect`; `filled` is the "on" variant.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32, filled: bool) {
    p.extend(shapes(rect, icon, color, filled));
}

/// The shapes of `icon` centred in `rect`. Glyph coordinates run from -1 to 1 over the 12 px
/// drawing area of the 16 px grid (2 px padding on every side).
pub fn shapes(rect: Rect, icon: Icon, color: Color32, filled: bool) -> Vec<Shape> {
    let side = rect.width().min(rect.height());
    let side = if side.is_finite() { side.max(1.0) } else { crate::theme::metrics::ICON };
    let c = if rect.center().is_finite() { rect.center() } else { Pos2::ZERO };
    let mut g = Glyph { c, s: side * 6.0 / 16.0, color, stroke: Stroke::new((1.5 * side / 16.0).max(1.0), color), filled, out: Vec::new() };
    g.draw(icon);
    g.out
}

/// Draws one glyph in the -1..1 square.
struct Glyph {
    c: Pos2,
    s: f32,
    color: Color32,
    stroke: Stroke,
    filled: bool,
    out: Vec<Shape>,
}

type P = (f32, f32);

impl Glyph {
    fn at(&self, p: P) -> Pos2 {
        self.c + vec2(p.0 * self.s, p.1 * self.s)
    }

    fn cap(&mut self, p: Pos2) {
        self.out.push(Shape::circle_filled(p, self.stroke.width * 0.5, self.color));
    }

    fn line(&mut self, a: P, b: P) {
        let (a, b) = (self.at(a), self.at(b));
        self.out.push(Shape::line_segment([a, b], self.stroke));
        self.cap(a);
        self.cap(b);
    }

    /// An open path with round joins and ends.
    fn polyline(&mut self, pts: &[P]) {
        let pts: Vec<Pos2> = pts.iter().map(|&p| self.at(p)).collect();
        for p in &pts {
            self.cap(*p);
        }
        self.out.push(Shape::line(pts, self.stroke));
    }

    /// A closed convex outline, filled in the "on" variant.
    fn body(&mut self, pts: &[P]) {
        let pts: Vec<Pos2> = pts.iter().map(|&p| self.at(p)).collect();
        let fill = if self.filled { self.color } else { Color32::TRANSPARENT };
        self.out.push(Shape::Path(PathShape::convex_polygon(pts, fill, self.stroke)));
    }

    /// A closed outline that is never filled.
    fn outline(&mut self, pts: &[P]) {
        let pts: Vec<Pos2> = pts.iter().map(|&p| self.at(p)).collect();
        self.out.push(Shape::closed_line(pts, self.stroke));
    }

    /// A solid shape (always filled, no outline).
    fn solid(&mut self, pts: &[P]) {
        let pts: Vec<Pos2> = pts.iter().map(|&p| self.at(p)).collect();
        self.out.push(Shape::convex_polygon(pts, self.color, Stroke::NONE));
    }

    fn rect(&mut self, a: P, b: P) {
        self.body(&[a, (b.0, a.1), b, (a.0, b.1)]);
    }

    fn rect_outline(&mut self, a: P, b: P) {
        self.outline(&[a, (b.0, a.1), b, (a.0, b.1)]);
    }

    /// A circle, filled in the "on" variant.
    fn circle(&mut self, c: P, r: f32) {
        let fill = if self.filled { self.color } else { Color32::TRANSPARENT };
        self.out.push(Shape::Circle(egui::epaint::CircleShape { center: self.at(c), radius: r * self.s, fill, stroke: self.stroke }));
    }

    fn ring(&mut self, c: P, r: f32) {
        self.out.push(Shape::circle_stroke(self.at(c), r * self.s, self.stroke));
    }

    fn dot(&mut self, c: P, r: f32) {
        self.out.push(Shape::circle_filled(self.at(c), r * self.s, self.color));
    }

    /// The points of an ellipse arc from `a0` to `a1` (radians; 0 = right, increasing clockwise
    /// on screen).
    fn arc_pts(c: P, rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<P> {
        let n = 18;
        (0..=n)
            .map(|i| {
                let a = a0 + (a1 - a0) * i as f32 / n as f32;
                (c.0 + a.cos() * rx, c.1 + a.sin() * ry)
            })
            .collect()
    }

    /// A circular arc; returns its end point.
    fn arc(&mut self, c: P, r: f32, a0: f32, a1: f32) -> P {
        let pts = Self::arc_pts(c, r, r, a0, a1);
        self.polyline(&pts);
        (c.0 + a1.cos() * r, c.1 + a1.sin() * r)
    }

    /// An open arrow head at `tip`, pointing along `dir` (radians).
    fn head(&mut self, tip: P, dir: f32) {
        let k = 0.4;
        let l = (tip.0 - (dir - 0.7).cos() * k, tip.1 - (dir - 0.7).sin() * k);
        let r = (tip.0 - (dir + 0.7).cos() * k, tip.1 - (dir + 0.7).sin() * k);
        self.polyline(&[l, tip, r]);
    }

    /// Dashes along the segments of a closed outline.
    fn dashed(&mut self, pts: &[P], dashes: usize) {
        let n = pts.len();
        for i in 0..n {
            let (p, q) = (pts[i], pts[(i + 1) % n]);
            for k in 0..dashes {
                let t0 = k as f32 / dashes as f32;
                let t1 = t0 + 0.55 / dashes as f32;
                let lerp = |t: f32| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
                let (s, e) = (self.at(lerp(t0)), self.at(lerp(t1)));
                self.out.push(Shape::line_segment([s, e], self.stroke));
            }
        }
    }

    fn draw(&mut self, icon: Icon) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
        match icon {
            // ── tools ──
            Icon::Pencil => {
                self.body(&[(-0.55, 0.85), (-0.85, 0.55), (0.55, -0.85), (0.85, -0.55)]);
                self.polyline(&[(-0.85, 0.55), (-1.0, 1.0), (-0.55, 0.85)]);
                self.line((0.3, -0.6), (0.6, -0.3));
            }
            Icon::Ink => {
                self.body(&[(0.0, -1.0), (0.6, -0.1), (0.0, 1.0), (-0.6, -0.1)]);
                self.line((0.0, 0.2), (0.0, 0.95));
                self.dot((0.0, -0.1), 0.16);
            }
            Icon::Airbrush => {
                self.circle((0.0, 0.0), 0.3);
                for (i, r) in [0.7f32, 0.92, 0.8, 0.98, 0.75, 0.95, 0.85, 0.66].into_iter().enumerate() {
                    let a = i as f32 * TAU / 8.0 + 0.3;
                    self.dot((a.cos() * r * 0.95, a.sin() * r * 0.95), 0.1);
                }
            }
            Icon::Chalk => {
                self.rect((-0.4, -0.9), (0.4, 0.9));
                for k in 0..3 {
                    let y = -0.5 + k as f32 * 0.5;
                    self.line((-0.18, y + 0.1), (0.18, y - 0.1));
                }
            }
            Icon::Eraser => {
                self.body(&[(-1.0, 0.3), (-0.2, -0.6), (1.0, 0.4), (0.2, 1.0)]);
                self.line((-0.6, -0.15), (0.6, 0.7));
            }
            Icon::Watercolour => {
                self.body(&[(0.0, -1.0), (0.55, 0.0), (0.4, 0.45), (0.0, 0.65), (-0.4, 0.45), (-0.55, 0.0)]);
                self.line((-0.95, 0.95), (0.95, 0.95));
            }
            Icon::Smudge => {
                self.circle((0.45, -0.45), 0.42);
                for k in 0..3 {
                    let y = -0.05 + k as f32 * 0.35;
                    self.line((0.15 - k as f32 * 0.15, y + 0.15), (-0.95, y + 0.45));
                }
            }
            Icon::Fill => {
                self.body(&[(-0.8, -0.3), (0.1, -1.0), (0.8, -0.1), (-0.1, 0.6)]);
                self.dot((0.7, 0.7), 0.24);
            }
            Icon::Picker => {
                self.line((-0.9, 0.9), (0.25, -0.25));
                self.circle((0.5, -0.5), 0.38);
            }
            Icon::Select | Icon::SelectRect => self.dashed(&[(-0.9, -0.75), (0.9, -0.75), (0.9, 0.75), (-0.9, 0.75)], 3),
            Icon::Shape => {
                self.rect((-0.95, -0.95), (0.3, 0.3));
                self.circle((0.4, 0.4), 0.55);
            }
            Icon::Gradient | Icon::GradientLinear => {
                for k in 0..4 {
                    let t = k as f32 / 4.0;
                    let x0 = -0.9 + t * 1.8;
                    let r = Rect::from_min_max(self.at((x0, -0.7)), self.at((x0 + 0.45, 0.7)));
                    self.out.push(Shape::rect_filled(r, 0.0, self.color.gamma_multiply(1.0 - t * 0.8)));
                }
                self.rect_outline((-0.9, -0.7), (0.9, 0.7));
            }
            Icon::Hand => {
                for k in 0..4 {
                    let x = -0.6 + k as f32 * 0.4;
                    self.line((x, 0.1), (x, -0.85 + (k as f32 - 1.5).abs() * 0.15));
                }
                self.body(&[(-0.75, 0.1), (0.75, 0.1), (0.5, 0.95), (-0.5, 0.95)]);
            }
            Icon::Transform => {
                self.rect_outline((-0.75, -0.4), (0.75, 0.85));
                for (x, y) in [(-0.75, -0.4), (0.75, -0.4), (0.75, 0.85), (-0.75, 0.85)] {
                    self.solid(&[(x - 0.15, y - 0.15), (x + 0.15, y - 0.15), (x + 0.15, y + 0.15), (x - 0.15, y + 0.15)]);
                }
                self.line((0.0, -0.4), (0.0, -0.7));
                self.circle((0.0, -0.85), 0.13);
            }
            Icon::Text => {
                self.line((-0.8, -0.85), (0.8, -0.85));
                self.line((0.0, -0.85), (0.0, 0.85));
                self.line((-0.3, 0.85), (0.3, 0.85));
                self.line((-0.8, -0.85), (-0.8, -0.6));
                self.line((0.8, -0.85), (0.8, -0.6));
            }
            Icon::Zoom => {
                self.circle((-0.2, -0.2), 0.6);
                self.line((0.25, 0.25), (0.85, 0.85));
                self.line((-0.45, -0.2), (0.05, -0.2));
                self.line((-0.2, -0.45), (-0.2, 0.05));
            }
            Icon::RotateView => {
                let e = self.arc((0.0, 0.1), 0.8, PI * 0.95, TAU + 0.1);
                self.head(e, 0.1 + FRAC_PI_2);
                self.dot((0.0, 0.1), 0.16);
            }
            Icon::SelectTransform => {
                self.dashed(&[(-0.75, -0.75), (0.75, -0.75), (0.75, 0.75), (-0.75, 0.75)], 2);
                for (x, y) in [(-0.75, -0.75), (0.75, -0.75), (0.75, 0.75), (-0.75, 0.75)] {
                    self.solid(&[(x - 0.17, y - 0.17), (x + 0.17, y - 0.17), (x + 0.17, y + 0.17), (x - 0.17, y + 0.17)]);
                }
            }
            Icon::Canvas => {
                self.rect((-0.95, -0.7), (0.95, 0.7));
                self.line((-0.5, 0.95), (0.5, 0.95));
            }
            Icon::Timeline => {
                self.rect_outline((-0.95, -0.8), (0.95, 0.8));
                self.line((-0.95, -0.3), (0.95, -0.3));
                self.solid(&[(-0.7, 0.0), (-0.1, 0.0), (-0.1, 0.5), (-0.7, 0.5)]);
                self.line((0.3, -0.8), (0.3, 0.8));
            }
            Icon::Camera => {
                self.rect((-0.95, -0.55), (0.35, 0.65));
                self.body(&[(0.35, 0.05), (0.95, -0.4), (0.95, 0.5)]);
            }
            Icon::LipSync => {
                let mut top = Self::arc_pts((0.0, 0.9), 1.1, 1.0, -PI * 0.8, -PI * 0.2);
                top.extend(Self::arc_pts((0.0, -0.3), 0.95, 0.75, PI * 0.15, PI * 0.85));
                self.outline(&top);
                self.line((-0.6, 0.12), (0.6, 0.12));
            }
            Icon::SecondView => {
                // two views of one picture, the second in front
                self.rect_outline((-0.95, -0.8), (0.35, 0.3));
                self.rect_outline((-0.35, -0.3), (0.95, 0.8));
                self.dot((0.3, 0.25), 0.18);
            }
            Icon::Navigator => {
                // the whole picture with the part on screen marked
                self.rect_outline((-0.95, -0.75), (0.95, 0.75));
                self.rect((-0.2, -0.35), (0.55, 0.25));
            }
            Icon::Drawings => {
                self.rect_outline((-0.95, -0.95), (0.45, 0.45));
                self.rect((-0.45, -0.45), (0.95, 0.95));
                let pts = Self::arc_pts((0.25, 0.45), 0.4, 0.3, PI, TAU);
                self.polyline(&pts);
            }
            Icon::Swap => {
                let e = self.arc((-0.25, 0.25), 0.7, -FRAC_PI_2, 0.0);
                self.head(e, FRAC_PI_2);
                self.head((-0.25, -0.45), PI);
            }
            Icon::SelectLasso => {
                let pts = Self::arc_pts((0.0, -0.25), 0.9, 0.6, 2.0, 2.0 + TAU * 0.85);
                self.polyline(&pts);
                let end = pts.last().copied().unwrap_or((0.0, 0.0));
                self.polyline(&[end, (-0.25, 0.6), (-0.45, 0.95)]);
            }
            Icon::SelectEllipse => {
                let n = 10;
                for k in 0..n {
                    let a0 = k as f32 / n as f32 * TAU;
                    let pts = Self::arc_pts((0.0, 0.0), 0.92, 0.7, a0, a0 + TAU / n as f32 * 0.55);
                    let pts: Vec<Pos2> = pts.iter().map(|&p| self.at(p)).collect();
                    self.out.push(Shape::line(pts, self.stroke));
                }
            }
            Icon::SelectWand => {
                self.line((-0.9, 0.9), (0.15, -0.15));
                self.body(&[(0.55, -1.0), (0.68, -0.68), (1.0, -0.55), (0.68, -0.42), (0.55, -0.1), (0.42, -0.42), (0.1, -0.55), (0.42, -0.68)]);
            }
            Icon::ShapeLine => {
                self.line((-0.8, 0.8), (0.8, -0.8));
                self.dot((-0.8, 0.8), 0.17);
                self.dot((0.8, -0.8), 0.17);
            }
            Icon::ShapeRect => self.rect((-0.9, -0.7), (0.9, 0.7)),
            Icon::ShapeEllipse => {
                let mut pts = Self::arc_pts((0.0, 0.0), 0.92, 0.7, 0.0, TAU);
                pts.pop();
                self.body(&pts);
            }
            Icon::GradientRadial => {
                for (k, r) in [0.92f32, 0.62, 0.32].into_iter().enumerate() {
                    self.out.push(Shape::circle_filled(self.at((0.0, 0.0)), r * self.s, self.color.gamma_multiply(0.3 + 0.2 * k as f32)));
                }
                self.ring((0.0, 0.0), 0.92);
            }
            // ── transport ──
            Icon::First => {
                self.line((-0.75, -0.75), (-0.75, 0.75));
                self.solid(&[(0.75, -0.75), (0.75, 0.75), (-0.45, 0.0)]);
            }
            Icon::PrevDrawing => {
                self.solid(&[(0.0, -0.7), (0.0, 0.7), (-0.95, 0.0)]);
                self.solid(&[(0.95, -0.7), (0.95, 0.7), (0.0, 0.0)]);
            }
            Icon::PrevFrame => self.polyline(&[(0.35, -0.8), (-0.4, 0.0), (0.35, 0.8)]),
            Icon::Play => self.solid(&[(-0.6, -0.9), (0.85, 0.0), (-0.6, 0.9)]),
            Icon::Pause => {
                self.solid(&[(-0.65, -0.85), (-0.2, -0.85), (-0.2, 0.85), (-0.65, 0.85)]);
                self.solid(&[(0.2, -0.85), (0.65, -0.85), (0.65, 0.85), (0.2, 0.85)]);
            }
            Icon::NextFrame => self.polyline(&[(-0.35, -0.8), (0.4, 0.0), (-0.35, 0.8)]),
            Icon::NextDrawing => {
                self.solid(&[(0.0, -0.7), (0.0, 0.7), (0.95, 0.0)]);
                self.solid(&[(-0.95, -0.7), (-0.95, 0.7), (0.0, 0.0)]);
            }
            Icon::Last => {
                self.line((0.75, -0.75), (0.75, 0.75));
                self.solid(&[(-0.75, -0.75), (-0.75, 0.75), (0.45, 0.0)]);
            }
            Icon::Loop => {
                // two arcs chasing each other round
                let e = self.arc((0.0, 0.0), 0.8, PI + 0.45, TAU - 0.2);
                self.head(e, -0.2 + FRAC_PI_2);
                let e = self.arc((0.0, 0.0), 0.8, 0.45, PI - 0.2);
                self.head(e, PI - 0.2 + FRAC_PI_2);
            }
            Icon::PlayAround => {
                self.line((0.0, -0.95), (0.0, 0.95));
                self.polyline(&[(-0.35, -0.4), (-0.85, 0.0), (-0.35, 0.4)]);
                self.polyline(&[(0.35, -0.4), (0.85, 0.0), (0.35, 0.4)]);
            }
            // ── view toggles ──
            Icon::Onion => {
                self.ring((-0.35, 0.0), 0.6);
                self.circle((0.35, 0.0), 0.6);
            }
            Icon::LightTable => {
                self.rect((-0.95, -0.45), (0.95, 0.8));
                self.line((-0.5, -0.95), (-0.5, -0.75));
                self.line((0.0, -1.0), (0.0, -0.75));
                self.line((0.5, -0.95), (0.5, -0.75));
            }
            Icon::Grid => {
                self.rect_outline((-0.9, -0.9), (0.9, 0.9));
                for t in [-0.3, 0.3] {
                    self.line((t, -0.9), (t, 0.9));
                    self.line((-0.9, t), (0.9, t));
                }
            }
            Icon::Mirror | Icon::FlipH => {
                self.line((0.0, -0.95), (0.0, 0.95));
                self.body(&[(-0.25, -0.6), (-0.25, 0.6), (-0.95, 0.6)]);
                self.outline(&[(0.25, -0.6), (0.25, 0.6), (0.95, 0.6)]);
                if icon == Icon::FlipH {
                    self.dot((-0.6, -0.85), 0.1);
                    self.dot((0.6, -0.85), 0.1);
                }
            }
            Icon::Rulers => {
                self.body(&[(-0.95, 0.3), (0.3, -0.95), (0.95, -0.3), (-0.3, 0.95)]);
                for k in 0..4 {
                    // ticks along the upper edge
                    let t = -0.55 + k as f32 * 0.35;
                    let base = (t, -t - 0.65);
                    let l = if k % 2 == 0 { 0.32 } else { 0.18 };
                    self.line(base, (base.0 + l * 0.7, base.1 + l * 0.7));
                }
            }
            Icon::FadeOthers => {
                let r = Rect::from_min_max(self.at((-0.95, -0.95)), self.at((0.35, 0.35)));
                self.out.push(Shape::rect_filled(r, 1.0, self.color.gamma_multiply(0.35)));
                self.rect((-0.35, -0.35), (0.95, 0.95));
            }
            Icon::SafeArea => {
                self.rect_outline((-0.95, -0.8), (0.95, 0.8));
                self.dashed(&[(-0.55, -0.42), (0.55, -0.42), (0.55, 0.42), (-0.55, 0.42)], 2);
            }
            // ── layer toggles ──
            Icon::Eye => {
                let mut all = Self::arc_pts((0.0, 0.75), 1.25, 1.25, -PI * 0.78, -PI * 0.22);
                all.extend(Self::arc_pts((0.0, -0.75), 1.25, 1.25, PI * 0.22, PI * 0.78));
                self.outline(&all);
                self.circle((0.0, 0.0), 0.28);
            }
            Icon::Lock => {
                self.rect((-0.7, -0.1), (0.7, 0.95));
                self.arc((0.0, -0.45), 0.42, PI, TAU);
                self.line((-0.42, -0.45), (-0.42, -0.1));
                self.line((0.42, -0.45), (0.42, -0.1));
            }
            Icon::LockAlpha => {
                self.rect_outline((-0.95, -0.1), (0.5, 0.95));
                self.arc((-0.22, -0.45), 0.4, PI, TAU);
                self.line((-0.62, -0.45), (-0.62, -0.1));
                self.line((0.18, -0.45), (0.18, -0.1));
                // a checker: the transparency it keeps
                self.solid(&[(-0.95, -0.1), (-0.22, -0.1), (-0.22, 0.42), (-0.95, 0.42)]);
                self.solid(&[(-0.22, 0.42), (0.5, 0.42), (0.5, 0.95), (-0.22, 0.95)]);
                if self.filled {
                    self.dot((0.8, -0.7), 0.18);
                } else {
                    self.ring((0.8, -0.7), 0.15);
                }
            }
            Icon::OnionInclude => {
                self.ring((-0.3, 0.15), 0.55);
                self.circle((0.3, 0.15), 0.55);
                self.line((0.5, -0.8), (0.95, -0.8));
                self.line((0.72, -1.0), (0.72, -0.6));
            }
            Icon::Label => self.body(&[(-0.9, -0.6), (0.4, -0.6), (0.95, 0.0), (0.4, 0.6), (-0.9, 0.6)]),
            // ── panels ──
            Icon::Colour => {
                self.ring((0.0, 0.0), 0.92);
                let mut half = Self::arc_pts((0.0, 0.0), 0.92, 0.92, -FRAC_PI_2, FRAC_PI_2);
                half.push((0.0, -0.92));
                self.solid(&half);
            }
            Icon::Palettes => {
                for (x, y) in [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)] {
                    self.rect((x - 0.36, y - 0.36), (x + 0.36, y + 0.36));
                }
            }
            Icon::Brush => {
                self.line((0.9, -0.9), (-0.05, 0.05));
                self.body(&[(-0.05, 0.05), (0.15, 0.25), (-0.3, 0.95), (-0.95, 0.95), (-0.65, 0.35)]);
            }
            Icon::Layers => {
                self.body(&[(0.0, -0.95), (0.95, -0.45), (0.0, 0.05), (-0.95, -0.45)]);
                self.polyline(&[(-0.95, 0.05), (0.0, 0.5), (0.95, 0.05)]);
                self.polyline(&[(-0.95, 0.5), (0.0, 0.95), (0.95, 0.5)]);
            }
            Icon::Library => {
                self.rect((-0.95, -0.9), (-0.45, 0.9));
                self.rect((-0.25, -0.9), (0.2, 0.9));
                self.body(&[(0.4, -0.8), (0.7, -0.9), (0.95, 0.8), (0.65, 0.9)]);
            }
            Icon::History => {
                self.arc((0.0, 0.0), 0.88, -PI * 0.9, PI * 0.75);
                let tip = ((-PI * 0.9).cos() * 0.88, (-PI * 0.9).sin() * 0.88);
                self.head(tip, -PI * 0.9 - FRAC_PI_2);
                self.polyline(&[(0.0, -0.5), (0.0, 0.0), (0.4, 0.3)]);
            }
            Icon::Curves => {
                let n = 16;
                let pts: Vec<P> = (0..=n)
                    .map(|i| i as f32 / n as f32)
                    .map(|t| {
                        let e = t * t * (3.0 - 2.0 * t);
                        (-0.85 + 1.7 * t, 0.8 - 1.6 * e)
                    })
                    .collect();
                self.polyline(&pts);
                self.dot((-0.85, 0.8), 0.17);
                self.dot((0.85, -0.8), 0.17);
            }
            Icon::Sound => {
                self.body(&[(-0.95, -0.35), (-0.5, -0.35), (0.05, -0.85), (0.05, 0.85), (-0.5, 0.35), (-0.95, 0.35)]);
                self.arc((0.15, 0.0), 0.42, -0.8, 0.8);
                self.arc((0.15, 0.0), 0.78, -0.8, 0.8);
            }
            Icon::Effects => {
                self.body(&[(0.0, -1.0), (0.22, -0.22), (1.0, 0.0), (0.22, 0.22), (0.0, 1.0), (-0.22, 0.22), (-1.0, 0.0), (-0.22, -0.22)]);
            }
            Icon::Timing => {
                self.circle((0.0, 0.15), 0.78);
                self.line((0.0, 0.15), (0.0, -0.3));
                self.line((0.0, 0.15), (0.35, 0.35));
                self.line((-0.25, -0.95), (0.25, -0.95));
            }
            // ── actions ──
            Icon::Add => {
                self.line((0.0, -0.85), (0.0, 0.85));
                self.line((-0.85, 0.0), (0.85, 0.0));
            }
            Icon::Duplicate | Icon::Copy => {
                self.rect_outline((-0.95, -0.95), (0.35, 0.35));
                self.rect((-0.35, -0.35), (0.95, 0.95));
                if icon == Icon::Duplicate {
                    self.line((0.3, 0.05), (0.3, 0.55));
                    self.line((0.05, 0.3), (0.55, 0.3));
                }
            }
            Icon::Delete => {
                self.line((-0.9, -0.6), (0.9, -0.6));
                self.polyline(&[(-0.3, -0.6), (-0.3, -0.95), (0.3, -0.95), (0.3, -0.6)]);
                self.body(&[(-0.68, -0.6), (0.68, -0.6), (0.52, 0.95), (-0.52, 0.95)]);
            }
            Icon::Rename => {
                self.rect_outline((-0.95, -0.5), (0.3, 0.5));
                self.line((0.65, -0.95), (0.65, 0.95));
                self.line((0.4, -0.95), (0.9, -0.95));
                self.line((0.4, 0.95), (0.9, 0.95));
            }
            Icon::More => {
                for x in [-0.7, 0.0, 0.7] {
                    self.dot((x, 0.0), 0.17);
                }
            }
            Icon::Search => {
                self.circle((-0.2, -0.2), 0.6);
                self.line((0.25, 0.25), (0.85, 0.85));
            }
            Icon::Undo => {
                self.arc((0.1, 0.2), 0.65, PI, TAU + FRAC_PI_2 * 0.9);
                self.head((-0.55, 0.2), FRAC_PI_2);
            }
            Icon::Redo => {
                self.arc((-0.1, 0.2), 0.65, -PI - FRAC_PI_2 * 0.9, 0.0);
                self.head((0.55, 0.2), FRAC_PI_2);
            }
            Icon::FlipV => {
                self.line((-0.95, 0.0), (0.95, 0.0));
                self.body(&[(-0.6, -0.25), (0.6, -0.25), (-0.6, -0.95)]);
                self.outline(&[(-0.6, 0.25), (0.6, 0.25), (-0.6, 0.95)]);
            }
            Icon::Rotate90 => {
                self.rect((-0.95, -0.05), (0.15, 0.95));
                let e = self.arc((-0.15, -0.05), 0.8, -FRAC_PI_2 - 0.3, -0.15);
                self.head(e, FRAC_PI_2 - 0.15);
            }
            Icon::Cut => {
                self.ring((-0.5, 0.6), 0.3);
                self.ring((0.5, 0.6), 0.3);
                self.line((-0.3, 0.35), (0.5, -0.9));
                self.line((0.3, 0.35), (-0.5, -0.9));
            }
            Icon::Paste => {
                self.rect((-0.75, -0.7), (0.75, 0.95));
                self.solid(&[(-0.35, -1.0), (0.35, -1.0), (0.35, -0.5), (-0.35, -0.5)]);
            }
            Icon::Peg => {
                self.circle((0.0, -0.45), 0.4);
                self.line((0.0, -0.05), (0.0, 0.9));
                self.line((-0.6, 0.9), (0.6, 0.9));
            }
            Icon::MergeDown => {
                self.line((-0.55, -0.95), (-0.55, -0.45));
                self.line((0.55, -0.95), (0.55, -0.45));
                self.polyline(&[(-0.55, -0.45), (0.0, 0.1), (0.55, -0.45)]);
                self.line((0.0, 0.1), (0.0, 0.9));
                self.head((0.0, 0.9), FRAC_PI_2);
            }
            Icon::Folder => self.body(&[(-0.95, -0.75), (-0.3, -0.75), (-0.1, -0.5), (0.95, -0.5), (0.95, 0.8), (-0.95, 0.8)]),
            Icon::Settings => {
                for k in 0..8 {
                    let a = k as f32 * TAU / 8.0;
                    self.line((a.cos() * 0.62, a.sin() * 0.62), (a.cos() * 0.92, a.sin() * 0.92));
                }
                self.circle((0.0, 0.0), 0.6);
                self.ring((0.0, 0.0), 0.2);
            }
            Icon::Theme => {
                self.ring((0.0, 0.0), 0.85);
                let mut half = Self::arc_pts((0.0, 0.0), 0.85, 0.85, FRAC_PI_2, PI + FRAC_PI_2);
                half.push((0.0, 0.85));
                self.solid(&half);
            }
            Icon::Close => {
                self.line((-0.7, -0.7), (0.7, 0.7));
                self.line((0.7, -0.7), (-0.7, 0.7));
            }
            Icon::ToolPalette => {
                // a panel holding a column of tool buttons and a row of them
                self.rect_outline((-0.95, -0.95), (0.95, 0.95));
                for y in [-0.5, 0.0, 0.5] {
                    self.rect((-0.65, y - 0.15), (-0.35, y + 0.15));
                }
                self.rect((0.0, 0.35), (0.3, 0.65));
                self.rect((0.45, 0.35), (0.75, 0.65));
            }
            Icon::Radial => {
                // a ring cut into slices around a centre dot
                self.ring((0.0, 0.0), 0.9);
                for k in 0..4 {
                    let a = FRAC_PI_4 + k as f32 * FRAC_PI_2;
                    self.line((a.cos() * 0.35, a.sin() * 0.35), (a.cos() * 0.9, a.sin() * 0.9));
                }
                self.dot((0.0, 0.0), 0.16);
            }
            // ── cell marks ──
            Icon::MarkKey => self.circle((0.0, 0.0), 0.7),
            Icon::MarkBreakdown => self.body(&[(0.0, -0.85), (0.85, 0.0), (0.0, 0.85), (-0.85, 0.0)]),
            Icon::MarkInbetween => self.body(&[(0.0, -0.7), (0.8, 0.6), (-0.8, 0.6)]),
        }
    }
}

/// A square icon button: the glyph (filled when `active`), an accessible name, and a tooltip
/// with the label and the shortcut. A disabled button stays visible and says why in its tooltip.
#[allow(dead_code, reason = "the shared icon family: the UI-pass shell, canvas and menu code draws from all of it; the tests paint every glyph")]
#[must_use = "add it with ui.add(..)"]
pub struct IconButton {
    icon: Icon,
    label: String,
    active: bool,
    toggle: bool,
    shortcut: Option<String>,
    disabled: Option<String>,
    side: f32,
}

#[allow(dead_code, reason = "the shared icon family: the UI-pass shell, canvas and menu code draws from all of it; the tests paint every glyph")]
impl IconButton {
    pub fn new(icon: Icon, label: impl Into<String>) -> Self {
        Self { icon, label: label.into(), active: false, toggle: false, shortcut: None, disabled: None, side: crate::theme::metrics::CONTROL_H }
    }

    /// Shown "on": the filled glyph on an accent tint. Makes the button a toggle for screen
    /// readers.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self.toggle = true;
        self
    }

    /// The key shown in the tooltip ("Ctrl+K"); an empty key or "none" shows no key.
    pub fn shortcut(mut self, key: impl Into<String>) -> Self {
        let k = key.into();
        self.shortcut = (!k.is_empty() && k != tl!("none")).then_some(k);
        self
    }

    /// Disable the button; `reason` is shown in its tooltip.
    pub fn disabled(mut self, reason: Option<String>) -> Self {
        self.disabled = reason;
        self
    }

    /// The button's side in points (default: the 22 px control height).
    pub fn size(mut self, side: f32) -> Self {
        self.side = if side.is_finite() { side.clamp(12.0, 64.0) } else { crate::theme::metrics::CONTROL_H };
        self
    }

    /// The tooltip text: the label and, when there is one, the shortcut.
    pub fn tooltip(&self) -> String {
        match &self.shortcut {
            Some(k) => format!("{} ({k})", self.label),
            None => self.label.clone(),
        }
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = self.disabled.is_none() && ui.is_enabled();
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, resp) = ui.allocate_exact_size(vec2(self.side, self.side), sense);
        let tip = self.tooltip();
        let (toggle, active, label) = (self.toggle, self.active, self.label.clone());
        resp.widget_info(|| {
            if toggle {
                egui::WidgetInfo::selected(egui::Role::Button, enabled, active, label.clone())
            } else {
                egui::WidgetInfo::labeled(egui::Role::Button, enabled, label.clone())
            }
        });
        if ui.is_rect_visible(rect) {
            let pal = crate::theme::palette(!ui.visuals().dark_mode);
            let r = crate::theme::metrics::RADIUS_CONTROL as f32;
            let hot = enabled && (resp.hovered() || resp.is_pointer_button_down_on());
            let bg = if self.active {
                pal.accent.gamma_multiply(0.18)
            } else if hot {
                pal.raised
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, r, bg);
            let fg = if !enabled {
                pal.weak.gamma_multiply(0.5)
            } else if self.active {
                pal.accent
            } else if hot {
                pal.text
            } else {
                pal.weak
            };
            let side = crate::theme::metrics::ICON.min(rect.width());
            paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(side, side)), self.icon, fg, self.active);
            // the keyboard focus ring
            if resp.has_focus() {
                ui.painter().rect_stroke(rect, r, Stroke::new(2.0, pal.playhead), egui::StrokeKind::Inside);
            }
        }
        match self.disabled {
            Some(why) => resp.on_disabled_hover_text(format!("{tip}: {why}")),
            None => resp.on_hover_text(tip),
        }
    }
}

/// Help ▸ Icon Reference: every glyph with its name, off and on (also `--open icons`).
pub fn reference_window(ctx: &egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    egui::Window::new(tl!("Icon Reference")).open(open).collapsible(false).default_width(640.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
            egui::Grid::new("tooncraft-icon-reference").num_columns(4).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
                for (i, icon) in Icon::ALL.into_iter().enumerate() {
                    ui.horizontal(|ui| {
                        let label = icon.label();
                        let _ = ui.add(IconButton::new(icon, label.clone()));
                        let _ = ui.add(IconButton::new(icon, tlf!("{icon}, on", icon = label)).active(true));
                        ui.label(label);
                    });
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        });
    });
}

/// `ui.icon_button(Icon, label, active)`.
#[allow(dead_code, reason = "the shared icon family: the UI-pass shell, canvas and menu code draws from all of it; the tests paint every glyph")]
pub trait IconUi {
    fn icon_button(&mut self, icon: Icon, label: impl Into<String>, active: bool) -> Response;
}

impl IconUi for Ui {
    fn icon_button(&mut self, icon: Icon, label: impl Into<String>, active: bool) -> Response {
        self.add(IconButton::new(icon, label).active(active))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;
    use std::collections::HashSet;

    fn visual_rect(shapes: &[Shape]) -> Rect {
        shapes.iter().fold(Rect::NOTHING, |r, s| r.union(s.visual_bounding_rect()))
    }

    #[test]
    fn every_icon_paints_inside_its_cell_and_has_a_label() {
        let cell = Rect::from_min_size(pos2(100.0, 100.0), vec2(16.0, 16.0));
        let mut seen = HashSet::new();
        for icon in Icon::ALL {
            assert!(seen.insert(icon), "{icon:?} is listed twice in Icon::ALL");
            for filled in [false, true] {
                let s = shapes(cell, icon, Color32::WHITE, filled);
                assert!(!s.is_empty(), "{icon:?} paints nothing");
                let v = visual_rect(&s);
                assert!(v.is_finite() && v.width() > 4.0 && v.height() > 1.0, "{icon:?} paints a speck: {v:?}");
                // the glyph stays inside its 16 px cell (2 px padding, less a stroke's half width)
                assert!(cell.contains_rect(v), "{icon:?} leaves its cell: {v:?}");
            }
            assert!(!icon.label().trim().is_empty(), "{icon:?} has no label");
        }
        assert!(Icon::ALL.len() >= 70, "the family covers about 70 glyphs");
    }

    #[test]
    fn toggles_look_different_on_and_off() {
        let cell = Rect::from_min_size(Pos2::ZERO, vec2(16.0, 16.0));
        for icon in [
            Icon::Onion,
            Icon::Eye,
            Icon::Lock,
            Icon::LockAlpha,
            Icon::OnionInclude,
            Icon::LightTable,
            Icon::Mirror,
            Icon::MarkKey,
            Icon::Label,
            Icon::FadeOthers,
        ] {
            let off = format!("{:?}", shapes(cell, icon, Color32::WHITE, false));
            let on = format!("{:?}", shapes(cell, icon, Color32::WHITE, true));
            assert_ne!(off, on, "{icon:?} looks the same on and off");
        }
    }

    #[test]
    fn glyphs_are_distinct() {
        let cell = Rect::from_min_size(Pos2::ZERO, vec2(16.0, 16.0));
        let mut seen = std::collections::HashMap::new();
        // the rectangle selection is the select tool's own glyph, and linear the gradient's
        let shared = [Icon::SelectRect, Icon::GradientLinear];
        for icon in Icon::ALL.into_iter().filter(|i| !shared.contains(i)) {
            let key = format!("{:?}", shapes(cell, icon, Color32::WHITE, false));
            if let Some(other) = seen.insert(key, icon) {
                panic!("{icon:?} draws the same as {other:?}");
            }
        }
    }

    #[test]
    fn every_tool_has_its_own_icon() {
        let mut seen = HashSet::new();
        for t in Tool::ALL {
            assert!(seen.insert(Icon::from(t)), "{t:?}");
        }
    }

    #[test]
    fn icons_survive_odd_rects() {
        for side in [0.0f32, 1.0, 8.0, 64.0, f32::NAN] {
            let r = Rect::from_min_size(Pos2::ZERO, vec2(side, side * 2.0));
            for icon in Icon::ALL {
                assert!(!shapes(r, icon, Color32::WHITE, true).is_empty());
            }
        }
    }

    #[test]
    fn icon_buttons_are_named_toggled_and_work_from_the_keyboard() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::{By, NodeT, Queryable};
        #[derive(Default)]
        struct S {
            onion: bool,
            undos: usize,
        }
        let mut h = Harness::builder().with_size(vec2(300.0, 80.0)).with_accessibility_check(true).build_ui_state(
            |ui, s: &mut S| {
                crate::theme::apply(ui.ctx(), false);
                ui.horizontal(|ui| {
                    if ui.icon_button(Icon::Onion, "Onion skin", s.onion).clicked() {
                        s.onion = !s.onion;
                    }
                    if ui.add(IconButton::new(Icon::Undo, "Undo").shortcut("Ctrl+Z")).clicked() {
                        s.undos += 1;
                    }
                    let _ = ui.add(IconButton::new(Icon::Redo, "Redo").disabled(Some("Nothing to redo".into())));
                });
            },
            S::default(),
        );
        h.run();
        let onion = h.get(By::new().role(egui::Role::Button).label("Onion skin"));
        assert_eq!(onion.accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
        assert!(h.get_by_label("Redo").accesskit_node().is_disabled(), "a disabled button stays visible, disabled");
        // Tab to the first button and press it with Space, then Tab on and press Enter
        h.key_press(egui::Key::Tab);
        h.run();
        h.key_press(egui::Key::Space);
        h.run();
        assert!(h.state().onion, "Space toggles the focused button");
        let onion = h.get(By::new().role(egui::Role::Button).label("Onion skin"));
        assert_eq!(onion.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
        h.key_press(egui::Key::Tab);
        h.run();
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(h.state().undos, 1, "Enter presses the focused button");
    }

    #[test]
    fn icon_button_tooltip_shows_the_key() {
        assert_eq!(IconButton::new(Icon::Undo, "Undo").shortcut("Ctrl+Z").tooltip(), "Undo (Ctrl+Z)");
        assert_eq!(IconButton::new(Icon::Undo, "Undo").shortcut("").tooltip(), "Undo");
        assert_eq!(IconButton::new(Icon::Undo, "Undo").size(f32::NAN).side, crate::theme::metrics::CONTROL_H);
    }
}
