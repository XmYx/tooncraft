//! Layer effects for ToonCraft (UI-free, no other ToonCraft dependency than the raster types).
//!
//! An [`Effect`] is one entry of a layer's effect stack: a kind with its parameters
//! ([`EffectKind`]), an on/off switch and optional keys. With keys, the parameters at a frame are
//! interpolated linearly between the surrounding keys (held before the first and after the last);
//! without keys the effect's own parameters apply to every frame.
//!
//! [`apply_stack`] runs a stack over a premultiplied RGBA8 image, top of the list first. Every
//! effect is a CPU routine over flat rows (sliding-window box blurs, a linear-time distance
//! transform, per-pixel lookup tables), split over the available cores; the results are
//! deterministic, so exports and playback show the same pixels.
//!
//! Effect parameters come from files and command lines, so they are hostile: [`Effect::sanitized`]
//! clamps every number to a documented range and drops keys of another kind.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod blur;
mod colour;
mod distance;
mod light;
mod look;
mod par;
mod particles;
mod shape;
mod time;

use serde::{Deserialize, Serialize};
use tooncraft_raster::{BlendMode, RgbaImage};

pub use blur::gaussian;
pub use look::MAX_GHOSTS;
pub use par::set_max_threads;
pub use shape::resample_closed;

/// Most effects on one layer.
pub const MAX_EFFECTS: usize = 32;
/// Most keys on one effect.
pub const MAX_KEYS: usize = 4096;
/// Most points on a curve.
pub const MAX_CURVE_POINTS: usize = 32;
/// Largest blur, glow and light-wrap radius, pixels.
pub const MAX_RADIUS: f32 = 200.0;

/// The shape of a feathered mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskShape {
    #[default]
    Rectangle,
    Ellipse,
    /// A closed polygon drawn on the canvas (the mask's `points`).
    Drawn,
}

/// How a transition brings a layer in (or takes it out).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionStyle {
    #[default]
    Dissolve,
    /// An edge sweeping across at the transition's angle.
    Wipe,
    /// A circle opening from the centre.
    Iris,
    /// Sliding in along the angle.
    Push,
}

/// What particles look like and how they move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticleStyle {
    /// Streaks falling along their direction.
    #[default]
    Rain,
    /// Soft flakes drifting from side to side.
    Snow,
    /// Bright streaks fading out over their life.
    Sparks,
    /// Faint twinkling motes.
    Dust,
}

pub use particles::MAX_LIVE as MAX_PARTICLES;

/// Most samples of a motion blur.
pub const MAX_BLUR_SAMPLES: u32 = 16;
/// Most copies a trail leaves.
pub const MAX_TRAILS: u32 = 16;

/// Most points of a drawn mask shape.
pub const MAX_MASK_POINTS: usize = 1024;
/// Points a drawn mask shape is resampled to when it is set from a selection, so every key of a
/// mask has as many points and blends point by point.
pub const MASK_SHAPE_POINTS: usize = 96;

/// An effect and its parameters. Distances are canvas pixels, colours straight RGB, amounts 0…1
/// unless noted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EffectKind {
    /// Soft blur (Gaussian, `radius` is its standard deviation).
    Blur { radius: f32 },
    /// Blur along one direction (motion streaks): `length` pixels at `angle` degrees.
    DirectionalBlur { length: f32, angle: f32 },
    /// Zoom blur towards a centre given as a fraction of the canvas (0.5, 0.5 is the middle).
    RadialBlur { amount: f32, centre_x: f32, centre_y: f32 },
    /// A blurred, tinted copy added on top: light from the drawing. `intensity` 0…5.
    Glow { radius: f32, intensity: f32, colour: [u8; 3] },
    /// A blurred, offset silhouette under the drawing.
    DropShadow { dx: f32, dy: f32, blur: f32, colour: [u8; 3], opacity: f32 },
    /// A solid outline `width` pixels around the drawing, under it.
    Outline { width: f32, colour: [u8; 3] },
    /// Grow (positive) or thin (negative) lines by `amount` pixels, keeping their colours.
    Thicken { amount: f32 },
    /// Input black / white points, gamma (0.1…10) and output black / white points.
    Levels { in_black: f32, in_white: f32, gamma: f32, out_black: f32, out_white: f32 },
    /// A smooth tone curve through `points` (x, y in 0…1), applied to red, green and blue.
    Curves { points: Vec<[f32; 2]> },
    /// Hue shift in degrees, saturation and lightness changes in −1…1.
    HueSaturation { hue: f32, saturation: f32, lightness: f32 },
    /// Mix the colours towards `colour` by `amount`.
    Tint { colour: [u8; 3], amount: f32 },
    /// Film grain on the drawn pixels: `amount`, grain `size` 1…16 px, new grain every frame
    /// when `animated`.
    Grain { amount: f32, size: f32, seed: u32, animated: bool },
    /// A surface texture (one of the brush grains, fixed to the canvas) thinning the paint in its
    /// valleys: paper, canvas weave, speckle or clouds over everything on the layer.
    Texture { kind: tooncraft_brush::GrainKind, scale: f32, angle: f32, contrast: f32, amount: f32 },
    /// Let the light of the layers below bleed into the drawing's edges.
    LightWrap { radius: f32, intensity: f32 },
    /// Hard edges: alpha at or above `threshold` becomes opaque, the rest transparent.
    AlphaThreshold { threshold: f32 },
    /// Keep only what is inside a rectangle or ellipse centred at `x`, `y` (canvas pixels), or
    /// inside the drawn polygon `points` (canvas pixels), with a soft edge `feather` pixels
    /// wide; `invert` keeps the outside.
    Mask {
        shape: MaskShape,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        feather: f32,
        invert: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        points: Vec<[f32; 2]>,
    },
    /// Make pixels close to `colour` transparent (scanned paper, coloured backdrops): within
    /// `tolerance` (0…1 colour distance) they go, within `tolerance + softness` they fade.
    ColourKey { colour: [u8; 3], tolerance: f32, softness: f32 },
    /// Map brightness to a colour between `dark` and `light`, `amount` of the way (0…1).
    GradientMap { dark: [u8; 3], light: [u8; 3], amount: f32 },
    /// `levels` steps per colour channel (2…64).
    Posterize { levels: u32 },
    /// Black or white by brightness against `level` (0…1).
    Threshold { level: f32 },
    /// Darken the drawing towards the edges: none within `radius` of `x`, `y` (canvas pixels),
    /// `amount` (0…1) beyond `radius + softness`.
    Vignette { x: f32, y: f32, radius: f32, softness: f32, amount: f32 },
    /// Red and blue pulled `amount` pixels apart (0…50), like a lens's colour fringe.
    ChromaticAberration { amount: f32 },
    /// Rings from `x`, `y`: `amplitude` pixels (0…100) every `wavelength` pixels (2…2000),
    /// moving `speed` waves per frame.
    Ripple { x: f32, y: f32, amplitude: f32, wavelength: f32, speed: f32 },
    /// A wave across the picture at `angle` degrees.
    Wave { amplitude: f32, wavelength: f32, angle: f32, speed: f32 },
    /// Smooth noise displacement: `amount` pixels (0…100), features `scale` pixels (2…1000),
    /// changing every frame when `animated`.
    Turbulence { amount: f32, scale: f32, seed: u32, animated: bool },
    /// Smooth jagged lines (`radius` 0…20 pixels).
    Smooth { radius: f32 },
    /// Light from a bright point at `x`, `y` (canvas pixels): a core `size` pixels wide with a
    /// ring, `rays` star rays (0, or 2…32) and `ghosts` discs (0…16) on the line from the light
    /// through `through_x`, `through_y` (usually the middle of the picture). `intensity` 0…4.
    LensFlare { x: f32, y: f32, through_x: f32, through_y: f32, size: f32, intensity: f32, colour: [u8; 3], rays: u32, ghosts: u32 },
    /// Move the picture by another layer's pixels (`layer`, its id): red moves across by up to
    /// `amount_x` pixels, green down by up to `amount_y` (mid grey: no move), or with
    /// `brightness` its brightness moves both. Where that layer is transparent nothing moves.
    Displace { layer: u64, amount_x: f32, amount_y: f32, brightness: bool },
    /// Blur from the layer's own movement (its peg and the camera): the drawing is drawn at
    /// `samples` (2…16) points of a shutter open for `shutter` frames (0…2) around the frame
    /// and averaged.
    MotionBlur { shutter: f32, samples: u32 },
    /// Copies of the layer's last `count` (1…16) pictures, `step` frames apart (1…24), each
    /// `fade` (0…1) as strong as the one after it, under the layer.
    Trails { count: u32, step: u32, fade: f32 },
    /// Mix `amount` (0…1) of the next frame's picture into this one (smooths a change).
    FrameBlend { amount: f32 },
    /// Show the layer's frame `offset + speed × frame` (rounded, held at the shot's ends):
    /// slow motion, fast motion, reversed or held. Key `speed` and `offset` to ramp.
    TimeRemap { speed: f32, offset: f32 },
    /// Bring the layer in (or, with `out`, take it out) over `length` frames from `start`:
    /// dissolve, wipe or push along `angle` degrees across `size` pixels, or an iris opening
    /// to radius `size` from `x`, `y`.
    Transition { style: TransitionStyle, start: u32, length: u32, out: bool, angle: f32, x: f32, y: f32, size: f32 },
    /// Particles from the rectangle `x`, `y`, `width`, `height` (canvas pixels): `rate` new ones
    /// a frame (0…500), each living `life` frames (1…600), moving `speed` pixels a frame at
    /// `angle` degrees ± `spread` / 2, pulled down by `gravity` pixels a frame², `size` pixels
    /// wide (0.5…64), in `colour` at `opacity`. Rendered from the frame alone (no simulation
    /// state), so every frame is the same each time.
    Particles {
        style: ParticleStyle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rate: f32,
        life: u32,
        speed: f32,
        angle: f32,
        spread: f32,
        gravity: f32,
        size: f32,
        colour: [u8; 3],
        opacity: f32,
        seed: u32,
    },
    /// Tone (multiply `colour`) or, with `highlight`, light (screen `colour`) the drawing where
    /// another layer (`layer`, its id; a painted shadow or light shape) covers it, moved by
    /// `offset_x`, `offset_y` pixels and softened by `softness` pixels, `amount` of the way.
    MatteTone { layer: u64, colour: [u8; 3], amount: f32, offset_x: f32, offset_y: f32, softness: f32, highlight: bool },
    /// Light the drawing from its normals: from the normal-map layer `normals` (its id), or from
    /// the drawing's own shape rounded over `depth` pixels. The light comes from `angle` degrees
    /// across the picture and `elevation` degrees above it, `intensity` 0…2, tinted `colour`.
    NormalLight {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        normals: Option<u64>,
        angle: f32,
        elevation: f32,
        depth: f32,
        intensity: f32,
        colour: [u8; 3],
    },
}

