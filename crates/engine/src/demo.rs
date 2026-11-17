//! A built-in demo shot, drawn entirely through commands (original work, code-drawn): a ball
//! bouncing across the paper with squash and stretch, on an ink layer over a paint layer, with a
//! held background. Used by `tooncraft --demo`, `tooncraft-cli demo`, screenshots and benchmarks.

use tooncraft_brush::{Brush, BrushKind, Sample, Smoothing};
use tooncraft_doc::Project;
use tooncraft_raster::fill::FillOptions;

use crate::{Command, Engine, FillReference, Result};

/// Frames in the demo shot.
pub const DEMO_FRAMES: u32 = 24;

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, start: f32, turns: f32, wobble: f32) -> Vec<Sample> {
    let n = ((rx.max(ry) * turns * 0.6) as usize).clamp(24, 400);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = start + t * turns * std::f32::consts::TAU;
            let w = 1.0 + (a * 3.0).sin() * wobble;
            // pressure swells in the middle of the line, like a hand-inked circle
            let p = 0.35 + 0.65 * (t * std::f32::consts::PI).sin();
            Sample { x: cx + a.cos() * rx * w, y: cy + a.sin() * ry * w, pressure: p, tilt_x: 0.0, tilt_y: 0.0, time: t as f64 * 0.6 }
        })
        .collect()
}

fn line(points: &[(f32, f32)], pressure: f32) -> Vec<Sample> {
    let mut out = Vec::new();
    for (i, w) in points.windows(2).enumerate() {
        for k in 0..20 {
            let t = k as f32 / 20.0;
            out.push(Sample::at(w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t, pressure, (i * 20 + k) as f64 * 0.01));
        }
    }
    if let Some(l) = points.last() {
        out.push(Sample::at(l.0, l.1, pressure, out.len() as f64 * 0.01));
    }
    out
}

/// Build the demo shot at `width`×`height` (scaled from a 1280×720 layout).
pub fn demo(width: u32, height: u32) -> Result<Engine> {
    let mut e = Engine::new(Project::new("Bouncing ball", width, height, 12, DEMO_FRAMES)?);
    let s = width as f32 / 1280.0;
    let sy = height as f32 / 720.0;
    let ink = Brush { size: 5.0 * s, color: [28, 26, 40, 255], smoothing: Smoothing { average: 0.4, ..Default::default() }, ..Brush::preset(BrushKind::Ink) };
    let pencil = Brush { size: 3.0 * s, color: [90, 86, 110, 255], ..Brush::preset(BrushKind::Pencil) };
    let chalk = Brush { size: 26.0 * s, color: [196, 170, 120, 255], ..Brush::preset(BrushKind::Chalk) };
    // layers, bottom to top: Background, Paint, Ink
    e.apply(Command::RenameLayer { layer: 0, name: "Background".into() })?;
    e.apply(Command::AddLayer { name: "Paint".into(), index: None })?;
    e.apply(Command::AddLayer { name: "Ink".into(), index: None })?;
    let ground = 600.0 * sy;
    // Background: a ground line and a chalky floor, held for the whole shot.
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: pencil, samples: line(&[(60.0 * s, ground), (1220.0 * s, ground + 4.0 * sy)], 0.8) })?;
    for k in 0..4 {
        let y = ground + (18.0 + k as f32 * 22.0) * sy;
        e.apply(Command::Stroke {
            layer: 0,
            frame: 0,
            brush: chalk,
            samples: line(&[(80.0 * s + k as f32 * 30.0, y), (1200.0 * s - k as f32 * 40.0, y + 3.0)], 0.4 + k as f32 * 0.1),
        })?;
    }
    e.apply(Command::ExtendHold { layer: 0, frame: 0 })?;
    let len = e.project.length();
    if let Ok(l) = e.project.layer_mut(0) {
        for f in 1..len as usize {
            l.cells[f] = tooncraft_doc::Cell::Hold;
        }
    }
    // The ball: 12 drawings on twos, a parabolic bounce left to right.
    let r = 46.0 * s;
    for i in 0..12u32 {
        let frame = i * 2;
        let t = i as f32 / 11.0;
        let x = (140.0 + t * 1000.0) * s;
        let phase = (t * 2.0).fract();
        let height = 1.0 - (2.0 * phase - 1.0).powi(2);
        let contact = height < 0.12;
        let (rx, ry) = if contact { (r * 1.3, r * 0.72) } else { (r * (1.0 - height * 0.08), r * (1.0 + height * 0.12)) };
        let cy = ground - ry - height * 380.0 * sy;
        e.apply(Command::Stroke { layer: 2, frame, brush: ink, samples: ellipse(x, cy, rx, ry, 0.3 * i as f32, 1.04, 0.012) })?;
        // a highlight arc
        e.apply(Command::Stroke {
            layer: 2,
            frame,
            brush: Brush { size: 3.0 * s, ..ink },
            samples: ellipse(x - rx * 0.1, cy - ry * 0.1, rx * 0.55, ry * 0.55, 3.6, 0.22, 0.0),
        })?;
        // paint under the ink
        e.apply(Command::Fill {
            layer: 1,
            frame,
            x: x as u32,
            y: (cy + ry * 0.5) as u32,
            color: [232, 92, 70, 255],
            options: FillOptions { gap: 4, ..Default::default() },
            reference: FillReference::AllLayers,
        })?;
        e.apply(Command::ExtendHold { layer: 1, frame })?;
        e.apply(Command::ExtendHold { layer: 2, frame })?;
    }
    e.project.light_table.enabled = false;
    // a slow push-in over the shot
    let (cx, cy) = (640.0 * s, 360.0 * sy);
    e.apply(Command::SetCameraKey { frame: 0, x: cx, y: cy, zoom: 1.0, angle: 0.0 })?;
    e.apply(Command::SetCameraKey { frame: DEMO_FRAMES - 1, x: cx + 60.0 * s, y: cy + 30.0 * sy, zoom: 1.12, angle: 0.0 })?;
    e.mark_saved();
    Ok(e)
}

