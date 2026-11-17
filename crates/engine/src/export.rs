//! Exports: PNG sequence, animated GIF (craft-gif) and OpenRaster for one frame (craft-ora).

use std::path::{Path, PathBuf};

use tooncraft_doc::{Project, RenderOptions, render_frame};
// Frame exports (PNG, GIF, APNG, MP4) render through the camera; layer exports (ORA) do not.

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
        let img = tooncraft_doc::render_camera(p, f, RenderOptions { paper: o.paper, ..Default::default() });
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
        let img = tooncraft_doc::render_camera(p, f, RenderOptions { paper: o.paper, ..Default::default() });
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
        // The drawing's own pixels: OpenRaster applies the layer opacity from the stack itself.
        let img = layer
            .drawing_at(frame)
            .and_then(|d| p.drawing(d))
            .map(|d| d.to_image())
            .unwrap_or_else(|| tooncraft_raster::RgbaImage::transparent(p.width(), p.height()));
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

/// BT.709 limited-range 4:2:0 planes of a premultiplied image composited over its paper (the
/// caller renders with paper on). Width and height must be even.
fn to_yuv420(img: &tooncraft_raster::RgbaImage, w: u32, h: u32) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (w, h) = (w as usize, h as usize);
    let mut y = vec![16u8; w * h];
    let mut u = vec![128u8; (w / 2) * (h / 2)];
    let mut v = vec![128u8; (w / 2) * (h / 2)];
    let rgb = |x: usize, yy: usize| {
        let p = img.pixel(x as u32, yy as u32);
        // over black for any transparency left (paper is normally opaque)
        (p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0)
    };
    for yy in 0..h {
        for x in 0..w {
            let (r, g, b) = rgb(x, yy);
            let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            y[yy * w + x] = (16.0 + l * 219.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    for cy in 0..h / 2 {
        for cx in 0..w / 2 {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = rgb(cx * 2 + dx, cy * 2 + dy);
                r += c.0 / 4.0;
                g += c.1 / 4.0;
                b += c.2 / 4.0;
            }
            let l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            u[cy * (w / 2) + cx] = (128.0 + (b - l) / 1.8556 * 224.0).round().clamp(0.0, 255.0) as u8;
            v[cy * (w / 2) + cx] = (128.0 + (r - l) / 1.5748 * 224.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    (y, u, v)
}

/// Write an H.264 MP4 (craft-video encoder, craft-media muxer), one sample per shot frame at the
/// shot's frame rate. Odd canvas sizes lose their last column or row (4:2:0 needs even sizes).
pub fn mp4(p: &Project, path: &Path, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<()> {
    use craft_media::isobmff::{AvcConfig, Brand, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions};
    use craft_video::h264::{Encoder, EncoderConfig, PacketFormat, RateControl, YuvFrame};
    o.check(p)?;
    let (w, h) = (p.width() & !1, p.height() & !1);
    if w < 16 || h < 16 {
        return Err(ExportError::Encode("video needs a canvas of at least 16×16".into()));
    }
    let fps = p.fps().max(1);
    let enc_err = |e: craft_video::h264::Error| ExportError::Encode(e.to_string());
    let mux_err = |e: craft_media::isobmff::Error| ExportError::Encode(e.to_string());
    let mut cfg = EncoderConfig::new(w, h, fps, 1);
    cfg.format = PacketFormat::LengthPrefixed;
    cfg.bframes = 0;
    cfg.keyint = fps * 2;
    cfg.rate = RateControl::Crf(18.0);
    cfg.writer = format!("ToonCraft {}", env!("CARGO_PKG_VERSION"));
    let mut enc = Encoder::new(cfg).map_err(enc_err)?;
    // The soundtrack under the exported frames, as AAC in its own M4A, whose track is copied in.
    let sound = match crate::audio::decode_project(p) {
        Some(Ok(pcm)) => {
            let s = p.soundtrack.as_ref().ok_or_else(|| ExportError::Encode("sound".into()))?;
            let seg = crate::audio::segment(&pcm, s, fps, o.start, o.end);
            let buf = craft_audio::AudioBuffer { sample_rate: seg.rate, channels: seg.channels };
            let opts = craft_audio::EncodeOptions::new(craft_audio::EncodeFormat::Aac);
            Some(craft_audio::encode(&buf, &opts).map_err(|e| ExportError::Encode(format!("sound: {e}")))?)
        }
        Some(Err(e)) => return Err(ExportError::Encode(format!("sound: {e}"))),
        None => None,
    };
    let entry = |enc: &Encoder| -> Result<SampleEntry> { Ok(SampleEntry::avc(AvcConfig::parse(&enc.avcc()).map_err(mux_err)?, w as u16, h as u16)) };
    let result = craft_atomic::atomic_write_with(path, |file| {
        let io = |e: ExportError| std::io::Error::other(e.to_string());
        let mut mux = Mp4Writer::new(file, WriterOptions::new(Brand::Mp4)).map_err(|e| io(mux_err(e)))?;
        let track = mux.add_track(TrackConfig::new(entry(&enc).map_err(io)?, fps)).map_err(|e| io(mux_err(e)))?;
        let write = |mux: &mut Mp4Writer<&mut std::fs::File>, packets: Vec<craft_video::h264::Packet>| -> std::io::Result<()> {
            for pk in packets {
                let s = WriteSample {
                    data: &pk.data,
                    duration: 1,
                    composition_offset: (pk.pts - pk.dts).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    is_sync: pk.keyframe,
                };
                mux.write_sample(track, s).map_err(|e| io(mux_err(e)))?;
            }
            Ok(())
        };
        for (i, f) in (o.start..o.end).enumerate() {
            let img = tooncraft_doc::render_camera(p, f, RenderOptions { paper: true, ..Default::default() });
            let (y, u, v) = to_yuv420(&img, w, h);
            let frame = YuvFrame { y: &y, u: &u, v: &v, y_stride: w as usize, uv_stride: w as usize / 2 };
            let packets = enc.encode(&frame, i as i64).map_err(|e| io(enc_err(e)))?;
            write(&mut mux, packets)?;
            progress(f);
        }
        let packets = enc.flush();
        write(&mut mux, packets)?;
        mux.set_sample_entry(track, entry(&enc).map_err(io)?).map_err(|e| io(mux_err(e)))?;
        if let Some(aac) = &sound {
            add_aac_track(&mut mux, aac).map_err(io)?;
        }
        mux.finish().map_err(|e| io(mux_err(e)))?;
        Ok(())
    });
    result.map_err(|e| ExportError::Io(path.display().to_string(), e))
}

/// Copy the audio track of an M4A into the movie being written.
fn add_aac_track<W: std::io::Write + std::io::Seek>(mux: &mut craft_media::isobmff::Mp4Writer<W>, m4a: &[u8]) -> Result<()> {
    use craft_media::isobmff::{TrackConfig, WriteSample};
    let e = |e: craft_media::isobmff::Error| ExportError::Encode(format!("sound: {e}"));
    let file = craft_media::isobmff::open(m4a).map_err(e)?;
    let Some((index, t)) = file.tracks.iter().enumerate().find(|(_, t)| !t.entries.is_empty() && !t.samples.is_empty()) else { return Ok(()) };
    let Some(entry) = t.entries.first().cloned() else { return Ok(()) };
    let mut cfg = TrackConfig::new(entry, t.timescale);
    cfg.media_start = t.edits.iter().find(|ed| ed.media_time >= 0).map(|ed| ed.media_time);
    cfg.roll_distance = Some(-1);
    let track = mux.add_track(cfg).map_err(e)?;
    for (i, smp) in t.samples.iter().enumerate() {
        let data = file.read_sample(m4a, index, i).map_err(e)?;
        mux.write_sample(track, WriteSample { data: &data, duration: smp.duration, composition_offset: 0, is_sync: smp.is_sync }).map_err(e)?;
    }
    Ok(())
}

/// Concatenated IDAT payload of a PNG (APNG frames carry it).
fn idat(png: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = 8usize;
    while at + 12 <= png.len() {
        let len = u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]) as usize;
        let kind = png.get(at + 4..at + 8)?;
        let data = png.get(at + 8..at + 8 + len)?;
        if kind == b"IDAT" {
            out.extend_from_slice(data);
        }
        at += 12 + len;
    }
    (!out.is_empty()).then_some(out)
}

/// Write an animated PNG (lossless, transparent background possible). Frames are held in memory
/// compressed; capped at 500 MB of output.
pub fn apng(p: &Project, path: &Path, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<()> {
    o.check(p)?;
    let delays = gif_delays(p.fps(), o.end - o.start);
    let mut payloads = Vec::new();
    for f in o.start..o.end {
        let img = tooncraft_doc::render_camera(p, f, RenderOptions { paper: o.paper, ..Default::default() });
        let png = tooncraft_format::encode_png(&img).map_err(|e| ExportError::Encode(e.to_string()))?;
        payloads.push(idat(&png).ok_or_else(|| ExportError::Encode("PNG without image data".into()))?);
        progress(f);
    }
    let frames: Vec<craft_image_codecs::apng::Frame<'_>> = payloads
        .iter()
        .enumerate()
        .map(|(i, d)| craft_image_codecs::apng::Frame {
            width: p.width(),
            height: p.height(),
            x: 0,
            y: 0,
            delay_num: delays.get(i).copied().unwrap_or(4),
            delay_den: 100,
            over: false,
            data: d,
        })
        .collect();
    let bytes = craft_image_codecs::apng::encode(p.width(), p.height(), 0, &frames, 500 << 20).map_err(|e| ExportError::Encode(e.to_string()))?;
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