impl EffectKind {
    /// Every kind with useful starting parameters for a canvas of `w`×`h` pixels.
    pub fn catalogue(w: u32, h: u32) -> Vec<EffectKind> {
        let (w, h) = (w as f32, h as f32);
        vec![
            EffectKind::Blur { radius: 4.0 },
            EffectKind::DirectionalBlur { length: 24.0, angle: 0.0 },
            EffectKind::RadialBlur { amount: 0.15, centre_x: 0.5, centre_y: 0.5 },
            EffectKind::Glow { radius: 12.0, intensity: 1.0, colour: [255, 240, 200] },
            EffectKind::DropShadow { dx: 8.0, dy: 8.0, blur: 6.0, colour: [0, 0, 0], opacity: 0.5 },
            EffectKind::Outline { width: 3.0, colour: [255, 255, 255] },
            EffectKind::Thicken { amount: 1.0 },
            EffectKind::Levels { in_black: 0.0, in_white: 1.0, gamma: 1.0, out_black: 0.0, out_white: 1.0 },
            EffectKind::Curves { points: vec![[0.0, 0.0], [0.25, 0.2], [0.75, 0.8], [1.0, 1.0]] },
            EffectKind::HueSaturation { hue: 0.0, saturation: 0.0, lightness: 0.0 },
            EffectKind::Tint { colour: [40, 60, 140], amount: 0.3 },
            EffectKind::Grain { amount: 0.15, size: 1.0, seed: 1, animated: true },
            EffectKind::Texture { kind: tooncraft_brush::GrainKind::Paper, scale: 3.0, angle: 0.0, contrast: 1.0, amount: 0.6 },
            EffectKind::LightWrap { radius: 10.0, intensity: 0.6 },
            EffectKind::AlphaThreshold { threshold: 0.5 },
            EffectKind::Mask {
                shape: MaskShape::Ellipse,
                x: w / 2.0,
                y: h / 2.0,
                width: w * 0.6,
                height: h * 0.6,
                feather: 40.0,
                invert: false,
                points: Vec::new(),
            },
            EffectKind::ColourKey { colour: [255, 255, 255], tolerance: 0.1, softness: 0.1 },
            EffectKind::GradientMap { dark: [30, 20, 60], light: [255, 220, 160], amount: 1.0 },
            EffectKind::Posterize { levels: 4 },
            EffectKind::Threshold { level: 0.5 },
            EffectKind::Vignette { x: w / 2.0, y: h / 2.0, radius: w.min(h) * 0.35, softness: w.min(h) * 0.4, amount: 0.7 },
            EffectKind::ChromaticAberration { amount: 2.0 },
            EffectKind::Ripple { x: w / 2.0, y: h / 2.0, amplitude: 4.0, wavelength: 40.0, speed: 0.1 },
            EffectKind::Wave { amplitude: 4.0, wavelength: 60.0, angle: 0.0, speed: 0.05 },
            EffectKind::Turbulence { amount: 3.0, scale: 40.0, seed: 1, animated: true },
            EffectKind::Smooth { radius: 1.5 },
            EffectKind::LensFlare {
                x: w * 0.25,
                y: h * 0.25,
                through_x: w / 2.0,
                through_y: h / 2.0,
                size: w.min(h) * 0.06,
                intensity: 1.0,
                colour: [255, 230, 190],
                rays: 6,
                ghosts: 4,
            },
            EffectKind::Displace { layer: 0, amount_x: 8.0, amount_y: 8.0, brightness: false },
            EffectKind::MotionBlur { shutter: 0.5, samples: 8 },
            EffectKind::Trails { count: 4, step: 1, fade: 0.6 },
            EffectKind::FrameBlend { amount: 0.5 },
            EffectKind::TimeRemap { speed: 0.5, offset: 0.0 },
            EffectKind::Particles {
                style: ParticleStyle::Snow,
                x: -w * 0.1,
                y: -20.0,
                width: w * 1.2,
                height: 10.0,
                rate: 6.0,
                life: 120,
                speed: 2.0,
                angle: 90.0,
                spread: 30.0,
                gravity: 0.0,
                size: 5.0,
                colour: [255, 255, 255],
                opacity: 0.9,
                seed: 1,
            },
            EffectKind::MatteTone { layer: 0, colour: [120, 110, 160], amount: 0.6, offset_x: 0.0, offset_y: 0.0, softness: 2.0, highlight: false },
            EffectKind::NormalLight { normals: None, angle: 225.0, elevation: 45.0, depth: 10.0, intensity: 0.8, colour: [255, 245, 220] },
            EffectKind::Transition {
                style: TransitionStyle::Wipe,
                start: 0,
                length: 12,
                out: false,
                angle: 0.0,
                x: w / 2.0,
                y: h / 2.0,
                size: w.hypot(h) / 2.0,
            },
        ]
    }

