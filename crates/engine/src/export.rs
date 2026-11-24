//! Exports: PNG, TGA and TIFF sequences, animated GIF (craft-gif), animated and still WebP
//! (craft-webp, craft-image-codecs) and OpenRaster for one frame (craft-ora).

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
    /// Output size divisor: 1 is the canvas size, 2 half, 4 a quarter (1 … [`MAX_SCALE`]).
    pub scale: u32,
    /// Animated GIF: loop forever (off: play once).
    pub loop_forever: bool,
    /// Image sequences: digits in the frame number (0: at least 4, more for long shots).
    pub digits: u8,
    /// Review copies: the shot name, frame and timecode in a bar along the bottom.
    pub burn_in: bool,
}

/// The largest output size divisor.
pub const MAX_SCALE: u32 = 8;

impl Default for ExportOptions {
    fn default() -> Self {
        Self { start: 0, end: 1, paper: true, scale: 1, loop_forever: true, digits: 0, burn_in: false }
    }
}

impl ExportOptions {
    pub fn whole(p: &Project) -> Self {
        Self { start: 0, end: p.length(), paper: p.paper.export, ..Self::default() }
    }

    fn check(&self, p: &Project) -> Result<()> {
        if self.start >= self.end || self.end > p.length() {
            return Err(ExportError::Range(self.start, self.end));
        }
        if self.scale == 0 || self.scale > MAX_SCALE || self.digits > 9 {
            return Err(ExportError::Encode(format!("output scale must be 1 to {MAX_SCALE} and digits 0 to 9")));
        }
        Ok(())
    }

    /// The output size for `p`.
    pub fn size(&self, p: &Project) -> (u32, u32) {
        let s = self.scale.clamp(1, MAX_SCALE);
        (p.width().div_ceil(s).max(1), p.height().div_ceil(s).max(1))
    }

    /// Frame `f` through the camera, at the output size.
    fn render(&self, p: &Project, f: u32, paper: bool) -> tooncraft_raster::RgbaImage {
        let mut img = tooncraft_doc::render_camera(p, f, RenderOptions { paper, ..Default::default() }).reduced(self.scale.clamp(1, MAX_SCALE));
        if self.burn_in {
            crate::burnin::stamp(&mut img, &crate::burnin::text_for(p, f));
        }
        img
    }

    /// Digits for the frame numbers of a sequence.
    fn digit_count(&self) -> usize {
        let needed = (self.end.max(1).ilog10() + 1) as usize;
        if self.digits == 0 { needed.max(4) } else { (self.digits as usize).max(needed) }
    }
}

