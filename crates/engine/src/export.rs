//! Exports: PNG sequence, animated GIF (craft-gif) and OpenRaster for one frame (craft-ora).

use std::path::{Path, PathBuf};

use tooncraft_doc::{Project, RenderOptions, render_frame};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("could not write {0}: {1}")]
    Io(String, std::io::Error),
    #[error("could not encode: {0}")]
    Encode(String),
    #[error("the frame range {0}…{1} is empty or outside the shot")]
    Range(u32, u32),
}

pub type Result<T> = std::result::Result<T, ExportError>;

/// Frames to export, `start..end`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportOptions {
    pub start: u32,
    pub end: u32,
    /// Include the paper (off: transparent background where the format allows it).
    pub paper: bool,
}

impl ExportOptions {
    pub fn whole(p: &Project) -> Self {
        Self { start: 0, end: p.length(), paper: p.paper.export }
    }

    fn check(&self, p: &Project) -> Result<()> {
        if self.start >= self.end || self.end > p.length() {
            return Err(ExportError::Range(self.start, self.end));
        }
        Ok(())
    }
}

/// Write `<dir>/<prefix>_0001.png`, … one PNG per frame. Returns the files written.
pub fn png_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    o.check(p)?;
    std::fs::create_dir_all(dir).map_err(|e| ExportError::Io(dir.display().to_string(), e))?;
    let digits = (o.end.max(1).ilog10() + 1).max(4) as usize;
    let mut out = Vec::new();
    for f in o.start..o.end {
        let img = render_frame(p, f, RenderOptions { paper: o.paper, ..Default::default() });
        let png = tooncraft_format::encode_png(&img).map_err(|e| ExportError::Encode(e.to_string()))?;
        let path = dir.join(format!("{prefix}_{:0digits$}.png", f + 1));
        craft_atomic::atomic_write(&path, &png).map_err(|e| ExportError::Io(path.display().to_string(), e))?;
        out.push(path);
        progress(f);
    }
    Ok(out)
}

/// GIF delays in centiseconds for frames at `fps`, spreading the rounding so the total stays
/// exact (12 fps: 8, 8, 9, 8, 8, 9 …).
pub fn gif_delays(fps: u32, frames: u32) -> Vec<u16> {
    let fps = fps.max(1) as u64;
    (0..frames as u64).map(|i| (((i + 1) * 100 + fps / 2) / fps - (i * 100 + fps / 2) / fps).max(2) as u16).collect()
}

/// Write an animated GIF that loops forever. Frames are encoded one at a time (streaming), so
/// memory does not grow with the shot length.
pub fn gif(p: &Project, path: &Path, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<()> {
    o.check(p)?;
    let delays = gif_delays(p.fps(), o.end - o.start);
    let opts = craft_gif::write::Options::default();
    let mut w = craft_gif::write::AnimationWriter::new(Vec::new(), p.width(), p.height(), opts).map_err(|e| ExportError::Encode(e.to_string()))?;
    for (i, f) in (o.start..o.end).enumerate() {
        let img = render_frame(p, f, RenderOptions { paper: o.paper, ..Default::default() });
        let straight = img.to_straight();
        let delay_cs = delays.get(i).copied().unwrap_or(4);
        w.write_frame(craft_gif::write::RgbaFrame { pixels: &straight, delay_cs }).map_err(|e| ExportError::Encode(e.to_string()))?;
        progress(f);
    }
    let bytes = w.finish().map_err(|e| ExportError::Encode(e.to_string()))?;
    craft_atomic::atomic_write(path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))
}

/// Write one frame as OpenRaster: one image layer per ToonCraft layer, plus the merged image.
pub fn ora_frame(p: &Project, frame: u32, path: &Path) -> Result<()> {
    if frame >= p.length() {
        return Err(ExportError::Range(frame, frame + 1));
    }
    let enc = |e: tooncraft_format::FormatError| ExportError::Encode(e.to_string());
    let ora = |e: craft_ora::Error| ExportError::Encode(e.to_string());
    let mut w = craft_ora::Writer::new();
    let mut stack = Vec::new();
    for (i, layer) in p.layers.iter().enumerate().rev() {
        let img = render_frame(p, frame, RenderOptions { paper: false, only_layer: Some(i), ..Default::default() });
        let src = format!("data/layer{i}.png");
        w.add(&src, &tooncraft_format::encode_png(&img).map_err(enc)?).map_err(ora)?;
        stack.push(craft_ora::Node::Layer(craft_ora::Layer {
            props: craft_ora::Props { name: Some(layer.name.clone()), visible: layer.visible, opacity: layer.opacity, ..Default::default() },
            src: Some(src),
            ..Default::default()
        }));
    }
    let merged = render_frame(p, frame, RenderOptions { paper: p.paper.export, ..Default::default() });
    w.add("mergedimage.png", &tooncraft_format::encode_png(&merged).map_err(enc)?).map_err(ora)?;
    w.add("Thumbnails/thumbnail.png", &tooncraft_format::encode_png(&merged.downscaled(256)).map_err(enc)?).map_err(ora)?;
    let image = craft_ora::Image { version: Some("0.0.6".into()), width: p.width(), height: p.height(), xres: 72.0, yres: 72.0, stack };
    w.add_stack(&image).map_err(ora)?;
    let bytes = w.finish().map_err(ora)?;
    craft_atomic::atomic_write(path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gif_delays_add_up() {
        let d = gif_delays(12, 12);
        assert_eq!(d.iter().map(|v| *v as u32).sum::<u32>(), 100);
        assert_eq!(gif_delays(24, 24).iter().map(|v| *v as u32).sum::<u32>(), 100);
        assert!(gif_delays(100, 5).iter().all(|d| *d >= 2), "browsers slow down delays under 2 cs");
    }
}
