//! The compositing graph: an optional node graph that builds the frame from the layers instead of
//! stacking them bottom to top. Layer nodes give a layer's picture (its drawing through its
//! transform, its mask, matte and effects, at its opacity); effect nodes run one effect on their
//! input; blend nodes lay one picture over another in a blend mode; mask nodes keep a picture
//! inside (or outside) another's shape; the output node is the frame. When the graph is switched
//! on and its output is linked, the compositor shows what it gives; otherwise the layer stack.
//!
//! The graph comes from files, so it is hostile: [`CompGraph::sanitized`] caps the nodes and
//! links, drops links to missing nodes or ports and links that would close a loop, and sanitizes
//! every effect. Evaluation visits each node at most once per frame and follows links only
//! backwards from the output, so it ends even on a damaged graph.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tooncraft_raster::{BlendMode, RgbaImage};

/// Most nodes in a graph.
pub const MAX_NODES: usize = 256;
/// Most links in a graph.
pub const MAX_LINKS: usize = 1024;

/// What a node does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeKind {
    /// A layer's picture (its id). No inputs.
    Layer { layer: u64 },
    /// One effect on the input picture.
    Effect { effect: tooncraft_fx::Effect },
    /// The front picture laid over the back one in `mode`, at `opacity`.
    Blend {
        #[serde(default)]
        mode: BlendMode,
        #[serde(default = "one")]
        opacity: f32,
    },
    /// The picture kept inside the matte's shape (or outside, with `invert`).
    Mask {
        #[serde(default)]
        invert: bool,
    },
    /// The frame.
    Output,
}

fn one() -> f32 {
    1.0
}

impl NodeKind {
    /// The names of its inputs, top to bottom (the host translates them).
    pub fn inputs(&self) -> &'static [&'static str] {
        match self {
            NodeKind::Layer { .. } => &[],
            NodeKind::Effect { .. } => &["Picture"],
            NodeKind::Blend { .. } => &["Back", "Front"],
            NodeKind::Mask { .. } => &["Picture", "Matte"],
            NodeKind::Output => &["Picture"],
        }
    }

    /// Does it have an output?
    pub fn has_output(&self) -> bool {
        !matches!(self, NodeKind::Output)
    }

    fn sanitized(self) -> NodeKind {
        match self {
            NodeKind::Effect { effect } => NodeKind::Effect { effect: effect.sanitized() },
            NodeKind::Blend { mode, opacity } => NodeKind::Blend { mode, opacity: if opacity.is_finite() { opacity.clamp(0.0, 1.0) } else { 1.0 } },
            k => k,
        }
    }
}

/// One node: its id, what it does and where it sits in the node view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompNode {
    pub id: u64,
    pub kind: NodeKind,
    #[serde(default)]
    pub pos: [f32; 2],
}

/// The output of `from` feeds input `input` of `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompLink {
    pub from: u64,
    pub to: u64,
    pub input: usize,
}

/// The shot's compositing graph.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompGraph {
    /// Use the graph to build frames (off: the layer stack, as without a graph).
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub nodes: Vec<CompNode>,
    #[serde(default)]
    pub links: Vec<CompLink>,
}

/// Why a graph edit was refused.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CompError {
    #[error("a compositing graph holds at most {MAX_NODES} nodes")]
    TooManyNodes,
    #[error("a compositing graph holds at most {MAX_LINKS} links")]
    TooManyLinks,
    #[error("there is no node {0}")]
    NoNode(u64),
    #[error("that link would make a loop or joins ports that do not exist")]
    BadLink,
    #[error("the graph already has an output")]
    TwoOutputs,
}