/// Write `<dir>/<prefix>_0001.png`, … one PNG per frame. Returns the files written.
pub fn png_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    o.check(p)?;
    std::fs::create_dir_all(dir).map_err(|e| ExportError::Io(dir.display().to_string(), e))?;
    let digits = o.digit_count();
    let mut out = Vec::new();
    for f in o.start..o.end {
        let img = o.render(p, f, o.paper);
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

/// Write an animated GIF (looping forever unless `o.loop_forever` is off). Frames are encoded one at a time (streaming), so
/// memory does not grow with the shot length.
pub fn gif(p: &Project, path: &Path, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<()> {
    o.check(p)?;
    let delays = gif_delays(p.fps(), o.end - o.start);
    let opts = craft_gif::write::Options { loop_forever: o.loop_forever, ..Default::default() };
    let (ow, oh) = o.size(p);
    let mut w = craft_gif::write::AnimationWriter::new(Vec::new(), ow, oh, opts).map_err(|e| ExportError::Encode(e.to_string()))?;
    for (i, f) in (o.start..o.end).enumerate() {
        let img = o.render(p, f, o.paper);
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
    let (w, h) = o.size(p);
    let (w, h) = (w & !1, h & !1);
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
    // The sound mix under the exported frames, as AAC in its own M4A, whose track is copied in.
    let sound = match crate::audio::mix_project(p, o.start, o.end) {
        Some(Ok(seg)) => {
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
            let img = o.render(p, f, true);
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
        let img = o.render(p, f, o.paper);
        let png = tooncraft_format::encode_png(&img).map_err(|e| ExportError::Encode(e.to_string()))?;
        payloads.push(idat(&png).ok_or_else(|| ExportError::Encode("PNG without image data".into()))?);
        progress(f);
    }
    let frames: Vec<craft_image_codecs::apng::Frame<'_>> = payloads
        .iter()
        .enumerate()
        .map(|(i, d)| craft_image_codecs::apng::Frame {
            width: o.size(p).0,
            height: o.size(p).1,
            x: 0,
            y: 0,
            delay_num: delays.get(i).copied().unwrap_or(4),
            delay_den: 100,
            over: false,
            data: d,
        })
        .collect();
    let bytes = craft_image_codecs::apng::encode(o.size(p).0, o.size(p).1, 0, &frames, 500 << 20).map_err(|e| ExportError::Encode(e.to_string()))?;
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

// ----- WebP, TGA and TIFF -----

/// How a WebP is encoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WebpOptions {
    /// Lossless (VP8L) keeps every pixel; lossy (VP8) is much smaller.
    pub lossless: bool,
    /// Lossy quality, 0 … 100.
    pub quality: u8,
}

impl Default for WebpOptions {
    fn default() -> Self {
        Self { lossless: true, quality: 80 }
    }
}

/// Most compressed bytes an animated WebP may hold in memory while it is assembled.
const MAX_WEBP_BYTES: usize = 1 << 30;

/// Frame times in milliseconds for frames at `fps`, spreading the rounding so the total stays
/// exact (24 fps: 41, 42, 42, 41, …).
pub fn frame_delays_ms(fps: u32, frames: u32) -> Vec<u32> {
    let fps = fps.max(1) as u64;
    (0..frames as u64).map(|i| (((i + 1) * 1000 + fps / 2) / fps - (i * 1000 + fps / 2) / fps).max(1) as u32).collect()
}

fn codec_err(e: impl std::fmt::Display) -> ExportError {
    ExportError::Encode(e.to_string())
}

fn straight_image(img: &tooncraft_raster::RgbaImage, alpha: bool) -> Result<craft_image_codecs::Image> {
    use craft_image_codecs::{ChannelLayout, Image, SampleType};
    let rgba = img.to_straight();
    if alpha {
        Image::from_raw(img.width, img.height, ChannelLayout::Rgba, SampleType::U8, rgba).map_err(codec_err)
    } else {
        let rgb: Vec<u8> = rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
        Image::from_raw(img.width, img.height, ChannelLayout::Rgb, SampleType::U8, rgb).map_err(codec_err)
    }
}

/// One frame (through the camera) as a still WebP.
pub fn webp_frame(p: &Project, frame: u32, path: &Path, o: WebpOptions, paper: bool) -> Result<()> {
    if frame >= p.length() {
        return Err(ExportError::Range(frame, frame.saturating_add(1)));
    }
    let img = tooncraft_doc::render_camera(p, frame, RenderOptions { paper, ..Default::default() });
    let eo = craft_image_codecs::EncodeOptions { webp_lossless: o.lossless, webp_quality: o.quality.min(100), ..Default::default() };
    let bytes = craft_image_codecs::encode(&straight_image(&img, !paper)?, craft_image_codecs::Format::WebP, &eo).map_err(codec_err)?;
    craft_atomic::atomic_write(path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))
}

/// The payload of the first chunk called `id` in a RIFF/WebP file.
fn riff_chunk<'a>(file: &'a [u8], id: &[u8; 4]) -> Option<&'a [u8]> {
    let mut at = 12usize;
    while at + 8 <= file.len() {
        let size = u32::from_le_bytes(file.get(at + 4..at + 8)?.try_into().ok()?) as usize;
        let body = file.get(at + 8..at + 8 + size)?;
        if &file[at..at + 4] == id {
            return Some(body);
        }
        at += 8 + size + (size & 1);
    }
    None
}

/// Write an animated WebP that loops forever. Frames are encoded one at a time; a frame equal to
/// the one before it only extends that frame's time, so held drawings cost nothing.
pub fn webp_animated(p: &Project, path: &Path, o: ExportOptions, opt: WebpOptions, mut progress: impl FnMut(u32)) -> Result<()> {
    o.check(p)?;
    let (w, h) = o.size(p);
    if !opt.lossless && (w > 16383 || h > 16383) {
        return Err(ExportError::Encode("lossy WebP is at most 16383 pixels a side; use lossless".into()));
    }
    let delays = frame_delays_ms(p.fps(), o.end - o.start);
    enum Coded {
        Lossless(Vec<u8>),
        Lossy(Vec<u8>, Option<Vec<u8>>),
    }
    let mut frames: Vec<(Coded, u32)> = Vec::new();
    let mut last: Option<Vec<u8>> = None;
    let mut bytes = 0usize;
    let mut any_alpha = false;
    for (i, f) in (o.start..o.end).enumerate() {
        let img = o.render(p, f, o.paper);
        let rgba = img.to_straight();
        let ms = delays.get(i).copied().unwrap_or(42);
        if last.as_deref() == Some(rgba.as_slice())
            && let Some(prev) = frames.last_mut()
        {
            prev.1 = prev.1.saturating_add(ms).min((1 << 24) - 1);
            progress(f);
            continue;
        }
        let alpha_used = rgba.as_chunks::<4>().0.iter().any(|px| px[3] != 255);
        any_alpha |= alpha_used;
        let coded = if opt.lossless {
            let eo = craft_image_codecs::EncodeOptions { webp_lossless: true, ..Default::default() };
            let still = craft_image_codecs::encode(&straight_image(&img, true)?, craft_image_codecs::Format::WebP, &eo).map_err(codec_err)?;
            let vp8l = riff_chunk(&still, b"VP8L").ok_or_else(|| ExportError::Encode("the WebP encoder made no lossless frame".into()))?;
            bytes += vp8l.len();
            Coded::Lossless(vp8l.to_vec())
        } else {
            let params = craft_video::vp8::Vp8Params { quality: opt.quality.min(100), loop_filter: None };
            let vp8 = craft_video::vp8::encode_rgba(&rgba, w, h, &params).map_err(codec_err)?.data;
            let alpha: Vec<u8> = rgba.iter().skip(3).step_by(4).copied().collect();
            let alpha = alpha_used.then_some(alpha);
            bytes += vp8.len() + alpha.as_ref().map_or(0, Vec::len);
            Coded::Lossy(vp8, alpha)
        };
        if bytes > MAX_WEBP_BYTES {
            return Err(ExportError::Encode("the animation is too large to assemble; export a shorter range".into()));
        }
        frames.push((coded, ms));
        last = Some(rgba);
        progress(f);
    }
    let list: Vec<craft_webp::anim::AnimFrame> = frames
        .iter()
        .map(|(c, ms)| craft_webp::anim::AnimFrame {
            x: 0,
            y: 0,
            width: w,
            height: h,
            duration_ms: *ms,
            blend: false,
            dispose_to_background: false,
            payload: match c {
                Coded::Lossless(b) => craft_webp::anim::Payload::Lossless { vp8l: b },
                Coded::Lossy(b, a) => craft_webp::anim::Payload::Lossy { vp8: b, alpha: a.as_deref() },
            },
        })
        .collect();
    let opts = craft_webp::anim::AnimOptions { loop_count: 0, alpha: any_alpha, ..Default::default() };
    let bytes = craft_webp::anim::animation(w, h, &list, &opts).map_err(codec_err)?;
    craft_atomic::atomic_write(path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))
}