    /// A stable machine name (`blur`, `drop_shadow` …), as in the JSON.
    pub fn id(&self) -> &'static str {
        match self {
            EffectKind::Blur { .. } => "blur",
            EffectKind::DirectionalBlur { .. } => "directional_blur",
            EffectKind::RadialBlur { .. } => "radial_blur",
            EffectKind::Glow { .. } => "glow",
            EffectKind::DropShadow { .. } => "drop_shadow",
            EffectKind::Outline { .. } => "outline",
            EffectKind::Thicken { .. } => "thicken",
            EffectKind::Levels { .. } => "levels",
            EffectKind::Curves { .. } => "curves",
            EffectKind::HueSaturation { .. } => "hue_saturation",
            EffectKind::Tint { .. } => "tint",
            EffectKind::Grain { .. } => "grain",
            EffectKind::Texture { .. } => "texture",
            EffectKind::LightWrap { .. } => "light_wrap",
            EffectKind::AlphaThreshold { .. } => "alpha_threshold",
            EffectKind::Mask { .. } => "mask",
            EffectKind::ColourKey { .. } => "colour_key",
            EffectKind::GradientMap { .. } => "gradient_map",
            EffectKind::Posterize { .. } => "posterize",
            EffectKind::Threshold { .. } => "threshold",
            EffectKind::Vignette { .. } => "vignette",
            EffectKind::ChromaticAberration { .. } => "chromatic_aberration",
            EffectKind::Ripple { .. } => "ripple",
            EffectKind::Wave { .. } => "wave",
            EffectKind::Turbulence { .. } => "turbulence",
            EffectKind::Smooth { .. } => "smooth",
            EffectKind::LensFlare { .. } => "lens_flare",
            EffectKind::Displace { .. } => "displace",
            EffectKind::MotionBlur { .. } => "motion_blur",
            EffectKind::Trails { .. } => "trails",
            EffectKind::FrameBlend { .. } => "frame_blend",
            EffectKind::TimeRemap { .. } => "time_remap",
            EffectKind::Transition { .. } => "transition",
            EffectKind::Particles { .. } => "particles",
            EffectKind::MatteTone { .. } => "matte_tone",
            EffectKind::NormalLight { .. } => "normal_light",
        }
    }

    /// Same kind (parameters may differ)?
    pub fn same_kind(&self, other: &EffectKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// Does this effect read the layers below it (light wrap)?
    pub fn needs_below(&self) -> bool {
        matches!(self, EffectKind::LightWrap { .. })
    }

    /// The layer (id) this effect reads, if it reads one.
    pub fn source_layer(&self) -> Option<u64> {
        match self {
            EffectKind::Displace { layer, .. } | EffectKind::MatteTone { layer, .. } => Some(*layer),
            EffectKind::NormalLight { normals, .. } => *normals,
            _ => None,
        }
    }

    /// Does the compositor work this effect out from the layer at other frames or other
    /// transforms (motion blur, trails, frame blending, time remap)? Its pixel pass does nothing.
    pub fn is_time(&self) -> bool {
        matches!(self, EffectKind::MotionBlur { .. } | EffectKind::Trails { .. } | EffectKind::FrameBlend { .. } | EffectKind::TimeRemap { .. })
    }

    /// Does this effect make pixels of its own, so it runs on a layer even where the layer has no
    /// drawing (a lens flare)?
    pub fn generates(&self) -> bool {
        matches!(self, EffectKind::LensFlare { .. } | EffectKind::Particles { .. })
    }

    /// Every number clamped to its range (NaN becomes the range's low end or a neutral value).
    pub fn sanitized(self) -> EffectKind {
        use EffectKind::*;
        match self {
            Blur { radius } => Blur { radius: c(radius, 0.0, MAX_RADIUS, 0.0) },
            DirectionalBlur { length, angle } => DirectionalBlur { length: c(length, 0.0, 500.0, 0.0), angle: c(angle, -3600.0, 3600.0, 0.0) },
            RadialBlur { amount, centre_x, centre_y } => {
                RadialBlur { amount: c(amount, 0.0, 1.0, 0.0), centre_x: c(centre_x, -1.0, 2.0, 0.5), centre_y: c(centre_y, -1.0, 2.0, 0.5) }
            }
            Glow { radius, intensity, colour } => Glow { radius: c(radius, 0.0, MAX_RADIUS, 0.0), intensity: c(intensity, 0.0, 5.0, 0.0), colour },
            DropShadow { dx, dy, blur, colour, opacity } => DropShadow {
                dx: c(dx, -4096.0, 4096.0, 0.0),
                dy: c(dy, -4096.0, 4096.0, 0.0),
                blur: c(blur, 0.0, MAX_RADIUS, 0.0),
                colour,
                opacity: c(opacity, 0.0, 1.0, 0.0),
            },
            Outline { width, colour } => Outline { width: c(width, 0.0, 100.0, 0.0), colour },
            Thicken { amount } => Thicken { amount: c(amount, -50.0, 50.0, 0.0) },
            Levels { in_black, in_white, gamma, out_black, out_white } => {
                let in_black = c(in_black, 0.0, 1.0, 0.0);
                let in_white = c(in_white, 0.0, 1.0, 1.0).max(in_black + 1.0 / 255.0);
                Levels { in_black, in_white, gamma: c(gamma, 0.1, 10.0, 1.0), out_black: c(out_black, 0.0, 1.0, 0.0), out_white: c(out_white, 0.0, 1.0, 1.0) }
            }
            Curves { points } => {
                let mut p: Vec<[f32; 2]> = points.into_iter().take(MAX_CURVE_POINTS).map(|[x, y]| [c(x, 0.0, 1.0, 0.0), c(y, 0.0, 1.0, 0.0)]).collect();
                p.sort_by(|a, b| a[0].total_cmp(&b[0]));
                p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-4);
                if p.len() < 2 {
                    p = vec![[0.0, 0.0], [1.0, 1.0]];
                }
                Curves { points: p }
            }
            HueSaturation { hue, saturation, lightness } => {
                HueSaturation { hue: c(hue, -180.0, 180.0, 0.0), saturation: c(saturation, -1.0, 1.0, 0.0), lightness: c(lightness, -1.0, 1.0, 0.0) }
            }
            Tint { colour, amount } => Tint { colour, amount: c(amount, 0.0, 1.0, 0.0) },
            Grain { amount, size, seed, animated } => Grain { amount: c(amount, 0.0, 1.0, 0.0), size: c(size, 1.0, 16.0, 1.0), seed, animated },
            Texture { kind, scale, angle, contrast, amount } => Texture {
                // an image grain needs the shot's tip table, which effects cannot see
                kind: if kind == tooncraft_brush::GrainKind::Image { tooncraft_brush::GrainKind::Paper } else { kind },
                scale: c(scale, 0.5, 64.0, 3.0),
                angle: if angle.is_finite() { (angle + 180.0).rem_euclid(360.0) - 180.0 } else { 0.0 },
                contrast: c(contrast, 0.1, tooncraft_brush::MAX_GRAIN_CONTRAST, 1.0),
                amount: c(amount, 0.0, 1.0, 0.0),
            },
            LightWrap { radius, intensity } => LightWrap { radius: c(radius, 0.0, MAX_RADIUS, 0.0), intensity: c(intensity, 0.0, 2.0, 0.0) },
            AlphaThreshold { threshold } => AlphaThreshold { threshold: c(threshold, 0.0, 1.0, 0.5) },
            Mask { shape, x, y, width, height, feather, invert, points } => Mask {
                shape,
                points: points
                    .into_iter()
                    .filter(|p| p[0].is_finite() && p[1].is_finite())
                    .take(MAX_MASK_POINTS)
                    .map(|p| p.map(|v| v.clamp(-65536.0, 65536.0)))
                    .collect(),
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                width: c(width, 0.0, 65536.0, 0.0),
                height: c(height, 0.0, 65536.0, 0.0),
                feather: c(feather, 0.0, 4096.0, 0.0),
                invert,
            },
            ColourKey { colour, tolerance, softness } => ColourKey { colour, tolerance: c(tolerance, 0.0, 1.0, 0.0), softness: c(softness, 0.0, 1.0, 0.0) },
            GradientMap { dark, light, amount } => GradientMap { dark, light, amount: c(amount, 0.0, 1.0, 0.0) },
            Posterize { levels } => Posterize { levels: levels.clamp(2, 64) },
            Threshold { level } => Threshold { level: c(level, 0.0, 1.0, 0.5) },
            Vignette { x, y, radius, softness, amount } => Vignette {
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                radius: c(radius, 0.0, 65536.0, 0.0),
                softness: c(softness, 1.0, 65536.0, 1.0),
                amount: c(amount, 0.0, 1.0, 0.0),
            },
            ChromaticAberration { amount } => ChromaticAberration { amount: c(amount, 0.0, 50.0, 0.0) },
            Ripple { x, y, amplitude, wavelength, speed } => Ripple {
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                amplitude: c(amplitude, 0.0, 100.0, 0.0),
                wavelength: c(wavelength, 2.0, 2000.0, 40.0),
                speed: c(speed, -10.0, 10.0, 0.0),
            },
            Wave { amplitude, wavelength, angle, speed } => Wave {
                amplitude: c(amplitude, 0.0, 100.0, 0.0),
                wavelength: c(wavelength, 2.0, 2000.0, 40.0),
                angle: c(angle, -3600.0, 3600.0, 0.0),
                speed: c(speed, -10.0, 10.0, 0.0),
            },
            Turbulence { amount, scale, seed, animated } => {
                Turbulence { amount: c(amount, 0.0, 100.0, 0.0), scale: c(scale, 2.0, 1000.0, 40.0), seed, animated }
            }
            Smooth { radius } => Smooth { radius: c(radius, 0.0, 20.0, 0.0) },
            LensFlare { x, y, through_x, through_y, size, intensity, colour, rays, ghosts } => LensFlare {
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                through_x: c(through_x, -65536.0, 65536.0, 0.0),
                through_y: c(through_y, -65536.0, 65536.0, 0.0),
                size: c(size, 1.0, 4096.0, 1.0),
                intensity: c(intensity, 0.0, 4.0, 0.0),
                colour,
                rays: if rays < 2 { 0 } else { rays.min(32) },
                ghosts: ghosts.min(look::MAX_GHOSTS),
            },
            Displace { layer, amount_x, amount_y, brightness } => {
                Displace { layer, amount_x: c(amount_x, -200.0, 200.0, 0.0), amount_y: c(amount_y, -200.0, 200.0, 0.0), brightness }
            }
            MotionBlur { shutter, samples } => MotionBlur { shutter: c(shutter, 0.0, 2.0, 0.0), samples: samples.clamp(2, MAX_BLUR_SAMPLES) },
            Trails { count, step, fade } => Trails { count: count.clamp(1, MAX_TRAILS), step: step.clamp(1, 24), fade: c(fade, 0.0, 1.0, 0.5) },
            FrameBlend { amount } => FrameBlend { amount: c(amount, 0.0, 1.0, 0.0) },
            TimeRemap { speed, offset } => TimeRemap { speed: c(speed, -8.0, 8.0, 1.0), offset: c(offset, -100_000.0, 100_000.0, 0.0) },
            Transition { style, start, length, out, angle, x, y, size } => Transition {
                style,
                start,
                length: length.clamp(1, 10_000),
                out,
                angle: c(angle, -3600.0, 3600.0, 0.0),
                x: c(x, -65536.0, 65536.0, 0.0),
                y: c(y, -65536.0, 65536.0, 0.0),
                size: c(size, 1.0, 65536.0, 1.0),
            },
            Particles { style, x, y, width, height, rate, life, speed, angle, spread, gravity, size, colour, opacity, seed } => {
                let life = life.clamp(1, 600);
                Particles {
                    style,
                    x: c(x, -65536.0, 65536.0, 0.0),
                    y: c(y, -65536.0, 65536.0, 0.0),
                    width: c(width, 0.0, 65536.0, 0.0),
                    height: c(height, 0.0, 65536.0, 0.0),
                    // never more than MAX_PARTICLES alive at once
                    rate: c(rate, 0.0, 500.0, 0.0).min(MAX_PARTICLES as f32 / life as f32),
                    life,
                    speed: c(speed, 0.0, 200.0, 0.0),
                    angle: c(angle, -3600.0, 3600.0, 90.0),
                    spread: c(spread, 0.0, 360.0, 0.0),
                    gravity: c(gravity, -10.0, 10.0, 0.0),
                    size: c(size, 0.5, 64.0, 1.0),
                    colour,
                    opacity: c(opacity, 0.0, 1.0, 1.0),
                    seed,
                }
            }
            MatteTone { layer, colour, amount, offset_x, offset_y, softness, highlight } => MatteTone {
                layer,
                colour,
                amount: c(amount, 0.0, 1.0, 0.0),
                offset_x: c(offset_x, -500.0, 500.0, 0.0),
                offset_y: c(offset_y, -500.0, 500.0, 0.0),
                softness: c(softness, 0.0, 100.0, 0.0),
                highlight,
            },
            NormalLight { normals, angle, elevation, depth, intensity, colour } => NormalLight {
                normals,
                angle: c(angle, -3600.0, 3600.0, 0.0),
                elevation: c(elevation, 5.0, 90.0, 45.0),
                depth: c(depth, 1.0, 100.0, 1.0),
                intensity: c(intensity, 0.0, 2.0, 0.0),
                colour,
            },
        }
    }

    /// Parameters `t` of the way from `self` to `to` (0…1). Numbers and colours interpolate;
    /// switches, seeds and shapes hold `self`'s until `to` is reached. Another kind: `self`.
    pub fn lerp(&self, to: &EffectKind, t: f32) -> EffectKind {
        use EffectKind::*;
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        if t >= 1.0 && self.same_kind(to) {
            return to.clone();
        }
        let l = |a: f32, b: f32| a + (b - a) * t;
        let lc = |a: [u8; 3], b: [u8; 3]| -> [u8; 3] { [0, 1, 2].map(|i| l(a[i] as f32, b[i] as f32).round().clamp(0.0, 255.0) as u8) };
        match (self, to) {
            (Blur { radius: a }, Blur { radius: b }) => Blur { radius: l(*a, *b) },
            (DirectionalBlur { length: a, angle: aa }, DirectionalBlur { length: b, angle: ba }) => DirectionalBlur { length: l(*a, *b), angle: l(*aa, *ba) },
            (RadialBlur { amount: a, centre_x: ax, centre_y: ay }, RadialBlur { amount: b, centre_x: bx, centre_y: by }) => {
                RadialBlur { amount: l(*a, *b), centre_x: l(*ax, *bx), centre_y: l(*ay, *by) }
            }
            (Glow { radius: ar, intensity: ai, colour: ac }, Glow { radius: br, intensity: bi, colour: bc }) => {
                Glow { radius: l(*ar, *br), intensity: l(*ai, *bi), colour: lc(*ac, *bc) }
            }
            (DropShadow { dx: a1, dy: a2, blur: a3, colour: ac, opacity: a4 }, DropShadow { dx: b1, dy: b2, blur: b3, colour: bc, opacity: b4 }) => {
                DropShadow { dx: l(*a1, *b1), dy: l(*a2, *b2), blur: l(*a3, *b3), colour: lc(*ac, *bc), opacity: l(*a4, *b4) }
            }
            (Outline { width: a, colour: ac }, Outline { width: b, colour: bc }) => Outline { width: l(*a, *b), colour: lc(*ac, *bc) },
            (Thicken { amount: a }, Thicken { amount: b }) => Thicken { amount: l(*a, *b) },
            (
                Levels { in_black: a1, in_white: a2, gamma: a3, out_black: a4, out_white: a5 },
                Levels { in_black: b1, in_white: b2, gamma: b3, out_black: b4, out_white: b5 },
            ) => Levels { in_black: l(*a1, *b1), in_white: l(*a2, *b2), gamma: l(*a3, *b3), out_black: l(*a4, *b4), out_white: l(*a5, *b5) },
            (Curves { points: a }, Curves { points: b }) if a.len() == b.len() => {
                Curves { points: a.iter().zip(b).map(|(p, q)| [l(p[0], q[0]), l(p[1], q[1])]).collect() }
            }
            (HueSaturation { hue: a1, saturation: a2, lightness: a3 }, HueSaturation { hue: b1, saturation: b2, lightness: b3 }) => {
                HueSaturation { hue: l(*a1, *b1), saturation: l(*a2, *b2), lightness: l(*a3, *b3) }
            }
            (Tint { colour: ac, amount: a }, Tint { colour: bc, amount: b }) => Tint { colour: lc(*ac, *bc), amount: l(*a, *b) },
            (Grain { amount: a, size: asz, seed, animated }, Grain { amount: b, size: bsz, .. }) => {
                Grain { amount: l(*a, *b), size: l(*asz, *bsz), seed: *seed, animated: *animated }
            }
            (Texture { kind, scale: a1, angle: a2, contrast: a3, amount: a4 }, Texture { scale: b1, angle: b2, contrast: b3, amount: b4, .. }) => {
                Texture { kind: *kind, scale: l(*a1, *b1), angle: l(*a2, *b2), contrast: l(*a3, *b3), amount: l(*a4, *b4) }
            }
            (LightWrap { radius: ar, intensity: ai }, LightWrap { radius: br, intensity: bi }) => LightWrap { radius: l(*ar, *br), intensity: l(*ai, *bi) },
            (AlphaThreshold { threshold: a }, AlphaThreshold { threshold: b }) => AlphaThreshold { threshold: l(*a, *b) },
            (
                Mask { shape, x: ax, y: ay, width: aw, height: ah, feather: af, invert, points: ap },
                Mask { x: bx, y: by, width: bw, height: bh, feather: bf, points: bp, .. },
            ) => Mask {
                shape: *shape,
                x: l(*ax, *bx),
                y: l(*ay, *by),
                width: l(*aw, *bw),
                height: l(*ah, *bh),
                feather: l(*af, *bf),
                invert: *invert,
                // shapes with as many points blend point by point; others hold
                points: if ap.len() == bp.len() { ap.iter().zip(bp).map(|(p, q)| [l(p[0], q[0]), l(p[1], q[1])]).collect() } else { ap.clone() },
            },
            (
                LensFlare { x: ax, y: ay, through_x: atx, through_y: aty, size: asz, intensity: ai, colour: ac, rays, ghosts },
                LensFlare { x: bx, y: by, through_x: btx, through_y: bty, size: bsz, intensity: bi, colour: bc, .. },
            ) => LensFlare {
                x: l(*ax, *bx),
                y: l(*ay, *by),
                through_x: l(*atx, *btx),
                through_y: l(*aty, *bty),
                size: l(*asz, *bsz),
                intensity: l(*ai, *bi),
                colour: lc(*ac, *bc),
                rays: *rays,
                ghosts: *ghosts,
            },
            (Displace { layer, amount_x: ax, amount_y: ay, brightness }, Displace { amount_x: bx, amount_y: by, .. }) => {
                Displace { layer: *layer, amount_x: l(*ax, *bx), amount_y: l(*ay, *by), brightness: *brightness }
            }
            (MotionBlur { shutter: a, samples }, MotionBlur { shutter: b, .. }) => MotionBlur { shutter: l(*a, *b), samples: *samples },
            (Trails { count, step, fade: a }, Trails { fade: b, .. }) => Trails { count: *count, step: *step, fade: l(*a, *b) },
            (FrameBlend { amount: a }, FrameBlend { amount: b }) => FrameBlend { amount: l(*a, *b) },
            (TimeRemap { speed: a, offset: ao }, TimeRemap { speed: b, offset: bo }) => TimeRemap { speed: l(*a, *b), offset: l(*ao, *bo) },
            (
                MatteTone { layer, colour: ac, amount: aa, offset_x: ax, offset_y: ay, softness: asf, highlight },
                MatteTone { colour: bc, amount: ba, offset_x: bx, offset_y: by, softness: bs, .. },
            ) => MatteTone {
                layer: *layer,
                colour: lc(*ac, *bc),
                amount: l(*aa, *ba),
                offset_x: l(*ax, *bx),
                offset_y: l(*ay, *by),
                softness: l(*asf, *bs),
                highlight: *highlight,
            },
            (
                NormalLight { normals, angle: aa, elevation: ae, depth: ad, intensity: ai, colour: ac },
                NormalLight { angle: ba, elevation: be, depth: bd, intensity: bi, colour: bc, .. },
            ) => {
                NormalLight { normals: *normals, angle: l(*aa, *ba), elevation: l(*ae, *be), depth: l(*ad, *bd), intensity: l(*ai, *bi), colour: lc(*ac, *bc) }
            }
            (ColourKey { colour: ac, tolerance: at, softness: asf }, ColourKey { colour: bc, tolerance: bt, softness: bs }) => {
                ColourKey { colour: lc(*ac, *bc), tolerance: l(*at, *bt), softness: l(*asf, *bs) }
            }
            _ => self.clone(),
        }
    }
}

