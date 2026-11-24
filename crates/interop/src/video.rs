//! Video file import: the pictures of an MP4 or QuickTime movie (ISO/IEC 14496-12 and -14, the
//! QuickTime file format), demuxed by craft-media and decoded by craft-video's H.264 (ITU-T
//! H.264) and H.265 (ITU-T H.265) decoders.
//!
//! [`probe`] reads the movie's video track; [`frames`] decodes it from a start time and hands each
//! chosen picture to a callback as premultiplied (opaque) RGBA8 with its time in seconds, in
//! presentation order. The movie is hostile input: the picture size is checked against
//! [`MAX_SIDE`] before any decoding, the number of samples decoded is capped at
//! [`MAX_DECODED_SAMPLES`], and the callback can stop the decoding at any time.

use craft_media::ByteSource;
use craft_media::isobmff::{CodecConfig, Mp4File, TrackKind};
use tooncraft_raster::{MAX_SIDE, RgbaImage};

use crate::{InteropError, Result};

/// Most samples one import decodes (frames before the start included). An hour at 60 fps.
pub const MAX_DECODED_SAMPLES: usize = 216_000;

/// The video track of a movie.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// Frames per second, from the samples' durations (0 when the track has one sample).
    pub fps: f64,
    /// Pictures in the track.
    pub frames: usize,
    /// Length in seconds.
    pub seconds: f64,
    /// `h264` or `hevc`.
    pub codec: &'static str,
}

/// Which pictures [`frames`] hands out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    /// Seconds into the movie of the first picture.
    pub from: f64,
    /// Every `every`-th picture from there (1 = all of them).
    pub every: u32,
    /// At most this many pictures.
    pub max: usize,
}

impl Default for Pick {
    fn default() -> Self {
        Pick { from: 0.0, every: 1, max: usize::MAX }
    }
}

impl Pick {
    /// Finite, non-negative start; `every` at least 1.
    pub fn sanitized(self) -> Self {
        Pick { from: if self.from.is_finite() { self.from.max(0.0) } else { 0.0 }, every: self.every.max(1), max: self.max }
    }
}

enum Decoder {
    H264(Box<craft_video::h264::decode::Decoder>),
    Hevc(Box<craft_video::hevc::decode::Decoder>),
}

/// One decoded picture before conversion: planes, strides and the colour matrix.
struct Planes<'a> {
    w: u32,
    h: u32,
    y: &'a [u8],
    u: &'a [u8],
    v: &'a [u8],
    y_stride: usize,
    uv_stride: usize,
    /// Chroma subsampling shifts (x, y).
    shift: (u32, u32),
    full_range: bool,
    matrix: u8,
    pts: i64,
}

fn bad(e: impl std::fmt::Display) -> InteropError {
    InteropError::Decode(e.to_string())
}

fn video_track(file: &Mp4File) -> Result<usize> {
    file.track_of_kind(TrackKind::Video).ok_or_else(|| InteropError::Decode("the movie has no video track".into()))
}

fn codec_name(file: &Mp4File, track: usize) -> Result<&'static str> {
    match file.tracks.get(track).and_then(|t| t.codec()) {
        Some(CodecConfig::Avc(_)) => Ok("h264"),
        Some(CodecConfig::Hevc(_)) => Ok("hevc"),
        Some(other) => Err(InteropError::Decode(format!("the {} video codec is not supported (H.264 and H.265 are)", other.name()))),
        None => Err(InteropError::Decode("the video track names no codec".into())),
    }
}

/// The movie's video track: size, rate, length and codec.
pub fn probe<S: ByteSource>(src: S) -> Result<VideoInfo> {
    let file = craft_media::isobmff::open(src).map_err(bad)?;
    info(&file)
}