impl CompGraph {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: u64) -> Option<&CompNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Will frames be built by the graph (on, with a linked output)?
    pub fn active(&self) -> bool {
        self.enabled && self.output().is_some_and(|o| self.source(o, 0).is_some())
    }

    fn output(&self) -> Option<u64> {
        self.nodes.iter().find(|n| n.kind == NodeKind::Output).map(|n| n.id)
    }

    /// The node feeding input `input` of `to`.
    pub fn source(&self, to: u64, input: usize) -> Option<u64> {
        self.links.iter().find(|l| l.to == to && l.input == input).map(|l| l.from)
    }

    /// A fresh node id.
    fn next_id(&self) -> u64 {
        self.nodes.iter().map(|n| n.id).max().map_or(1, |m| m.saturating_add(1))
    }

    /// Add a node; returns its id.
    pub fn add(&mut self, kind: NodeKind, pos: [f32; 2]) -> Result<u64, CompError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(CompError::TooManyNodes);
        }
        if kind == NodeKind::Output && self.output().is_some() {
            return Err(CompError::TwoOutputs);
        }
        let id = self.next_id();
        self.nodes.push(CompNode { id, kind: kind.sanitized(), pos: finite_pos(pos) });
        Ok(id)
    }

    /// Remove a node and its links.
    pub fn remove(&mut self, id: u64) -> Result<(), CompError> {
        let before = self.nodes.len();
        self.nodes.retain(|n| n.id != id);
        if self.nodes.len() == before {
            return Err(CompError::NoNode(id));
        }
        self.links.retain(|l| l.from != id && l.to != id);
        Ok(())
    }

    /// Would `link` join an existing output to an existing input without closing a loop?
    pub fn can_link(&self, link: &CompLink) -> bool {
        let (Some(from), Some(to)) = (self.node(link.from), self.node(link.to)) else { return false };
        if link.from == link.to || !from.kind.has_output() || link.input >= to.kind.inputs().len() {
            return false;
        }
        // a loop: `to` already feeds `from`
        !self.feeds(link.to, link.from)
    }

    /// Does `a` feed `b`, directly or through other nodes? Each node is visited once.
    fn feeds(&self, a: u64, b: u64) -> bool {
        let mut stack = vec![b];
        let mut seen = std::collections::HashSet::new();
        while let Some(n) = stack.pop() {
            if n == a {
                return true;
            }
            if !seen.insert(n) {
                continue;
            }
            stack.extend(self.links.iter().filter(|l| l.to == n).map(|l| l.from));
        }
        false
    }

    /// Make the link; it replaces any link into the same input.
    pub fn link(&mut self, link: CompLink) -> Result<(), CompError> {
        if !self.can_link(&link) {
            return Err(CompError::BadLink);
        }
        self.links.retain(|l| !(l.to == link.to && l.input == link.input));
        if self.links.len() >= MAX_LINKS {
            return Err(CompError::TooManyLinks);
        }
        self.links.push(link);
        Ok(())
    }

    /// Take away the link into `input` of `to`; true if there was one.
    pub fn unlink(&mut self, to: u64, input: usize) -> bool {
        let before = self.links.len();
        self.links.retain(|l| !(l.to == to && l.input == input));
        self.links.len() != before
    }

    /// Move a node in the node view.
    pub fn place(&mut self, id: u64, pos: [f32; 2]) -> Result<(), CompError> {
        let n = self.nodes.iter_mut().find(|n| n.id == id).ok_or(CompError::NoNode(id))?;
        n.pos = finite_pos(pos);
        Ok(())
    }

    /// Change what a node does (the same number of inputs keeps its links; fewer drops the rest).
    pub fn set_kind(&mut self, id: u64, kind: NodeKind) -> Result<(), CompError> {
        if kind == NodeKind::Output && self.output().is_some_and(|o| o != id) {
            return Err(CompError::TwoOutputs);
        }
        let n = self.nodes.iter_mut().find(|n| n.id == id).ok_or(CompError::NoNode(id))?;
        n.kind = kind.sanitized();
        let (inputs, output) = (n.kind.inputs().len(), n.kind.has_output());
        self.links.retain(|l| !(l.to == id && l.input >= inputs) && (output || l.from != id));
        Ok(())
    }

    /// A graph that gives the same frame as the layer stack: a layer node for each layer, laid
    /// over each other bottom to top, into the output. A starting point the user then rewires.
    pub fn from_layers(p: &crate::Project) -> CompGraph {
        let mut g = CompGraph { enabled: true, ..Default::default() };
        let mut below: Option<u64> = None;
        for (i, l) in p.layers.iter().enumerate().take(MAX_NODES / 2 - 1) {
            let y = i as f32 * 90.0;
            let Ok(node) = g.add(NodeKind::Layer { layer: l.id.0 }, [0.0, y]) else { break };
            below = Some(match below {
                None => node,
                Some(b) => {
                    let Ok(blend) = g.add(NodeKind::Blend { mode: BlendMode::Normal, opacity: 1.0 }, [220.0, y]) else { break };
                    let _ = g.link(CompLink { from: b, to: blend, input: 0 });
                    let _ = g.link(CompLink { from: node, to: blend, input: 1 });
                    blend
                }
            });
        }
        if let (Ok(out), Some(b)) = (g.add(NodeKind::Output, [440.0, 0.0]), below) {
            let _ = g.link(CompLink { from: b, to: out, input: 0 });
        }
        g
    }

    /// Every number clamped, at most [`MAX_NODES`] nodes with unique ids and one output, at
    /// most [`MAX_LINKS`] links between existing ports, none closing a loop.
    pub fn sanitized(self) -> CompGraph {
        let mut out = CompGraph { enabled: self.enabled, ..Default::default() };
        let mut has_output = false;
        for n in self.nodes.into_iter().take(MAX_NODES) {
            if out.node(n.id).is_some() || (n.kind == NodeKind::Output && has_output) {
                continue;
            }
            has_output |= n.kind == NodeKind::Output;
            out.nodes.push(CompNode { id: n.id, kind: n.kind.sanitized(), pos: finite_pos(n.pos) });
        }
        for l in self.links.into_iter().take(MAX_LINKS) {
            let _ = out.link(l);
        }
        out
    }

    /// Build the frame: `layer(id)` gives a layer's picture (the whole canvas, premultiplied),
    /// `None` when there is no such layer. `None` when the graph is not active.
    pub fn render(&self, frame: u32, size: (u32, u32), layer: &dyn Fn(u64) -> Option<RgbaImage>) -> Option<RgbaImage> {
        if !self.active() {
            return None;
        }
        let out = self.output()?;
        let mut memo: HashMap<u64, RgbaImage> = HashMap::new();
        let src = self.source(out, 0)?;
        Some(self.eval(src, frame, size, layer, &mut memo, 0))
    }

    fn eval(
        &self,
        id: u64,
        frame: u32,
        size: (u32, u32),
        layer: &dyn Fn(u64) -> Option<RgbaImage>,
        memo: &mut HashMap<u64, RgbaImage>,
        depth: usize,
    ) -> RgbaImage {
        if let Some(img) = memo.get(&id) {
            return img.clone();
        }
        let blank = || RgbaImage::transparent(size.0, size.1);
        // links never close a loop (sanitized), and the depth cap ends a damaged graph anyway
        if depth > MAX_NODES {
            return blank();
        }
        let input = |memo: &mut HashMap<u64, RgbaImage>, i: usize| match self.source(id, i) {
            Some(s) => self.eval(s, frame, size, layer, memo, depth + 1),
            None => blank(),
        };
        let img = match self.node(id).map(|n| n.kind.clone()) {
            Some(NodeKind::Layer { layer: l }) => layer(l).filter(|i| (i.width, i.height) == size).unwrap_or_else(blank),
            Some(NodeKind::Effect { effect }) => {
                let mut img = input(memo, 0);
                tooncraft_fx::apply_stack(&mut img, std::slice::from_ref(&effect), frame, None);
                img
            }
            Some(NodeKind::Blend { mode, opacity }) => {
                let mut back = input(memo, 0);
                let front = input(memo, 1);
                back.composite(&front, mode, opacity);
                back
            }
            Some(NodeKind::Mask { invert }) => {
                let mut img = input(memo, 0);
                let matte = input(memo, 1);
                for (d, s) in img.data.as_chunks_mut::<4>().0.iter_mut().zip(matte.data.as_chunks::<4>().0.iter()) {
                    let k = if invert { 255 - s[3] } else { s[3] };
                    if k < 255 {
                        *d = tooncraft_raster::scale_alpha(*d, k);
                    }
                }
                img
            }
            Some(NodeKind::Output) | None => blank(),
        };
        memo.insert(id, img.clone());
        img
    }
}