/// Clamp a hostile number; NaN and infinities become `nan`.
fn c(v: f32, lo: f32, hi: f32, nan: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { nan }
}

fn yes() -> bool {
    true
}

/// The parameters of an effect from one frame on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectKey {
    pub frame: u32,
    pub value: EffectKind,
}

/// One entry of a layer's effect stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The parameters when the effect has no keys.
    pub kind: EffectKind,
    /// Keys sorted by frame, all of `kind`'s kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<EffectKey>,
    /// How the effect's result combines with what it was given (normal: replaces it).
    #[serde(default, skip_serializing_if = "is_normal")]
    pub blend: BlendMode,
    /// How much of the result is used, 0…1 (1: all of it).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub mix: f32,
}

fn is_normal(b: &BlendMode) -> bool {
    *b == BlendMode::Normal
}
fn one() -> f32 {
    1.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}

impl Effect {
    pub fn new(kind: EffectKind) -> Self {
        Self { enabled: true, kind: kind.sanitized(), keys: Vec::new(), blend: BlendMode::Normal, mix: 1.0 }
    }

    /// The parameters at `frame`.
    pub fn at(&self, frame: u32) -> EffectKind {
        let k = &self.keys;
        let Some(first) = k.first() else { return self.kind.clone() };
        if frame <= first.frame {
            return first.value.clone();
        }
        let i = k.partition_point(|key| key.frame <= frame);
        match (k.get(i.wrapping_sub(1)), k.get(i)) {
            (Some(a), Some(b)) if b.frame > a.frame => a.value.lerp(&b.value, (frame - a.frame) as f32 / (b.frame - a.frame) as f32),
            (Some(a), _) => a.value.clone(),
            _ => self.kind.clone(),
        }
    }

