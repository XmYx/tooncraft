//! Layer operations: duplicate a layer, merge down, merge the visible layers, flatten, and copy
//! a layer's look to another layer.

use std::collections::{BTreeMap, HashMap};

use tooncraft_doc::{Cell, DocError, DrawingId, Layer, Project, RenderOptions};
use tooncraft_raster::Surface;

use crate::undo::{Entry, Part};
use crate::{Engine, EngineError, Result};

/// Which layers a merge takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Merge {
    /// The layer and the one below it.
    Down(usize),
    /// Every visible layer, into the lowest of them.
    Visible,
    /// Every visible layer into one; hidden layers are removed.
    Flatten,
}

fn refuse(why: impl Into<String>) -> EngineError {
    EngineError::Merge(why.into())
}

/// A layer next to `layer` with everything of it: its look, and a private copy of every drawing
/// it exposes. The copy goes just above the original.
pub(crate) fn duplicate(e: &mut Engine, layer: usize) -> Result<()> {
    let src = e.project.layer(layer)?.clone();
    let before = Part::Layers(e.project.layers.clone());
    let mut made = Vec::new();
    match build_copy(e, layer, &src, &mut made) {
        Ok(()) => {
            let mut parts: Vec<Part> = made.into_iter().map(|id| Part::Drawing { id, surface: None }).collect();
            parts.push(before);
            e.push(Entry { label: "Duplicate layer".into(), parts });
            Ok(())
        }
        Err(err) => {
            for id in made {
                e.project.swap_drawing(id, None);
            }
            Entry { label: String::new(), parts: vec![before] }.swap(&mut e.project);
            Err(err)
        }
    }
}

fn build_copy(e: &mut Engine, layer: usize, src: &Layer, made: &mut Vec<DrawingId>) -> Result<()> {
    let name: String = format!("{} copy", src.name).chars().take(200).collect();
    let at = e.project.add_layer(&name, layer + 1)?;
    let mut map: BTreeMap<DrawingId, DrawingId> = BTreeMap::new();
    let mut copy_of = |e: &mut Engine, d: DrawingId, made: &mut Vec<DrawingId>| -> Result<Option<DrawingId>> {
        if let Some(n) = map.get(&d) {
            return Ok(Some(*n));
        }
        let Some(surface) = e.project.drawing_owned(d)? else { return Ok(None) };
        let n = e.project.add_drawing(Some(surface))?;
        made.push(n);
        map.insert(d, n);
        Ok(Some(n))
    };
    let mut cells = Vec::with_capacity(src.cells.len());
    for c in &src.cells {
        cells.push(match c {
            Cell::Key(d) => copy_of(e, *d, made)?.map_or(Cell::Blank, Cell::Key),
            Cell::Hold => Cell::Hold,
            Cell::Blank => Cell::Blank,
        });
    }
    let mut chart = BTreeMap::new();
    for (mouth, d) in &src.mouth_chart {
        if let Some(n) = copy_of(e, *d, made)? {
            chart.insert(*mouth, n);
        }
    }
    // the copy gets its own mask drawing
    let mask = match src.mask {
        Some(m) => copy_of(e, m.drawing, made)?.map(|d| tooncraft_doc::LayerMask { drawing: d, ..m }),
        None => None,
    };
    let l = e.project.layer_mut(at)?;
    let id = l.id;
    *l = Layer { id, name, cells, mouth_chart: chart, mask, ..src.clone() };
    Ok(())
}

/// Copy the look of layer `from` onto layer `to`: opacity (and its keys), blend mode, label
/// colour, folder, peg, depth and the effect stack. Names, drawings, timing, locks and the
/// matte, clipping and lip-sync settings stay as they were.
pub(crate) fn copy_properties(e: &mut Engine, from: usize, to: usize) -> Result<()> {
    let src = e.project.layer(from)?.clone();
    e.check_unlocked(to)?;
    let l = e.project.layer_mut(to)?;
    l.opacity = src.opacity;
    l.opacity_keys = src.opacity_keys;
    l.blend = src.blend;
    l.color = src.color;
    l.folder = src.folder;
    l.peg = src.peg;
    l.depth = src.depth;
    l.effects = src.effects;
    Ok(())
}