/// A big stress shot: `frames` drawings on ones at `width`×`height` (4K by default from the CLI),
/// an ink layer with a different drawing every frame and a held background. For the
/// large-document benchmarks.
pub fn large(width: u32, height: u32, frames: u32) -> Result<Engine> {
    let mut e = Engine::new(Project::new("Large shot", width, height, 24, frames.max(1))?);
    let s = width as f32 / 1280.0;
    let pencil = Brush { size: 3.0 * s, color: [70, 66, 90, 255], ..Brush::preset(BrushKind::Pencil) };
    let ink = Brush { size: 4.0 * s, color: [28, 26, 40, 255], ..Brush::preset(BrushKind::Ink) };
    e.apply(Command::Stroke { layer: 0, frame: 0, brush: pencil, samples: line(&[(40.0 * s, 650.0 * s), (1240.0 * s, 650.0 * s)], 0.7) })?;
    if let Ok(l) = e.project.layer_mut(0) {
        for c in l.cells.iter_mut().skip(1) {
            *c = tooncraft_doc::Cell::Hold;
        }
    }
    e.apply(Command::AddLayer { name: "Ink".into(), index: None })?;
    for f in 0..frames.max(1) {
        let t = f as f32 / frames.max(1) as f32;
        let x = (100.0 + t * 1080.0) * s;
        let y = (360.0 + (t * 12.0).sin() * 160.0) * s;
        e.apply(Command::Stroke { layer: 1, frame: f, brush: ink, samples: ellipse(x, y, 60.0 * s, 60.0 * s, t * 6.0, 1.05, 0.02) })?;
        e.apply(Command::Stroke {
            layer: 1,
            frame: f,
            brush: ink,
            samples: line(&[(x, y + 60.0 * s), (x - 20.0 * s, y + 200.0 * s), (x + 30.0 * s, y + 280.0 * s)], 0.8),
        })?;
    }
    e.mark_saved();
    Ok(e)
}

#[cfg(test)]
mod tests {
    #[test]
    fn demo_builds() {
        let e = super::demo(640, 360).unwrap();
        assert_eq!(e.project.layers.len(), 3);
        assert_eq!(crate::sheet(&e.project.layers[2].cells), "#-".repeat(12));
        assert_eq!(e.project.drawing_count(), 1 + 12 + 12);
        assert!(!e.is_dirty());
    }
}
