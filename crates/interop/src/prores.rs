//! Apple ProRes in a QuickTime movie, the usual hand-off to editing and compositing. The codec
//! is craft-video's clean-room encoder (SMPTE RDD 36), the container craft-media's MOV writer.
//!
//! - `alpha: true`: ProRes 4444 (`ap4h`), 12-bit 4:4:4 with a 16-bit-coded alpha channel; the
//!   frames have no paper. Colour is stored straight (not premultiplied) — the usual
//!   interpretation of ProRes 4444 alpha; set "straight" if the compositor asks.
//! - `alpha: false`: ProRes 422 HQ (`apch`), 10-bit 4:2:2, the frames composited on the paper.
//!
//! Both are progressive, BT.709 video range, one sample per shot frame at the shot's frame rate
//! (media timescale = fps, every sample a sync sample). Any canvas size works: the encoder pads
//! partial macroblocks.

use std::path::Path;

use craft_media::isobmff::{Brand, FourCc, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions};
use craft_video::prores::{ChromaFormat, Encoder, EncoderConfig, Frame, Profile};
use tooncraft_doc::{Project, RenderOptions, render_frame};
use tooncraft_raster::{RgbaImage, unpremultiply};

use crate::common::check_range;
use crate::{InteropError, Result};

/// BT.709 video-range Y, Cb, Cr of an RGB colour (0…1) at `depth` bits.
fn ycbcr(r: f32, g: f32, b: f32, depth: u8) -> (u16, u16, u16) {
    let scale = (1u32 << (depth - 8)) as f32;
    let max = ((1u32 << depth) - 1) as f32;
    let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let q = |v: f32| (v * scale).round().clamp(0.0, max) as u16;
    (q(16.0 + 219.0 * l), q(128.0 + 224.0 * (b - l) / 1.8556), q(128.0 + 224.0 * (r - l) / 1.5748))
}

