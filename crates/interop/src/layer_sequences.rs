//! Per-layer PNG sequences for compositing.
//!
//! ```text
//! <dir>/<Layer>/<Layer>_0001.png     frame 1 of the layer
//! <dir>/<Layer>/<Layer>_0002.png
//! ```
//!
//! One file per frame of the range for every visible layer, numbered from the shot's first
//! frame (frame 1 is `_0001`, at least four digits), transparent background, no paper. The
//! layer's opacity is baked into the pixels so the sequence composites like the shot; blank
//! frames are written as fully transparent pictures so every sequence is complete. Folder names
//! are the layer names made unique and safe for the file system, as in [`crate::levels`].

use std::path::{Path, PathBuf};

use tooncraft_doc::{Project, RenderOptions, render_frame};

use crate::common::{check_range, create_dir, encode_png, track_names, write_file};
use crate::{Result, safe_file_name};

/// Write the sequences of frames `start..end`. `progress` is called after each frame with the
/// frame number. Returns the files written, layer by layer (bottom first).
pub fn export(p: &Project, dir: &Path, start: u32, end: u32, mut progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    check_range(p, start, end)?;
    let digits = (end.ilog10() + 1).max(4) as usize;
    let names = track_names(&p.layers);
    let mut folders = Vec::new();
    for (i, layer) in p.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        let name = safe_file_name(names.get(i).map_or(layer.name.as_str(), String::as_str));
        let folder = dir.join(&name);
        create_dir(&folder)?;
        folders.push((i, name, folder));
    }
    let mut out = vec![Vec::new(); folders.len()];
    for f in start..end {
        for ((i, name, folder), files) in folders.iter().zip(out.iter_mut()) {
            let img = render_frame(p, f, RenderOptions { paper: false, only_layer: Some(*i), ..Default::default() });
            let path = folder.join(format!("{name}_{:0digits$}.png", f + 1));
            write_file(&path, &encode_png(&img)?)?;
            files.push(path);
        }
        progress(f);
    }
    Ok(out.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InteropError;
    use crate::common::decode_png;
    use crate::testutil::{shot, temp_dir};

    #[test]
    fn writes_one_sequence_per_visible_layer() {
        let mut p = shot();
        p.layers[1].visible = true;
        p.layers[1].name = "../Colour".into();
        let dir = temp_dir("sequences");
        let mut seen = Vec::new();
        let files = export(&p, &dir, 1, 4, |f| seen.push(f)).unwrap();
        assert_eq!(seen, [1, 2, 3]);
        let rel: Vec<String> = files.iter().map(|f| f.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(
            rel,
            ["Ink/Ink_0002.png", "Ink/Ink_0003.png", "Ink/Ink_0004.png", "_Colour/_Colour_0002.png", "_Colour/_Colour_0003.png", "_Colour/_Colour_0004.png"]
        );
        let ink = decode_png(&std::fs::read(&files[1]).unwrap(), 100).unwrap();
        assert_eq!((ink.width, ink.height), (40, 30));
        assert_eq!(ink.pixel(0, 0), [0; 4], "transparent background");
        assert_eq!(ink.pixel(21, 11)[3], 128, "the layer's opacity is baked in");
        assert!(matches!(export(&p, &dir, 3, 3, |_| {}), Err(InteropError::Range(3, 3))));
        assert!(matches!(export(&p, &dir, 0, 7, |_| {}), Err(InteropError::Range(0, 7))));
        p.layers[0].visible = false;
        p.layers[1].visible = false;
        assert!(export(&p, &dir, 0, 1, |_| {}).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