    /// Set (or replace) the key at `frame`. A value of another kind is refused.
    pub fn set_key(&mut self, frame: u32, value: EffectKind) -> bool {
        if !value.same_kind(&self.kind) {
            return false;
        }
        let value = value.sanitized();
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                if let Some(k) = self.keys.get_mut(i) {
                    k.value = value;
                }
            }
            Err(i) => {
                if self.keys.len() >= MAX_KEYS {
                    return false;
                }
                self.keys.insert(i, EffectKey { frame, value });
            }
        }
        true
    }

    /// Remove the key at `frame`; true if there was one.
    pub fn remove_key(&mut self, frame: u32) -> bool {
        let before = self.keys.len();
        self.keys.retain(|k| k.frame != frame);
        self.keys.len() != before
    }

    /// Change the parameters at `frame`: with keys, set a key there; without, change the effect.
    pub fn edit(&mut self, frame: u32, value: EffectKind) -> bool {
        if !value.same_kind(&self.kind) {
            return false;
        }
        if self.keys.is_empty() {
            self.kind = value.sanitized();
            true
        } else {
            self.set_key(frame, value)
        }
    }

    /// Clamp every parameter, drop keys of another kind and duplicate frames, sort the keys.
    pub fn sanitized(mut self) -> Effect {
        self.kind = self.kind.sanitized();
        let kind = self.kind.clone();
        let mut keys: Vec<EffectKey> = self
            .keys
            .into_iter()
            .filter(|k| k.value.same_kind(&kind))
            .take(MAX_KEYS)
            .map(|k| EffectKey { frame: k.frame, value: k.value.sanitized() })
            .collect();
        keys.sort_by_key(|k| k.frame);
        keys.dedup_by_key(|k| k.frame);
        self.keys = keys;
        self.mix = c(self.mix, 0.0, 1.0, 1.0);
        self
    }
}