/// Write one file per frame: `<dir>/<prefix>_0001.<ext>`, each made by `encode` from the frame
/// rendered through the camera. Returns the files written.
fn frame_files(
    p: &Project,
    dir: &Path,
    prefix: &str,
    ext: &str,
    o: ExportOptions,
    mut progress: impl FnMut(u32),
    encode: impl Fn(&tooncraft_raster::RgbaImage, bool) -> Result<Vec<u8>>,
) -> Result<Vec<PathBuf>> {
    o.check(p)?;
    std::fs::create_dir_all(dir).map_err(|e| ExportError::Io(dir.display().to_string(), e))?;
    let digits = o.digit_count();
    let mut out = Vec::new();
    for f in o.start..o.end {
        let img = o.render(p, f, o.paper);
        let bytes = encode(&img, !o.paper)?;
        let path = dir.join(format!("{prefix}_{:0digits$}.{ext}", f + 1));
        craft_atomic::atomic_write(&path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))?;
        out.push(path);
        progress(f);
    }
    Ok(out)
}

/// `<dir>/<prefix>_0001.tga`, … run-length encoded Targa files (with alpha when the paper is off).
pub fn tga_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    frame_files(p, dir, prefix, "tga", o, progress, |img, alpha| {
        let rgba = img.to_straight();
        if alpha {
            craft_tga::encode(img.width, img.height, craft_tga::Layout::Rgba, &rgba).map_err(codec_err)
        } else {
            let rgb: Vec<u8> = rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
            craft_tga::encode(img.width, img.height, craft_tga::Layout::Rgb, &rgb).map_err(codec_err)
        }
    })
}