fn finite_pos(p: [f32; 2]) -> [f32; 2] {
    p.map(|v| if v.is_finite() { v.clamp(-1e6, 1e6) } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(c: [u8; 4]) -> RgbaImage {
        RgbaImage::filled(4, 4, c)
    }

    fn blur() -> NodeKind {
        NodeKind::Effect { effect: tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Blur { radius: 1.0 }) }
    }

    #[test]
    fn links_refuse_loops_and_bad_ports() {
        let mut g = CompGraph::default();
        let a = g.add(NodeKind::Layer { layer: 1 }, [0.0, 0.0]).unwrap();
        let fx = g.add(blur(), [0.0, 0.0]).unwrap();
        let out = g.add(NodeKind::Output, [0.0, 0.0]).unwrap();
        assert_eq!(g.add(NodeKind::Output, [0.0, 0.0]), Err(CompError::TwoOutputs));
        g.link(CompLink { from: a, to: fx, input: 0 }).unwrap();
        g.link(CompLink { from: fx, to: out, input: 0 }).unwrap();
        assert_eq!(g.link(CompLink { from: fx, to: fx, input: 0 }), Err(CompError::BadLink), "itself");
        assert_eq!(g.link(CompLink { from: a, to: fx, input: 3 }), Err(CompError::BadLink), "no such input");
        assert_eq!(g.link(CompLink { from: out, to: fx, input: 0 }), Err(CompError::BadLink), "the output has no output");
        let fx2 = g.add(blur(), [0.0, 0.0]).unwrap();
        g.link(CompLink { from: fx, to: fx2, input: 0 }).unwrap();
        // fx feeds fx2, so fx2 → fx would close a loop
        assert_eq!(g.link(CompLink { from: fx2, to: fx, input: 0 }), Err(CompError::BadLink));
        g.enabled = true;
        assert!(g.active());
        g.remove(fx).unwrap();
        assert!(g.links.iter().all(|l| l.from != fx && l.to != fx));
        assert!(!g.active(), "the output lost its source");
    }

    #[test]
    fn blends_masks_and_effects_build_the_frame() {
        let mut g = CompGraph { enabled: true, ..Default::default() };
        let red = g.add(NodeKind::Layer { layer: 1 }, [0.0, 0.0]).unwrap();
        let blue = g.add(NodeKind::Layer { layer: 2 }, [0.0, 0.0]).unwrap();
        let blend = g.add(NodeKind::Blend { mode: BlendMode::Normal, opacity: 0.5 }, [0.0, 0.0]).unwrap();
        let out = g.add(NodeKind::Output, [0.0, 0.0]).unwrap();
        g.link(CompLink { from: red, to: blend, input: 0 }).unwrap();
        g.link(CompLink { from: blue, to: blend, input: 1 }).unwrap();
        g.link(CompLink { from: blend, to: out, input: 0 }).unwrap();
        let layer = |id: u64| match id {
            1 => Some(picture([255, 0, 0, 255])),
            2 => Some(picture([0, 0, 255, 255])),
            _ => None,
        };
        let img = g.render(0, (4, 4), &layer).unwrap();
        let p = img.pixel(1, 1);
        assert!(p[0] > 100 && p[2] > 100 && p[3] == 255, "half blue over red: {p:?}");
        // a mask by a missing layer keeps nothing
        let mask = g.add(NodeKind::Mask { invert: false }, [0.0, 0.0]).unwrap();
        let gone = g.add(NodeKind::Layer { layer: 99 }, [0.0, 0.0]).unwrap();
        g.link(CompLink { from: blend, to: mask, input: 0 }).unwrap();
        g.link(CompLink { from: gone, to: mask, input: 1 }).unwrap();
        g.link(CompLink { from: mask, to: out, input: 0 }).unwrap();
        assert_eq!(g.render(0, (4, 4), &layer).unwrap().pixel(1, 1)[3], 0);
        g.set_kind(mask, NodeKind::Mask { invert: true }).unwrap();
        assert_eq!(g.render(0, (4, 4), &layer).unwrap().pixel(1, 1)[3], 255, "inverted: everything kept");
        g.enabled = false;
        assert!(g.render(0, (4, 4), &layer).is_none(), "switched off: the layer stack");
    }

    #[test]
    fn hostile_graphs_are_cleaned_and_end() {
        // duplicate ids, two outputs, loops, links to nowhere, too many nodes
        let mut nodes: Vec<CompNode> = (0..MAX_NODES as u64 + 50)
            .map(|i| CompNode { id: i % 300, kind: NodeKind::Blend { mode: BlendMode::Normal, opacity: f32::NAN }, pos: [f32::INFINITY, 0.0] })
            .collect();
        nodes.insert(0, CompNode { id: 500, kind: NodeKind::Output, pos: [0.0, 0.0] });
        nodes.push(CompNode { id: 1000, kind: NodeKind::Output, pos: [0.0, 0.0] });
        let links = vec![
            CompLink { from: 1, to: 2, input: 0 },
            CompLink { from: 2, to: 1, input: 0 },
            CompLink { from: 7, to: 7, input: 1 },
            CompLink { from: 3, to: 4242, input: 0 },
            CompLink { from: 2, to: 500, input: 0 },
        ];
        let g = CompGraph { enabled: true, nodes, links }.sanitized();
        assert!(g.nodes.len() <= MAX_NODES);
        assert_eq!(g.nodes.iter().filter(|n| n.kind == NodeKind::Output).count(), 1);
        assert!(g.nodes.iter().all(|n| n.pos[0].is_finite()));
        assert!(g.nodes.iter().all(|n| !matches!(n.kind, NodeKind::Blend { opacity, .. } if !opacity.is_finite())));
        assert_eq!(g.links.len(), 2, "the loop's second half, the self link and the dangling one are gone: {:?}", g.links);
        let img = g.render(0, (2, 2), &|_| None).unwrap();
        assert_eq!(img.data.len(), 16);
    }
}