/// What decides how a frame of the merge looks: when it is the same as the previous frame's, the
/// result is a hold of the previous drawing.
type Signature = Vec<(Option<u64>, bool, u32, [u32; 6])>;

fn signature(p: &Project, frame: u32) -> Signature {
    p.layers
        .iter()
        .enumerate()
        .map(|(li, l)| {
            let (visible, opacity) = p.effective_at(l, frame);
            let t = p.layer_transform(li, frame);
            let effects = tooncraft_fx::is_active(&l.effects);
            // effect keys move with the frame, so such layers are never "the same" as another frame
            (
                l.drawing_at(frame).map(|d| d.0 + if effects { (frame as u64 + 1) << 40 } else { 0 }),
                visible,
                opacity.to_bits(),
                [t.a, t.b, t.c, t.d, t.tx, t.ty].map(f32::to_bits),
            )
        })
        .collect()
}

/// Merge layers into one, frame by frame, through the project's own compositor, so blend modes,
/// opacity keys, pegs, mattes and effects of the merged layers look exactly as they did. The
/// result is a normal layer at full opacity with no effects or peg.
pub(crate) fn merge(e: &mut Engine, kind: Merge) -> Result<()> {
    let p = &e.project;
    let owners = p.matte_owners();
    let set: Vec<usize> = match kind {
        Merge::Down(layer) => {
            p.layer(layer)?;
            if layer == 0 {
                return Err(refuse("there is no layer below"));
            }
            vec![layer - 1, layer]
        }
        Merge::Visible | Merge::Flatten => p.layers.iter().enumerate().filter(|(_, l)| p.effective(l).0).map(|(i, _)| i).collect(),
    };
    if set.len() < 2 {
        return Err(refuse("fewer than two visible layers"));
    }
    let Some(&target) = set.first() else { return Err(refuse("fewer than two visible layers")) };
    for &li in &set {
        let l = p.layer(li)?;
        if p.is_locked(l) {
            return Err(EngineError::Locked(l.name.clone()));
        }
        if !p.effective(l).0 {
            return Err(refuse(format!("layer “{}” is hidden", l.name)));
        }
        if let Some(m) = owners.get(li).copied().flatten()
            && !set.contains(&m)
        {
            return Err(refuse(format!("layer “{}” is cut by a matte layer that is not part of the merge", l.name)));
        }
        if l.matte.is_some() && owners.iter().enumerate().any(|(o, owner)| *owner == Some(li) && !set.contains(&o)) {
            return Err(refuse(format!("matte layer “{}” cuts layers that are not part of the merge", l.name)));
        }
    }
    let lowest = p.layer(target)?.clone();
    if lowest.clip {
        return Err(refuse(format!("layer “{}” is clipped to the layer below", lowest.name)));
    }
    if let Merge::Down(_) = kind {
        let (a, b) = (p.layer(set[0])?, p.layer(set[1])?);
        if a.folder != b.folder {
            return Err(refuse("the two layers are in different folders"));
        }
        // a folder's opacity is applied to each of its layers on its own, so a merged layer under
        // it would look different where the two overlap
        if p.effective(a).1 != a.opacity {
            return Err(refuse("the folder is not at full opacity: set it to 100 % first"));
        }
    }

    // the layers as they will be composited: only the merged ones; for a merge down the folder
    // (shared by both) stays outside, so its opacity is not applied twice
    let mut temp = p.clone();
    let keep: Vec<Layer> = set
        .iter()
        .filter_map(|&i| p.layers.get(i))
        .cloned()
        .map(|mut l| {
            if matches!(kind, Merge::Down(_)) {
                l.folder = None;
            }
            l
        })
        .collect();
    temp.layers = keep;
    if matches!(kind, Merge::Down(_)) {
        temp.folders.clear();
    }

    let (w, h, frames) = (p.width(), p.height(), p.length());
    let before = Part::Layers(p.layers.clone());
    let folders_before = Part::Folders(p.folders.clone());
    let mut made: Vec<DrawingId> = Vec::new();
    let result = render_cells(e, &temp, (w, h, frames), &mut made);
    let cells = match result {
        Ok(c) => c,
        Err(err) => {
            for id in made {
                e.project.swap_drawing(id, None);
            }
            return Err(err);
        }
    };

    let mut merged = lowest;
    merged.cells = cells;
    merged.effects = Vec::new();
    merged.opacity = 1.0;
    merged.opacity_keys = Vec::new();
    merged.blend = Default::default();
    merged.peg = None;
    merged.depth = 0.0;
    merged.clip = false;
    merged.matte = None;
    merged.mouth_chart = BTreeMap::new();
    merged.visible = true;
    if !matches!(kind, Merge::Down(_)) {
        merged.folder = None;
        merged.name = "Merged".into();
    }
    let hidden: Vec<usize> = if kind == Merge::Flatten { (0..e.project.layers.len()).filter(|i| !set.contains(i)).collect() } else { Vec::new() };
    let layers = &mut e.project.layers;
    if let Some(slot) = layers.get_mut(target) {
        *slot = merged;
    }
    let mut gone: Vec<usize> = set.iter().copied().filter(|&i| i != target).chain(hidden).collect();
    gone.sort_unstable_by(|a, b| b.cmp(a));
    for i in gone {
        if i < layers.len() {
            layers.remove(i);
        }
    }
    e.project.prune_folders();
    let mut parts: Vec<Part> = made.into_iter().map(|id| Part::Drawing { id, surface: None }).collect();
    parts.push(before);
    // emptied folders go away with their layers; undo brings them back
    parts.push(folders_before);
    let label = match kind {
        Merge::Down(_) => "Merge down",
        Merge::Visible => "Merge visible layers",
        Merge::Flatten => "Flatten",
    };
    e.push(Entry { label: label.into(), parts });
    Ok(())
}