/// Sanitize a whole stack (at most [`MAX_EFFECTS`]).
pub fn sanitize_stack(effects: Vec<Effect>) -> Vec<Effect> {
    effects.into_iter().take(MAX_EFFECTS).map(Effect::sanitized).collect()
}

/// Does any enabled effect of the stack change pixels?
pub fn is_active(effects: &[Effect]) -> bool {
    effects.iter().any(|e| e.enabled)
}

/// Does any enabled effect read the layers below (so the composite under the layer is needed)?
pub fn needs_below(effects: &[Effect]) -> bool {
    effects.iter().any(|e| e.enabled && e.kind.needs_below())
}

/// The layers (ids) the enabled effects read, each once, in stack order.
pub fn source_layers(effects: &[Effect]) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    for id in effects.iter().take(MAX_EFFECTS).filter(|e| e.enabled).filter_map(|e| e.kind.source_layer()) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// The enabled time effects of a stack at `frame` (see [`EffectKind::is_time`]), in order.
pub fn time_effects(effects: &[Effect], frame: u32) -> Vec<EffectKind> {
    effects.iter().take(MAX_EFFECTS).filter(|e| e.enabled && e.kind.is_time()).map(|e| e.at(frame).sanitized()).collect()
}

/// Does an enabled effect make pixels of its own (see [`EffectKind::generates`])?
pub fn generates(effects: &[Effect]) -> bool {
    effects.iter().take(MAX_EFFECTS).any(|e| e.enabled && e.kind.generates())
}

/// What a stack may read besides its own layer: the composite under the layer (light wrap) and
/// other layers' pixels (displacement), each the same size and place as the image the stack runs
/// on. An effect whose input is missing leaves the picture as it is.
#[derive(Clone, Copy, Default)]
pub struct Inputs<'a> {
    pub below: Option<&'a RgbaImage>,
    /// (layer id, its pixels) for [`source_layers`].
    pub layers: &'a [(u64, RgbaImage)],
}

/// Run the enabled effects of a stack over `img` (premultiplied, the whole canvas), in order.
/// `below` is the composite under the layer, for light wrap (without it light wrap does nothing).
pub fn apply_stack(img: &mut RgbaImage, effects: &[Effect], frame: u32, below: Option<&RgbaImage>) {
    apply_stack_at(img, (0, 0), effects, frame, below);
}

