//! Animated GIF import (GIF89a, a public specification), through craft-gif's bounded frame
//! decoder.
//!
//! [`import`] returns every frame composited on the logical screen as premultiplied RGBA8, with
//! its delay in hundredths of a second as stored (0 when the file gives none; players commonly
//! show such frames for 10 cs, which is left to the caller). Decoding stops at
//! [`MAX_FRAMES`] frames or [`MAX_BYTES`] of pictures. A damaged frame after the first ends the
//! animation there; a file whose first frame does not decode is an error.

use craft_gif::frames::{FrameLimits, read_frames};
use tooncraft_raster::{MAX_SIDE, RgbaImage};

use crate::common::premultiply_in_place;
use crate::{InteropError, Result};

/// Most frames kept.
pub const MAX_FRAMES: usize = tooncraft_doc::MAX_FRAMES as usize;
/// Most bytes of decoded pictures kept.
pub const MAX_BYTES: u64 = 1 << 30;

/// Decode an animated (or still) GIF into frames and their delays in centiseconds.
pub fn import(bytes: &[u8]) -> Result<(Vec<RgbaImage>, Vec<u16>)> {
    let mut limits = FrameLimits::default();
    limits.max_frames = MAX_FRAMES;
    limits.max_bytes = MAX_BYTES;
    limits.max_canvas_bytes = limits.max_canvas_bytes.min(MAX_BYTES);
    let anim = read_frames(bytes, &limits).map_err(|e| match e {
        craft_gif::frames::FrameError::LimitExceeded(m) => InteropError::Limit(m),
        craft_gif::frames::FrameError::Malformed(m) => InteropError::Decode(m),
    })?;
    let (w, h) = (anim.width, anim.height);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(InteropError::Limit(format!("a {w}×{h} animation (at most {MAX_SIDE} a side)")));
    }
    let mut images = Vec::with_capacity(anim.frames.len());
    let mut delays = Vec::with_capacity(anim.frames.len());
    for mut f in anim.frames {
        if f.rgba.len() != w as usize * h as usize * 4 {
            return Err(InteropError::Decode("a GIF frame has the wrong size".into()));
        }
        premultiply_in_place(&mut f.rgba);
        images.push(RgbaImage { width: w, height: h, data: f.rgba });
        delays.push((f.delay_ms / 10).min(u16::MAX as u32) as u16);
    }
    if images.is_empty() {
        return Err(InteropError::Decode("the GIF has no frames".into()));
    }
    Ok((images, delays))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn sample() -> Vec<u8> {
        let mut w = craft_gif::write::AnimationWriter::new(Vec::new(), 4, 3, craft_gif::write::Options::default()).unwrap();
        let red = [255u8, 0, 0, 255].repeat(12);
        let mut blue = [0u8, 0, 255, 255].repeat(12);
        blue[0..4].copy_from_slice(&[0, 0, 0, 0]);
        w.write_frame(craft_gif::write::RgbaFrame { pixels: &red, delay_cs: 8 }).unwrap();
        w.write_frame(craft_gif::write::RgbaFrame { pixels: &blue, delay_cs: 9 }).unwrap();
        w.finish().unwrap()
    }

    #[test]
    fn frames_and_delays() {
        let (frames, delays) = import(&sample()).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(delays, [8, 9]);
        assert_eq!((frames[0].width, frames[0].height), (4, 3));
        assert_eq!(frames[0].pixel(1, 1), [255, 0, 0, 255]);
        assert_eq!(frames[1].pixel(1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn garbage_fails() {
        assert!(import(b"").is_err());
        assert!(import(b"GIF89a").is_err());
        assert!(import(b"not a gif at all").is_err());
        let bytes = sample();
        assert!(import(&bytes[..13]).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]
        #[test]
        fn corrupt_bytes_never_panic(at in 0usize..512, val in any::<u8>(), cut in 0usize..512) {
            let mut bytes = sample();
            let i = at % bytes.len();
            bytes[i] = val;
            bytes.truncate(cut.min(bytes.len()));
            let _ = import(&bytes);
        }
    }
}