/// `<dir>/<prefix>_0001.tif`, … TIFF files, LZW-compressed (with alpha when the paper is off).
/// Write `<dir>/<prefix>_0001.jpg`, …: JPEG at `quality` (1…100) on the paper (JPEG has no
/// transparency, so the paper is always under it).
pub fn jpeg_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, quality: u8, progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    let o = ExportOptions { paper: true, ..o };
    frame_files(p, dir, prefix, "jpg", o, progress, |img, _| {
        let eo = craft_image_codecs::EncodeOptions { jpeg_quality: quality.clamp(1, 100), ..Default::default() };
        craft_image_codecs::encode(&straight_image(img, false)?, craft_image_codecs::Format::Jpeg, &eo).map_err(codec_err)
    })
}

/// Write `<dir>/<prefix>_0001.exr`, …: OpenEXR frames (one flat RGBA image each) for compositing.
pub fn exr_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    frame_files(p, dir, prefix, "exr", o, progress, |img, alpha| {
        craft_image_codecs::encode(&straight_image(img, alpha)?, craft_image_codecs::Format::OpenExr, &Default::default()).map_err(codec_err)
    })
}

/// Samples one layered OpenEXR frame may hold in memory (all layers and the composite).
pub const MAX_LAYERED_EXR_SAMPLES: usize = 1 << 28;

/// Write `<dir>/<prefix>_0001.exr`, …: one OpenEXR file per frame holding the composite as the
/// main picture (`R`, `G`, `B`, `A`) and every visible layer, through the camera, as a layer of
/// its own (`<name>.R`, `<name>.G`, …). Samples are linear and premultiplied, as compositing
/// applications expect.
pub fn exr_layered_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, mut progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    o.check(p)?;
    let names = exr_layer_names(p);
    let (w, h) = o.size(p);
    let pictures = names.iter().flatten().count() + 1;
    let samples = (w as usize).saturating_mul(h as usize).saturating_mul(4).saturating_mul(pictures);
    if samples > MAX_LAYERED_EXR_SAMPLES {
        return Err(ExportError::Encode("too many layers at this size for one file; export at a smaller size or hide some layers".into()));
    }
    std::fs::create_dir_all(dir).map_err(|e| ExportError::Io(dir.display().to_string(), e))?;
    let digits = o.digit_count();
    let linear = srgb_to_linear_table();
    let mut out = Vec::new();
    for f in o.start..o.end {
        let mut channels = exr_channels("", &o.render(p, f, o.paper), &linear);
        for (i, name) in names.iter().enumerate() {
            let Some(name) = name else { continue };
            let img = tooncraft_doc::render_camera(p, f, RenderOptions { paper: false, only_layer: Some(i), ..Default::default() })
                .reduced(o.scale.clamp(1, MAX_SCALE));
            channels.extend(exr_channels(name, &img, &linear));
        }
        let bytes = craft_exr::decode::write(w, h, &channels, craft_exr::decode::Compression::Zip16).map_err(codec_err)?;
        let path = dir.join(format!("{prefix}_{:0digits$}.exr", f + 1));
        craft_atomic::atomic_write(&path, &bytes).map_err(|e| ExportError::Io(path.display().to_string(), e))?;
        out.push(path);
        progress(f);
    }
    Ok(out)
}