/// How far (pixels) an effect can move pixels or read its neighbours; `None`: anywhere on the
/// canvas (zoom blur), so it must run on the whole canvas.
pub fn reach(kind: &EffectKind) -> Option<u32> {
    let r = |v: f32| (v.max(0.0).ceil() as u32).saturating_add(2);
    match kind {
        EffectKind::Blur { radius } => Some(r(radius * 3.0)),
        EffectKind::DirectionalBlur { length, .. } => Some(r(length / 2.0)),
        EffectKind::RadialBlur { .. } => None,
        EffectKind::Glow { radius, .. } | EffectKind::LightWrap { radius, .. } => Some(r(radius * 3.0)),
        EffectKind::DropShadow { dx, dy, blur, .. } => Some(r(dx.abs().max(dy.abs()) + blur * 3.0)),
        EffectKind::Outline { width, .. } => Some(r(*width)),
        EffectKind::Thicken { amount } => Some(r(*amount)),
        EffectKind::Levels { .. }
        | EffectKind::Curves { .. }
        | EffectKind::HueSaturation { .. }
        | EffectKind::Tint { .. }
        | EffectKind::Grain { .. }
        | EffectKind::Texture { .. }
        | EffectKind::AlphaThreshold { .. }
        | EffectKind::Mask { .. }
        | EffectKind::ColourKey { .. }
        | EffectKind::GradientMap { .. }
        | EffectKind::Posterize { .. }
        | EffectKind::Threshold { .. }
        | EffectKind::Vignette { .. } => Some(0),
        EffectKind::ChromaticAberration { amount } => Some(r(*amount)),
        EffectKind::Ripple { amplitude, .. } | EffectKind::Wave { amplitude, .. } => Some(r(*amplitude)),
        EffectKind::Turbulence { amount, .. } => Some(r(*amount)),
        EffectKind::Smooth { radius } => Some(r(radius * 3.0)),
        EffectKind::LensFlare { .. } => None,
        EffectKind::MotionBlur { .. } | EffectKind::Trails { .. } | EffectKind::FrameBlend { .. } | EffectKind::TimeRemap { .. } => Some(0),
        EffectKind::Transition { style: TransitionStyle::Push, .. } => None,
        EffectKind::Transition { .. } => Some(0),
        EffectKind::Particles { .. } => None,
        EffectKind::MatteTone { offset_x, offset_y, softness, .. } => Some(r(offset_x.abs().max(offset_y.abs()) + softness * 1.5 + 2.0)),
        // the shape's height is blurred over `depth / 2` and read one pixel either side
        EffectKind::NormalLight { depth, .. } => Some(r(depth * 1.5 + 2.0)),
        EffectKind::Displace { amount_x, amount_y, .. } => Some(r(amount_x.abs().max(amount_y.abs()))),
    }
}

/// The reach of a whole stack at `frame` (the enabled effects' reaches add up).
pub fn stack_reach(effects: &[Effect], frame: u32) -> Option<u32> {
    effects.iter().take(MAX_EFFECTS).filter(|e| e.enabled).try_fold(0u32, |acc, e| reach(&e.at(frame).sanitized()).map(|r| acc.saturating_add(r)))
}

/// [`apply_stack`] on a part of the canvas: `img` (and `below`, the same size) sit at canvas
/// position `origin`. Pixels outside `img` must be transparent and further than the stack's
/// [`stack_reach`] from anything drawn; then the result equals running on the whole canvas.
pub fn apply_stack_at(img: &mut RgbaImage, origin: (u32, u32), effects: &[Effect], frame: u32, below: Option<&RgbaImage>) {
    apply_stack_with(img, origin, effects, frame, Inputs { below, layers: &[] });
}

/// [`apply_stack_at`] with every input a stack may read ([`Inputs`]).
pub fn apply_stack_with(img: &mut RgbaImage, origin: (u32, u32), effects: &[Effect], frame: u32, inputs: Inputs<'_>) {
    for e in effects.iter().take(MAX_EFFECTS) {
        if !e.enabled || e.mix <= 0.0 {
            continue;
        }
        let k = e.at(frame).sanitized();
        if e.blend == BlendMode::Normal && (e.mix.is_nan() || e.mix >= 1.0) {
            apply_at(img, origin, &k, frame, inputs);
        } else {
            let input = img.clone();
            apply_at(img, origin, &k, frame, inputs);
            combine(img, &input, e.blend, e.mix);
        }
    }
}

/// `out` becomes `input` with the effect's result `out` laid over it in `mode`, `mix` of the way.
fn combine(out: &mut RgbaImage, input: &RgbaImage, mode: BlendMode, mix: f32) {
    let m = if mix.is_finite() { mix.clamp(0.0, 1.0) } else { 1.0 };
    for (o, i) in out.data.as_chunks_mut::<4>().0.iter_mut().zip(input.data.as_chunks::<4>().0.iter()) {
        let (fx, src) = ([o[0], o[1], o[2], o[3]], [i[0], i[1], i[2], i[3]]);
        let target = if mode == BlendMode::Normal { fx } else { tooncraft_raster::blend(src, fx, mode) };
        for c in 0..4 {
            o[c] = (src[c] as f32 + (target[c] as f32 - src[c] as f32) * m).round().clamp(0.0, 255.0) as u8;
        }
        let a = o[3];
        for c in 0..3 {
            o[c] = o[c].min(a);
        }
    }
}

/// Run one effect over `img` (premultiplied, the whole canvas). Parameters are expected sanitized.
pub fn apply(img: &mut RgbaImage, kind: &EffectKind, frame: u32, below: Option<&RgbaImage>) {
    apply_at(img, (0, 0), kind, frame, Inputs { below, layers: &[] });
}