fn info(file: &Mp4File) -> Result<VideoInfo> {
    let index = video_track(file)?;
    let codec = codec_name(file, index)?;
    let t = file.tracks.get(index).ok_or_else(|| bad("no video track"))?;
    let (width, height) = match t.video() {
        Some(v) if v.width > 0 && v.height > 0 => (v.width as u32, v.height as u32),
        _ => (t.width, t.height),
    };
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(InteropError::Limit(format!("a {width}×{height} video (at most {MAX_SIDE} a side)")));
    }
    let timescale = t.timescale.max(1) as f64;
    let total: u64 = t.samples.iter().map(|s| s.duration as u64).sum();
    let seconds = total as f64 / timescale;
    let fps = if t.samples.len() > 1 && total > 0 { t.samples.len() as f64 / seconds } else { 0.0 };
    Ok(VideoInfo { width, height, fps, frames: t.samples.len(), seconds, codec })
}

/// Decode the movie's pictures chosen by `pick`, in presentation order, handing each to `each`
/// with its time in seconds from the start of the movie; `each` returns `false` to stop.
/// Returns the track's [`VideoInfo`].
pub fn frames<S: ByteSource>(src: S, pick: Pick, mut each: impl FnMut(RgbaImage, f64) -> Result<bool>) -> Result<VideoInfo> {
    let pick = pick.sanitized();
    let file = craft_media::isobmff::open(&src).map_err(bad)?;
    let vi = info(&file)?;
    let index = video_track(&file)?;
    let t = file.tracks.get(index).ok_or_else(|| bad("no video track"))?;
    let mut dec = match t.codec() {
        Some(CodecConfig::Avc(c)) => Decoder::H264(Box::new(craft_video::h264::decode::Decoder::from_avcc(&c.to_bytes()).map_err(bad)?)),
        Some(CodecConfig::Hevc(c)) => Decoder::Hevc(Box::new(craft_video::hevc::decode::Decoder::from_hvcc(&c.to_bytes()).map_err(bad)?)),
        _ => return Err(bad("unsupported codec")),
    };
    let timescale = t.timescale.max(1) as f64;
    let first = t.samples.iter().map(|s| s.pts).min().unwrap_or(0);
    let from_ticks = first.saturating_add((pick.from * timescale).round().min(i64::MAX as f64) as i64);
    // start decoding at the key picture before the start; earlier pictures are decoded but dropped
    let start = file.sample_at_pts(index, from_ticks).and_then(|s| file.sync_sample_before(index, s)).unwrap_or(0);
    let mut seen = 0u64;
    let mut given = 0usize;
    let mut decoded = 0usize;
    let mut done = false;
    let mut handle = |p: Planes<'_>, seen: &mut u64, given: &mut usize| -> Result<bool> {
        if p.pts < from_ticks {
            return Ok(true);
        }
        let take = seen.is_multiple_of(pick.every as u64);
        *seen += 1;
        if !take {
            return Ok(true);
        }
        *given += 1;
        let img = to_rgba(&p)?;
        let keep_going = each(img, (p.pts - first) as f64 / timescale)?;
        Ok(keep_going && *given < pick.max)
    };
    for i in start..t.samples.len() {
        if done {
            break;
        }
        decoded += 1;
        if decoded > MAX_DECODED_SAMPLES {
            return Err(InteropError::Limit(format!("more than {MAX_DECODED_SAMPLES} video frames")));
        }
        let data = file.read_sample(&src, index, i).map_err(bad)?;
        let pts = t.samples.get(i).map(|s| s.pts).unwrap_or(0);
        done = !emit(&mut dec, Some((&data, pts)), &mut |p| handle(p, &mut seen, &mut given))?;
    }
    if !done {
        emit(&mut dec, None, &mut |p| handle(p, &mut seen, &mut given))?;
    }
    Ok(vi)
}