/// Render every frame of `temp` into new drawings of `e`'s project and return the cells.
fn render_cells(e: &mut Engine, temp: &Project, (w, h, frames): (u32, u32, u32), made: &mut Vec<DrawingId>) -> Result<Vec<Cell>> {
    let opts = RenderOptions { paper: false, ..Default::default() };
    let mut cache: HashMap<Signature, Option<DrawingId>> = HashMap::new();
    let mut cells = Vec::with_capacity(frames as usize);
    let mut previous: Option<Option<DrawingId>> = None;
    for f in 0..frames {
        let sig = signature(temp, f);
        let shown = match cache.get(&sig) {
            Some(d) => *d,
            None => {
                let img = tooncraft_doc::render_region(temp, f, (0, 0, w, h), opts);
                let d = if img.opaque_bounds().is_some() {
                    let mut s = Surface::new(w, h).map_err(DocError::from)?;
                    s.load_image(&img).map_err(DocError::from)?;
                    let id = e.project.add_drawing(Some(s))?;
                    made.push(id);
                    Some(id)
                } else {
                    None
                };
                cache.insert(sig, d);
                d
            }
        };
        cells.push(match shown {
            None => Cell::Blank,
            Some(d) if previous == Some(Some(d)) => Cell::Hold,
            Some(d) => Cell::Key(d),
        });
        previous = Some(shown);
    }
    Ok(cells)
}

/// Give layer `layer` an empty painted mask and start painting into it (one undo step).
pub(crate) fn add_mask(e: &mut Engine, layer: usize, invert: bool) -> Result<()> {
    e.check_unlocked(layer)?;
    if e.project.layer(layer)?.mask.is_some() {
        return Err(EngineError::HasMask);
    }
    let before = Part::Layers(e.project.layers.clone());
    let id = e.project.add_drawing(None)?;
    if let Ok(l) = e.project.layer_mut(layer) {
        l.mask = Some(tooncraft_doc::LayerMask { drawing: id, enabled: true, invert, editing: true });
    }
    e.push(Entry { label: "Add layer mask".into(), parts: vec![Part::Drawing { id, surface: None }, before] });
    Ok(())
}