fn apply_at(img: &mut RgbaImage, origin: (u32, u32), kind: &EffectKind, frame: u32, inputs: Inputs<'_>) {
    let below = inputs.below;
    if img.width == 0 || img.height == 0 || img.data.len() != img.width as usize * img.height as usize * 4 {
        return;
    }
    match kind {
        EffectKind::Blur { radius } => blur::gaussian(img, *radius),
        EffectKind::DirectionalBlur { length, angle } => blur::directional(img, *length, *angle),
        EffectKind::RadialBlur { amount, centre_x, centre_y } => blur::radial(img, *amount, *centre_x, *centre_y),
        EffectKind::Glow { radius, intensity, colour } => blur::glow(img, *radius, *intensity, *colour),
        EffectKind::DropShadow { dx, dy, blur, colour, opacity } => blur::drop_shadow(img, (*dx, *dy), *blur, *colour, *opacity),
        EffectKind::Outline { width, colour } => distance::outline(img, *width, *colour),
        EffectKind::Thicken { amount } => distance::thicken(img, *amount),
        EffectKind::Levels { in_black, in_white, gamma, out_black, out_white } => {
            colour::lut(img, &colour::levels_table(*in_black, *in_white, *gamma, *out_black, *out_white))
        }
        EffectKind::Curves { points } => colour::lut(img, &colour::curve_table(points)),
        EffectKind::HueSaturation { hue, saturation, lightness } => colour::hue_saturation(img, *hue, *saturation, *lightness),
        EffectKind::Tint { colour, amount } => colour::tint(img, *colour, *amount),
        EffectKind::Grain { amount, size, seed, animated } => colour::grain(img, origin, *amount, *size, *seed, if *animated { frame } else { 0 }),
        EffectKind::Texture { kind, scale, angle, contrast, amount } => {
            colour::texture(img, origin, &tooncraft_brush::Grain::new(*kind, *scale, *angle, *contrast, None), *amount)
        }
        EffectKind::LightWrap { radius, intensity } => {
            if let Some(b) = below {
                blur::light_wrap(img, b, *radius, *intensity)
            }
        }
        EffectKind::AlphaThreshold { threshold } => colour::alpha_threshold(img, *threshold),
        EffectKind::Mask { shape: MaskShape::Drawn, feather, invert, points, .. } => shape::drawn(img, origin, points, *feather, *invert),
        EffectKind::Mask { shape, x, y, width, height, feather, invert, .. } => {
            shape::mask(img, *shape, (*x - origin.0 as f32, *y - origin.1 as f32, *width, *height), *feather, *invert)
        }
        EffectKind::ColourKey { colour, tolerance, softness } => colour::key(img, *colour, *tolerance, *softness),
        EffectKind::GradientMap { dark, light, amount } => look::gradient_map(img, *dark, *light, *amount),
        EffectKind::Posterize { levels } => look::posterize(img, *levels),
        EffectKind::Threshold { level } => look::threshold(img, *level),
        EffectKind::Vignette { x, y, radius, softness, amount } => look::vignette(img, origin, (*x, *y), *radius, *softness, *amount),
        EffectKind::ChromaticAberration { amount } => look::chromatic_aberration(img, *amount),
        EffectKind::Ripple { x, y, amplitude, wavelength, speed } => look::ripple(img, origin, (*x, *y), *amplitude, *wavelength, *speed * frame as f32),
        EffectKind::Wave { amplitude, wavelength, angle, speed } => look::wave(img, origin, *amplitude, *wavelength, *angle, *speed * frame as f32),
        EffectKind::Turbulence { amount, scale, seed, animated } => {
            look::turbulence(img, origin, *amount, *scale, seed.wrapping_add(if *animated { frame } else { 0 }))
        }
        EffectKind::Smooth { radius } => look::smooth(img, *radius),
        EffectKind::LensFlare { x, y, through_x, through_y, size, intensity, colour, rays, ghosts } => look::lens_flare(
            img,
            origin,
            &look::Flare {
                light: (*x, *y),
                through: (*through_x, *through_y),
                size: *size,
                intensity: *intensity,
                colour: *colour,
                rays: *rays,
                ghosts: *ghosts,
            },
        ),
        EffectKind::Displace { layer, amount_x, amount_y, brightness } => {
            if let Some((_, map)) = inputs.layers.iter().find(|(id, _)| id == layer) {
                look::displace_by(img, map, *amount_x, *amount_y, *brightness)
            }
        }
        // worked out by the compositor from other frames
        EffectKind::MotionBlur { .. } | EffectKind::Trails { .. } | EffectKind::FrameBlend { .. } | EffectKind::TimeRemap { .. } => {}
        EffectKind::Particles { style, x, y, width, height, rate, life, speed, angle, spread, gravity, size, colour, opacity, seed } => particles::draw(
            img,
            origin,
            &particles::Emitter {
                style: *style,
                rect: (*x, *y, *width, *height),
                rate: *rate,
                life: *life,
                speed: *speed,
                angle: *angle,
                spread: *spread,
                gravity: *gravity,
                size: *size,
                colour: *colour,
                opacity: *opacity,
                seed: *seed,
            },
            frame,
        ),
        EffectKind::MatteTone { layer, colour, amount, offset_x, offset_y, softness, highlight } => {
            if let Some((_, map)) = inputs.layers.iter().find(|(id, _)| id == layer) {
                light::matte_tone(img, map, *colour, *amount, (*offset_x, *offset_y), *softness, *highlight)
            }
        }
        EffectKind::NormalLight { normals, angle, elevation, depth, intensity, colour } => {
            let map = normals.and_then(|n| inputs.layers.iter().find(|(id, _)| *id == n)).map(|(_, m)| m);
            light::normal_light(img, map, &light::Light { angle: *angle, elevation: *elevation, depth: *depth, intensity: *intensity, colour: *colour })
        }
        EffectKind::Transition { style, start, length, out, angle, x, y, size } => time::apply(
            img,
            origin,
            &time::Transition { style: *style, shown: time::shown(*start, *length, *out, frame), angle: *angle, centre: (*x, *y), size: *size },
        ),
    }
}

#[cfg(test)]
mod tests;

/// The built-in effect looks: (id, effects). Names are the app's to translate.
pub fn presets(w: u32, h: u32) -> Vec<(&'static str, Vec<Effect>)> {
    let (wf, hf) = (w as f32, h as f32);
    let e = Effect::new;
    vec![
        (
            "film",
            vec![
                e(EffectKind::GradientMap { dark: [40, 30, 50], light: [255, 236, 200], amount: 0.35 }),
                e(EffectKind::Grain { amount: 0.12, size: 1.5, seed: 7, animated: true }),
                e(EffectKind::Vignette { x: wf / 2.0, y: hf / 2.0, radius: wf.min(hf) * 0.35, softness: wf.min(hf) * 0.45, amount: 0.6 }),
            ],
        ),
        ("glow", vec![e(EffectKind::Glow { radius: 14.0, intensity: 1.2, colour: [255, 240, 200] }), e(EffectKind::ChromaticAberration { amount: 1.5 })]),
        ("ink", vec![e(EffectKind::Smooth { radius: 1.2 }), e(EffectKind::Thicken { amount: 0.5 })]),
        (
            "dream",
            vec![
                e(EffectKind::Blur { radius: 2.0 }),
                e(EffectKind::Glow { radius: 20.0, intensity: 0.8, colour: [230, 210, 255] }),
                e(EffectKind::Wave { amplitude: 2.0, wavelength: 80.0, angle: 0.0, speed: 0.04 }),
            ],
        ),
        ("comic", vec![e(EffectKind::Posterize { levels: 5 }), e(EffectKind::Outline { width: 2.0, colour: [20, 20, 20] })]),
    ]
}

/// Largest saved stack file read.
pub const MAX_STACK_FILE: usize = 1 << 20;

/// A stack as `.toonfx` JSON (`{"effects": [...]}`).
pub fn stack_to_json(effects: &[Effect]) -> Result<String, String> {
    serde_json::to_string_pretty(&serde_json::json!({ "toonfx": 1, "effects": effects })).map_err(|e| e.to_string())
}

/// A stack from `.toonfx` JSON: bounded, every effect sanitized.
pub fn stack_from_json(text: &str) -> Result<Vec<Effect>, String> {
    if text.len() > MAX_STACK_FILE {
        return Err("the effect stack file is too large".into());
    }
    #[derive(Deserialize)]
    struct File {
        effects: Vec<Effect>,
    }
    let f: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if f.effects.len() > MAX_EFFECTS {
        return Err(format!("an effect stack holds at most {MAX_EFFECTS} effects"));
    }
    Ok(f.effects.into_iter().map(Effect::sanitized).collect())
}