/// Planar ProRes input for a premultiplied picture.
fn to_frame(img: &RgbaImage, chroma: ChromaFormat, depth: u8, alpha: bool) -> Result<Frame> {
    let err = |e: craft_video::prores::Error| InteropError::Encode(e.to_string());
    let (w, h) = (img.width as usize, img.height as usize);
    let mut f = Frame::new(img.width, img.height, chroma, depth, alpha).map_err(err)?;
    let cw = f.chroma_width() as usize;
    let amax = ((1u32 << depth) - 1) as f32;
    let rgb = |x: usize, y: usize| -> [f32; 3] {
        let p = img.pixel(x as u32, y as u32);
        // Straight colour with alpha; without, the premultiplied picture is over black.
        let c = if alpha { unpremultiply(p) } else { p };
        [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
    };
    for y in 0..h {
        for x in 0..w {
            let [r, g, b] = rgb(x, y);
            let (yy, cb, cr) = ycbcr(r, g, b, depth);
            if let Some(s) = f.y.get_mut(y * w + x) {
                *s = yy;
            }
            if chroma == ChromaFormat::Yuv444
                && let (Some(a), Some(b)) = (f.cb.get_mut(y * cw + x), f.cr.get_mut(y * cw + x))
            {
                (*a, *b) = (cb, cr);
            }
            if let Some(a) = f.alpha.as_mut().and_then(|a| a.get_mut(y * w + x)) {
                *a = (img.pixel(x as u32, y as u32)[3] as f32 / 255.0 * amax).round() as u16;
            }
        }
        if chroma == ChromaFormat::Yuv422 {
            for cx in 0..cw {
                let a = rgb(cx * 2, y);
                let b = if cx * 2 + 1 < w { rgb(cx * 2 + 1, y) } else { a };
                let (_, cb, cr) = ycbcr((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0, depth);
                if let (Some(u), Some(v)) = (f.cb.get_mut(y * cw + cx), f.cr.get_mut(y * cw + cx)) {
                    (*u, *v) = (cb, cr);
                }
            }
        }
    }
    Ok(f)
}

/// Write frames `start..end` as a ProRes QuickTime movie: 4444 with alpha when `alpha`, else
/// 422 HQ on the paper. The file is replaced atomically.
pub fn export_mov(p: &Project, path: &Path, start: u32, end: u32, alpha: bool) -> Result<()> {
    write_mov(p, path, start, end, alpha, None)
}

/// One ProRes 4444 movie with alpha per visible layer, `<dir>/<Layer>.mov`, each with the layer's
/// effects and opacity baked in (as the per-layer PNG sequences). Returns the files written.
pub fn export_layer_movs(p: &Project, dir: &Path, start: u32, end: u32) -> Result<Vec<std::path::PathBuf>> {
    check_range(p, start, end)?;
    crate::common::create_dir(dir)?;
    let names = crate::common::track_names(&p.layers);
    let mut out = Vec::new();
    for (i, layer) in p.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        let name = crate::safe_file_name(names.get(i).map_or(layer.name.as_str(), String::as_str));
        let path = dir.join(format!("{name}.mov"));
        write_mov(p, &path, start, end, true, Some(i))?;
        out.push(path);
    }
    Ok(out)
}

fn write_mov(p: &Project, path: &Path, start: u32, end: u32, alpha: bool, only_layer: Option<usize>) -> Result<()> {
    check_range(p, start, end)?;
    let (w, h) = (p.width(), p.height());
    let (profile, depth) = if alpha { (Profile::P4444, 12) } else { (Profile::Hq, 10) };
    let mut cfg = EncoderConfig::new(profile, w, h);
    cfg.encode_alpha = alpha;
    cfg.threads = false;
    let mut enc = Encoder::with_config(cfg);
    let fps = p.fps().max(1);
    let entry = SampleEntry::prores(FourCc(profile.fourcc()), w.min(u16::MAX as u32) as u16, h.min(u16::MAX as u32) as u16);
    let io = |e: InteropError| std::io::Error::other(e.to_string());
    let mux = |e: craft_media::isobmff::Error| io(InteropError::Encode(e.to_string()));
    let written = craft_atomic::atomic_write_with(path, |file| {
        let mut opts = WriterOptions::new(Brand::Mov);
        opts.metadata.push(("©too".into(), format!("ToonCraft {}", env!("CARGO_PKG_VERSION"))));
        let mut m = Mp4Writer::new(file, opts).map_err(mux)?;
        let track = m.add_track(TrackConfig::new(entry, fps)).map_err(mux)?;
        for f in start..end {
            let img = render_frame(p, f, RenderOptions { paper: !alpha, only_layer, ..Default::default() });
            let frame = to_frame(&img, profile.chroma(), depth, alpha).map_err(io)?;
            let data = enc.encode(&frame).map_err(|e| io(InteropError::Encode(e.to_string())))?;
            m.write_sample(track, WriteSample { data: &data, duration: 1, composition_offset: 0, is_sync: true }).map_err(mux)?;
        }
        m.finish().map_err(mux)?;
        Ok(())
    });
    written.map_err(|e| InteropError::Io(path.display().to_string(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{shot, temp_dir};
    use craft_media::isobmff::TrackKind;

    fn check(alpha: bool, fourcc: &[u8; 4]) {
        let p = shot();
        let dir = temp_dir(if alpha { "prores-4444" } else { "prores-422" });
        let path = dir.join("shot.mov");
        export_mov(&p, &path, 1, 5, alpha).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mov = craft_media::isobmff::open(bytes.as_slice()).unwrap();
        assert!(mov.is_quicktime);
        assert_eq!(mov.tracks.len(), 1);
        let t = &mov.tracks[0];
        assert_eq!(t.kind, TrackKind::Video);
        assert_eq!(t.samples.len(), 4);
        assert_eq!(t.timescale, 12);
        assert_eq!((t.width, t.height), (40, 30));
        assert_eq!(t.entries[0].format.0, *fourcc);
        let sample = mov.read_sample(bytes.as_slice(), 0, 1).unwrap();
        let frame = craft_video::prores::decode_frame(&sample).unwrap();
        assert_eq!((frame.width, frame.height), (40, 30));
        // frame 2 shows the green drawing at (20,10)…(25,15) at half opacity
        let d = frame.bit_depth;
        let luma = |x: usize, y: usize| frame.y[y * 40 + x] as f32 / (1u32 << (d - 8)) as f32;
        if alpha {
            let a = frame.alpha.as_ref().expect("4444 keeps the alpha channel");
            let amax = ((1u32 << d) - 1) as f32;
            assert!(a[0] as f32 / amax < 0.02, "no paper: transparent");
            assert!((a[12 * 40 + 22] as f32 / amax - 0.5).abs() < 0.03);
            assert!((luma(22, 12) - (16.0 + 219.0 * 0.7152)).abs() < 6.0, "straight green");
        } else {
            assert!(frame.alpha.is_none());
            assert!(luma(0, 0) > 200.0, "paper");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prores_4444_with_alpha() {
        check(true, b"ap4h");
    }

    #[test]
    fn prores_422_hq() {
        check(false, b"apch");
    }

    #[test]
    fn bad_range_and_path() {
        let p = shot();
        assert!(matches!(export_mov(&p, Path::new("x.mov"), 4, 2, false), Err(InteropError::Range(4, 2))));
        let dir = temp_dir("prores-bad");
        assert!(matches!(export_mov(&p, &dir.join("missing/dir/x.mov"), 0, 1, false), Err(InteropError::Io(..))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn colour_conversion() {
        assert_eq!(ycbcr(0.0, 0.0, 0.0, 10), (64, 512, 512));
        assert_eq!(ycbcr(1.0, 1.0, 1.0, 10), (940, 512, 512));
        assert_eq!(ycbcr(1.0, 1.0, 1.0, 12), (3760, 2048, 2048));
    }

    #[test]
    fn one_movie_per_layer_with_effects() {
        let mut p = shot();
        p.layers[0].effects = vec![tooncraft_fx::Effect::new(tooncraft_fx::EffectKind::Blur { radius: 2.0 })];
        let dir = temp_dir("prores-layers");
        let files = export_layer_movs(&p, &dir, 0, 2).unwrap();
        assert_eq!(files.len(), p.layers.iter().filter(|l| l.visible).count());
        for f in &files {
            let bytes = std::fs::read(f).unwrap();
            let mov = craft_media::isobmff::open(bytes.as_slice()).unwrap();
            assert_eq!(mov.tracks.len(), 1);
        }
    }
}