/// The OpenEXR layer name of each layer of `p` (`None` for hidden layers): dots become
/// underscores (a dot separates layer and channel), an empty name becomes `Layer N`, and a name
/// already taken gets ` 2`, ` 3`, …
fn exr_layer_names(p: &Project) -> Vec<Option<String>> {
    let mut taken = std::collections::HashSet::new();
    p.layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if !l.visible {
                return None;
            }
            let base: String = l.name.trim().chars().filter(|c| !c.is_control()).map(|c| if c == '.' { '_' } else { c }).take(200).collect();
            let base = if base.is_empty() { format!("Layer {}", i + 1) } else { base };
            let mut name = base.clone();
            let mut n = 2;
            while !taken.insert(name.clone()) {
                name = format!("{base} {n}");
                n += 1;
            }
            Some(name)
        })
        .collect()
}

/// sRGB 8-bit to linear light.
fn srgb_to_linear_table() -> [f32; 256] {
    let mut t = [0.0f32; 256];
    for (i, v) in t.iter_mut().enumerate() {
        let c = i as f32 / 255.0;
        *v = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    }
    t
}

/// Four half-float channels (`<layer>.R` … or `R` … for the main picture) from a premultiplied
/// sRGB image: un-premultiplied, made linear, premultiplied again.
fn exr_channels(layer: &str, img: &tooncraft_raster::RgbaImage, linear: &[f32; 256]) -> Vec<craft_exr::decode::Channel> {
    use craft_exr::decode::{Channel, Samples, f16};
    let straight = img.to_straight();
    let n = straight.len() / 4;
    let mut planes: [Vec<f16>; 4] = std::array::from_fn(|_| Vec::with_capacity(n));
    for px in straight.as_chunks::<4>().0 {
        let a = f32::from(px[3]) / 255.0;
        for c in 0..3 {
            planes[c].push(f16::from_f32(linear[px[c] as usize] * a));
        }
        planes[3].push(f16::from_f32(a));
    }
    let name = |c: &str| if layer.is_empty() { c.to_string() } else { format!("{layer}.{c}") };
    let [r, g, b, a] = planes;
    vec![
        Channel { name: name("R"), samples: Samples::F16(r) },
        Channel { name: name("G"), samples: Samples::F16(g) },
        Channel { name: name("B"), samples: Samples::F16(b) },
        Channel { name: name("A"), samples: Samples::F16(a) },
    ]
}

pub fn tiff_sequence(p: &Project, dir: &Path, prefix: &str, o: ExportOptions, progress: impl FnMut(u32)) -> Result<Vec<PathBuf>> {
    frame_files(p, dir, prefix, "tif", o, progress, |img, alpha| {
        let eo = craft_image_codecs::EncodeOptions { tiff_compression: craft_image_codecs::TiffCompression::Lzw, ..Default::default() };
        craft_image_codecs::encode(&straight_image(img, alpha)?, craft_image_codecs::Format::Tiff, &eo).map_err(codec_err)
    })
}