/// Feed one access unit (or flush with `None`) and pass every picture that comes out to `f`,
/// stopping when `f` returns `false`. Returns whether to go on.
fn emit(dec: &mut Decoder, unit: Option<(&[u8], i64)>, f: &mut dyn FnMut(Planes<'_>) -> Result<bool>) -> Result<bool> {
    match dec {
        Decoder::H264(d) => {
            let pics = match unit {
                Some((data, pts)) => d.decode(data, pts).map_err(bad)?,
                None => d.flush(),
            };
            for p in &pics {
                let planes = Planes {
                    w: p.width,
                    h: p.height,
                    y: &p.y,
                    u: &p.u,
                    v: &p.v,
                    y_stride: p.y_stride,
                    uv_stride: p.uv_stride,
                    shift: (1, 1),
                    full_range: p.color.full_range,
                    matrix: p.color.matrix,
                    pts: p.pts,
                };
                if !f(planes)? {
                    return Ok(false);
                }
            }
        }
        Decoder::Hevc(d) => {
            let pics = match unit {
                Some((data, pts)) => d.decode(data, pts).map_err(bad)?,
                None => d.flush(),
            };
            for p in &pics {
                let (Some(y), Some(u), Some(v)) = (p.y.as_u8(), p.u.as_u8(), p.v.as_u8()) else {
                    return Err(InteropError::Decode(format!("{}-bit H.265 video is not supported (8-bit is)", p.bit_depth)));
                };
                let planes = Planes {
                    w: p.width,
                    h: p.height,
                    y,
                    u,
                    v,
                    y_stride: p.y_stride,
                    uv_stride: p.uv_stride,
                    shift: p.chroma.shifts(),
                    full_range: p.color.full_range,
                    matrix: p.color.matrix,
                    pts: p.pts,
                };
                if !f(planes)? {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

/// Planar Y'CbCr → opaque RGBA8 (ITU-R BT.709 for matrix 1 and for unspecified HD sizes, BT.601
/// otherwise; limited or full range as flagged).
fn to_rgba(p: &Planes<'_>) -> Result<RgbaImage> {
    let (w, h) = (p.w, p.h);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(InteropError::Limit(format!("a {w}×{h} video frame")));
    }
    let (sx, sy) = (p.shift.0.min(1), p.shift.1.min(1));
    let cw = (w as usize).div_ceil(1 << sx);
    let ch = (h as usize).div_ceil(1 << sy);
    let need = |len: usize, stride: usize, rows: usize, cols: usize| rows == 0 || (stride >= cols && len >= (rows - 1) * stride + cols);
    if !need(p.y.len(), p.y_stride, h as usize, w as usize) || !need(p.u.len(), p.uv_stride, ch, cw) || !need(p.v.len(), p.uv_stride, ch, cw) {
        return Err(InteropError::Decode("a video frame's planes are too short".into()));
    }
    let bt709 = match p.matrix {
        1 => true,
        5 | 6 => false,
        _ => w >= 1280 || h >= 720,
    };
    let (kr, kb) = if bt709 { (0.2126f32, 0.0722f32) } else { (0.299f32, 0.114f32) };
    let kg = 1.0 - kr - kb;
    let (y_off, y_scale, c_scale) = if p.full_range { (0.0f32, 1.0 / 255.0, 1.0 / 255.0) } else { (16.0f32, 1.0 / 219.0, 1.0 / 224.0) };
    let mut data = vec![0u8; w as usize * h as usize * 4];
    for yy in 0..h as usize {
        let yrow = &p.y[yy * p.y_stride..];
        let crow = (yy >> sy) * p.uv_stride;
        for x in 0..w as usize {
            let l = (yrow[x] as f32 - y_off) * y_scale;
            let ci = crow + (x >> sx);
            let cb = (p.u[ci] as f32 - 128.0) * c_scale;
            let cr = (p.v[ci] as f32 - 128.0) * c_scale;
            let r = l + 2.0 * (1.0 - kr) * cr;
            let b = l + 2.0 * (1.0 - kb) * cb;
            let g = (l - kr * r - kb * b) / kg;
            let o = (yy * w as usize + x) * 4;
            data[o] = (r * 255.0).round().clamp(0.0, 255.0) as u8;
            data[o + 1] = (g * 255.0).round().clamp(0.0, 255.0) as u8;
            data[o + 2] = (b * 255.0).round().clamp(0.0, 255.0) as u8;
            data[o + 3] = 255;
        }
    }
    Ok(RgbaImage { width: w, height: h, data })
}

/// Test movies: an H.264 MP4 written with craft-video's encoder and craft-media's muxer.
#[cfg(test)]
pub(crate) mod sample {
    use craft_media::isobmff::{AvcConfig, Brand, Mp4Writer, SampleEntry, TrackConfig, WriteSample, WriterOptions};
    use craft_video::h264::{Encoder, EncoderConfig, PacketFormat, RateControl, YuvFrame};

    /// `frames` pictures of `w`×`h` at `fps`: picture `i` is mid grey with a white square whose
    /// left edge is at `i * 4`, so each picture can be told apart.
    pub fn movie(w: u32, h: u32, fps: u32, frames: u32) -> Vec<u8> {
        let mut cfg = EncoderConfig::new(w, h, fps, 1);
        cfg.format = PacketFormat::LengthPrefixed;
        cfg.bframes = 0;
        cfg.keyint = 4;
        cfg.rate = RateControl::Crf(10.0);
        let mut enc = Encoder::new(cfg).unwrap();
        let mut out = std::io::Cursor::new(Vec::new());
        let entry = |enc: &Encoder| SampleEntry::avc(AvcConfig::parse(&enc.avcc()).unwrap(), w as u16, h as u16);
        let mut mux = Mp4Writer::new(&mut out, WriterOptions::new(Brand::Mp4)).unwrap();
        let track = mux.add_track(TrackConfig::new(entry(&enc), fps)).unwrap();
        let mut packets = Vec::new();
        for i in 0..frames {
            let mut y = vec![126u8; (w * h) as usize];
            for yy in 4..12 {
                for x in (i * 4)..(i * 4 + 8).min(w) {
                    y[(yy * w + x) as usize] = 235;
                }
            }
            let c = vec![128u8; (w / 2 * h / 2) as usize];
            let f = YuvFrame { y: &y, u: &c, v: &c, y_stride: w as usize, uv_stride: w as usize / 2 };
            packets.extend(enc.encode(&f, i as i64).unwrap());
        }
        packets.extend(enc.flush());
        for pk in &packets {
            let s = WriteSample { data: &pk.data, duration: 1, composition_offset: (pk.pts - pk.dts) as i32, is_sync: pk.keyframe };
            mux.write_sample(track, s).unwrap();
        }
        mux.set_sample_entry(track, entry(&enc)).unwrap();
        mux.finish().unwrap();
        out.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Left edge of the white square in a decoded picture's row 8.
    fn square_at(img: &RgbaImage) -> Option<u32> {
        (0..img.width).find(|x| img.pixel(*x, 8)[0] > 200)
    }

    #[test]
    fn probe_reads_the_track() {
        let m = sample::movie(64, 32, 12, 6);
        let i = probe(m.as_slice()).unwrap();
        assert_eq!((i.width, i.height, i.frames, i.codec), (64, 32, 6, "h264"));
        assert!((i.fps - 12.0).abs() < 1e-6, "{}", i.fps);
        assert!((i.seconds - 0.5).abs() < 1e-6);
    }

    #[test]
    fn every_picture_in_order_with_its_time() {
        let m = sample::movie(64, 32, 12, 6);
        let mut got = Vec::new();
        frames(m.as_slice(), Pick::default(), |img, t| {
            assert_eq!((img.width, img.height), (64, 32));
            // grey background: limited-range 126 is about 50 % grey
            let bg = img.pixel(40, 28);
            assert!((120..=135).contains(&bg[0]) && bg[3] == 255, "{bg:?}");
            got.push((square_at(&img), t));
            Ok(true)
        })
        .unwrap();
        assert_eq!(got.len(), 6);
        for (i, (x, t)) in got.iter().enumerate() {
            let x = x.unwrap();
            assert!(x.abs_diff(i as u32 * 4) <= 1, "picture {i} square at {x}");
            assert!((t - i as f64 / 12.0).abs() < 1e-9, "picture {i} at {t}");
        }
    }

    #[test]
    fn a_start_a_step_and_a_cap() {
        let m = sample::movie(64, 32, 12, 10);
        let mut times = Vec::new();
        // from 0.25 s (picture 3, after the key picture at 0 is decoded and dropped), every 2nd, 3 at most
        frames(m.as_slice(), Pick { from: 0.25, every: 2, max: 3 }, |img, t| {
            times.push((square_at(&img).unwrap(), (t * 12.0).round() as u32));
            Ok(true)
        })
        .unwrap();
        let idx: Vec<u32> = times.iter().map(|t| t.1).collect();
        assert_eq!(idx, vec![3, 5, 7]);
        for (x, i) in times {
            assert!(x.abs_diff(i * 4) <= 1);
        }
    }

    #[test]
    fn the_callback_can_stop() {
        let m = sample::movie(64, 32, 12, 8);
        let mut n = 0;
        frames(m.as_slice(), Pick::default(), |_, _| {
            n += 1;
            Ok(n < 2)
        })
        .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn not_a_movie_is_an_error() {
        assert!(probe(b"not a movie at all".as_slice()).is_err());
        assert!(frames(Vec::<u8>::new(), Pick::default(), |_, _| Ok(true)).is_err());
    }

    #[test]
    fn hostile_picks_are_sanitized() {
        let p = Pick { from: f64::NAN, every: 0, max: 0 }.sanitized();
        assert_eq!((p.from, p.every), (0.0, 1));
        let p = Pick { from: -5.0, every: 3, max: 1 }.sanitized();
        assert_eq!(p.from, 0.0);
        let m = sample::movie(32, 16, 12, 3);
        let mut n = 0;
        frames(m.as_slice(), Pick { from: f64::INFINITY, every: 0, max: 1 }, |_, _| {
            n += 1;
            Ok(true)
        })
        .unwrap();
        assert_eq!(n, 1, "an infinite start is the beginning, and max 1 gives one picture");
        // a start past the end gives nothing, without an error
        let mut n = 0;
        frames(m.as_slice(), Pick { from: 1e300, ..Pick::default() }, |_, _| {
            n += 1;
            Ok(true)
        })
        .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn yuv_conversion_ranges_and_matrices() {
        let px = |y: u8, u: u8, v: u8, full: bool, matrix: u8| {
            let (yy, uu, vv) = ([y; 4], [u; 1], [v; 1]);
            let p = Planes { w: 2, h: 2, y: &yy, u: &uu, v: &vv, y_stride: 2, uv_stride: 1, shift: (1, 1), full_range: full, matrix, pts: 0 };
            to_rgba(&p).unwrap().pixel(1, 1)
        };
        assert_eq!(px(16, 128, 128, false, 1), [0, 0, 0, 255]);
        assert_eq!(px(235, 128, 128, false, 1), [255, 255, 255, 255]);
        assert_eq!(px(255, 128, 128, true, 6), [255, 255, 255, 255]);
        let red = px(81, 90, 240, false, 6);
        assert!(red[0] > 240 && red[1] < 15 && red[2] < 15, "{red:?}");
        // short planes are an error, not a panic
        let p = Planes { w: 4, h: 4, y: &[0; 3], u: &[0; 4], v: &[0; 4], y_stride: 4, uv_stride: 2, shift: (1, 1), full_range: false, matrix: 1, pts: 0 };
        assert!(to_rgba(&p).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        /// Damaged movies give an error or fewer pictures, never a panic.
        #[test]
        fn damaged_movies_never_panic(cut in 0usize..4000, flips in proptest::collection::vec((0usize..4000, any::<u8>()), 0..8)) {
            let mut m = sample::movie(32, 16, 12, 4);
            for (at, b) in flips {
                let n = m.len();
                m[at % n] ^= b;
            }
            m.truncate(cut.min(m.len()));
            let _ = probe(m.as_slice());
            let mut n = 0;
            let _ = frames(m.as_slice(), Pick::default(), |_, _| { n += 1; Ok(n < 16) });
            prop_assert!(n <= 16);
        }
    }
}
