# ToonCraft vs TVPaint, Harmony, Clip Studio Paint and Procreate Dreams: parity map

> **Last reviewed:** 2026-11-18 · **Last updated:** 2026-11-18 · **Change:** major (full-product map) · **Target:** TVPaint Animation 12 (raster frame-by-frame depth), Toon Boom Harmony 24 Premium (the drawing and animation side, plus the cut-out and rigging basics an animator expects), Clip Studio Paint EX 4 (drawing tools and the animation timeline), Procreate Dreams 1.x (touch-first performance animation)

This is the only file that names the reference products. Every row was written from their public
manuals, feature lists, release notes and tutorial videos, in our own words. No code, strings,
icons, assets or file internals were read (see [AGENTS.md](../AGENTS.md), clean room). Labels
inside ToonCraft are our own ([feature-names.md](feature-names.md)).

The program goal is **90 % parity, measured honestly**. Until this revision the map had 155 rows
written alongside the features, scored 87 % with every row counting the same, and treated a whole
effects package or the exposure sheet as one row each. That number could not measure a 90 % goal.
This revision lists the full products area by area, gives every row an effort size and every area
a weight, and the score below is counted from it.

## Headline

`cargo xtask parity` counts every table below and writes this block; `cargo xtask ci` validates the
rows strictly but only warns when the block is stale (feature PRs flip rows only; a maintainer
refreshes the block with `cargo xtask parity --write` in a separate PR). Numbers here and in
[ROADMAP.md](../ROADMAP.md) are copied from its output, never estimated by hand.

<!-- parity:start -->
| | Value | Kind |
|---|---|---|
| In-scope feature rows | 267 | counted |
| Done / exceeds / partial / missing | 112 / 3 / 34 / 118 | counted |
| Out of scope (not in any denominator) | 4 | counted |
| **Row parity**, (D + X + ½·P) / rows | **49.4 %** | counted |
| **Weighted parity**, area weights × size-weighted rows | **36.7 %** | counted |
| Size-weighted parity over all rows, no area weights | 35.5 % | counted |

| Area | Weight | Rows | D | X | P | M | Out | Row parity | Effort parity |
|---|---|---|---|---|---|---|---|---|---|
| Brush engine | 9 (9 %) | 29 | 10 | 0 | 2 | 17 | 1 | 38 % | 27 % |
| Paper and textures | 3 (3 %) | 10 | 5 | 0 | 2 | 3 | 0 | 60 % | 55 % |
| Drawing tools | 7 (7 %) | 17 | 3 | 0 | 1 | 13 | 0 | 21 % | 9 % |
| Selection and transform | 5 (5 %) | 14 | 4 | 0 | 1 | 9 | 0 | 32 % | 25 % |
| Colour and paint | 7 (7 %) | 18 | 5 | 0 | 3 | 10 | 0 | 36 % | 23 % |
| Layers | 6 (6 %) | 13 | 6 | 0 | 1 | 6 | 0 | 50 % | 43 % |
| Timeline and exposure sheet | 9 (9 %) | 20 | 10 | 0 | 4 | 6 | 0 | 60 % | 36 % |
| Onion skin and light table | 4 (4 %) | 11 | 7 | 0 | 1 | 3 | 0 | 68 % | 62 % |
| Animation tools | 9 (9 %) | 18 | 8 | 1 | 2 | 7 | 0 | 56 % | 35 % |
| Camera | 4 (4 %) | 8 | 4 | 0 | 0 | 4 | 0 | 50 % | 40 % |
| Effects and compositing | 7 (7 %) | 22 | 10 | 0 | 3 | 9 | 0 | 52 % | 32 % |
| Audio and lip-sync | 4 (4 %) | 14 | 10 | 0 | 1 | 3 | 0 | 75 % | 67 % |
| Import, export and pipeline | 7 (7 %) | 28 | 14 | 0 | 3 | 11 | 1 | 55 % | 46 % |
| Scripting and automation | 4 (4 %) | 7 | 1 | 0 | 2 | 4 | 0 | 29 % | 22 % |
| UI and workspace | 5 (5 %) | 14 | 5 | 1 | 4 | 4 | 0 | 57 % | 53 % |
| Tablet and input | 4 (4 %) | 9 | 2 | 0 | 3 | 4 | 1 | 39 % | 35 % |
| Collaboration and review | 2 (2 %) | 5 | 1 | 0 | 0 | 4 | 1 | 20 % | 11 % |
| Stability and performance | 4 (4 %) | 10 | 7 | 1 | 1 | 1 | 0 | 85 % | 69 % |
<!-- parity:end -->

## How to read the map

**Status** per row, checked against the code on `main` (2026-11-18, commit `a4ed418`):

| Code | Meaning | Credit |
|---|---|---|
| D | Done: works in ToonCraft today, end to end, and a test covers it | 1 |
| X | Exceeds the references in a way we mean to keep | 1 |
| P | Partial: works with the limit given in the notes | ½ |
| M | Missing | 0 |
| O | Out of scope, with the reason in the notes; left out of every denominator | — |

**Refs** says which reference has the feature in a comparable form: **TV** TVPaint Animation,
**TB** Toon Boom Harmony, **CS** Clip Studio Paint EX, **PD** Procreate Dreams; **all** means all
four. **—** marks a suite rule none of them meets (only used on X rows).

**Size** is the effort to reach parity on that row, in agent hours, rough: **S** up to about 4 h
(weight 1), **M** about 4–12 h (2), **L** about 12–40 h (4), **XL** 40 h or more (8). The
calibration: the merged slices so far (pegs, the effect stack, the sound tracks, the brush library)
each took 4–10 hours for 1–4 rows.

**Scores.** *Row parity* counts every in-scope row the same. *Effort parity* weights each row of an
area by its size, so a missing exposure sheet weighs four times a missing colour-history strip.
*Weighted parity* averages the areas' effort parities with the area weights; an area's weight says
how much an animator coming from the references relies on it and how much of the products' depth
sits in it. Weighted parity is the number the 90 % goal is measured against; row parity is shown
because it is the simpler count.

**Out of scope.** A row is marked O only when matching it is impossible or meaningless for an open,
Rust-only, offline desktop app: a vendor's cloud service, reading a vendor's proprietary native
project files where the legal position is unclear, or a platform the suite does not target.
Everything an animator would expect from a full product stays in the denominator, however large;
that includes vector stroke layers and the deformation rig, which ToonCraft does not have yet.

**Row rules** (enforced by `cargo xtask parity`, a broken row fails CI and names its line): every
table with a `| Feature | Refs | Status | Size | Notes |` header is a feature table; each of its rows
has exactly those five cells, a status from the table above and a size of S, M, L or XL; a feature
name appears once per area; an area with rows has a `Weight:` line; an O row gives its reason.

## Area weights

| Area | Weight | Why |
|---|---|---|
| Brush engine | 9 | Drawing is most of the working day; TVPaint, Clip Studio and Procreate compete on their brush engines |
| Paper and textures | 3 | Paper look and grain matter, but they are a small part of the products |
| Drawing tools | 7 | Shapes, rulers, symmetry, text, vector strokes: daily use, wide in Clip Studio and Harmony |
| Selection and transform | 5 | Daily for clean-up and re-use of drawings |
| Colour and paint | 7 | Ink-and-paint is half of the traditional pipeline (Harmony's palettes, TVPaint's fill tools) |
| Layers | 6 | Structure of every shot |
| Timeline and exposure sheet | 9 | The core of an animation app: timing, the exposure sheet, scenes |
| Onion skin and light table | 4 | Indispensable, but a contained feature set |
| Animation tools | 9 | Pegs, deformation, cut-out rigs and inbetweening aids: where Harmony's depth sits |
| Camera | 4 | Every shot has one; multiplane and depth are expected |
| Effects and compositing | 7 | Finishing inside the app (TVPaint FX, Harmony's node compositing) |
| Audio and lip-sync | 4 | Needed for dialogue shots; a contained area |
| Import, export and pipeline | 7 | Getting work in and out is a hard requirement in studios |
| Scripting and automation | 4 | Studios automate (TVPaint George, Harmony scripting) |
| UI and workspace | 5 | Workspace quality decides whether people stay |
| Tablet and input | 4 | Pen and touch input quality is felt in every stroke |
| Collaboration and review | 2 | Studio feature; most of it is in Harmony's server |
| Stability and performance | 4 | Everyone depends on it; the references are mature here |

## Readiness by audience

| Audience | Ready? | Why |
|---|---|---|
| Student or hobbyist doing pencil tests and short loops | Preview | Pencil, ink, chalk, airbrush, onion skin, light table, holds, sound and MP4 / GIF export work; no brush editor yet |
| Traditional 2D animator (rough to clean-up) | Preview | Keys and breakdowns, spacing charts, re-timing, pegs, XDTS exchange; no exposure-sheet view, no stroke rulers |
| Ink-and-paint artist | No | Gap-closing fill across frames works; no palette files, linked colour models, lock-transparency or auto-paint |
| Cut-out or rigged character animator | No | Pegs with pivots and easing work; no deformers, kinematics or drawing-swap rigs |
| Compositor finishing shots in the app | Preview | Keyframable effect stack, mattes, colour key, ProRes with alpha; no node graph, particles or motion blur |
| Studio pipeline (many shots, many people) | No | Command-line control and XDTS work; one shot per project, no scripting language, no shared asset database |

## Brush engine

Weight: 9

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Pressure to size and opacity | all | D | S | |
| Tilt to dab angle and flattening | TV CS PD | D | S | |
| Speed to size (thin when fast, or thick) | TV CS | D | S | |
| Pen barrel rotation as an input | TV CS | M | S | The pen sample has no rotation field yet |
| Response curves for every input and dab property | TV CS PD | M | M | Fixed linear amounts per input today |
| Random jitter of size, opacity, angle and scatter | TV CS PD | M | S | |
| Colour jitter per dab and per stroke (hue, saturation, value) | TV CS PD | M | S | |
| Stroke smoothing (moving average) | all | D | S | |
| Pulled-string stabiliser with catch-up at the end | TV CS PD | D | S | |
| Built-in families: pencil, ink, airbrush, chalk, eraser | all | D | M | Each with its own coverage rule; the airbrush builds up with flow |
| Opacity and flow without darkening where dabs overlap | TV CS | D | S | |
| Image brush tips | TV CS PD | D | S | Tips stored in the shot |
| Tip shape: hardness, spacing, roundness and angle | all | P | S | Hardness and spacing; no roundness or fixed angle |
| Brush presets as files and a preset folder | all | D | S | `.toonbrush` files |
| Brush library: categories, favourites, recent brushes, search | TV CS PD | D | M | **Ours:** every list and the search work from the keyboard (Ctrl+B) |
| Brush editor panel with a live stroke preview | TV CS PD | M | M | The Brush panel edits the fixed settings only |
| Wet paint: colour pickup, mixing and dilution | TV CS PD | M | L | |
| Watercolour edges: darkening and bleeding | TV CS PD | M | M | |
| Smudge brush | all | M | M | |
| Blur and soften brush | TV CS PD | M | S | |
| Thick paint with depth and lighting (impasto) | TV CS PD | M | L | |
| Dual tips (a second tip or texture masking the first) | CS PD | M | M | |
| Scatter and spray brushes (many dabs per step) | TV CS PD | M | M | |
| Pattern and ribbon brushes (an image repeated along the stroke) | TV CS | M | M | |
| Animated brushes (a tip that changes per dab or per frame) | TV CS | M | M | |
| Brush blend modes (paint as multiply, behind, and so on) | TV CS PD | M | S | |
| Brushes at large sizes stay real time | all | P | M | Measured up to a 120 px airbrush (0.15 ms a sample); not measured at 500–1000 px ([performance.md](performance.md)) |
| Brush rendering on the GPU | TV CS PD | M | L | **Ours:** a craft-gpu dab path (RFC-0022) with the CPU path kept as the test oracle |
| Import of Photoshop ABR brushes | CS PD | M | M | Tips and basic settings |
| Import of a vendor's own brush packages | CS PD | O | M | Out of scope: the native brush formats of Clip Studio and Procreate are proprietary and undocumented |

## Paper and textures

Weight: 3

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Paper colour | all | D | S | Exported or left out per shot |
| Paper grain that shapes pencil and chalk dabs | TV CS PD | D | S | Hashed grain, so a replay gives the same pixels |
| Visible paper texture under the drawing | TV CS PD | M | S | |
| Paper and grain library, grain images from files | TV CS PD | M | M | One built-in grain |
| Grain scale, rotation and contrast | TV CS | P | S | Scale only |
| Texture overlay on a layer or a fill | CS PD | M | S | |
| Canvas size presets | all | P | S | Four sizes in the new-shot dialog; no user presets |
| Canvas resize and crop with anchors | all | D | S | Drawings move whole pixels, never resampled; one undo step |
| Large canvases (up to 16384 px a side) | TV | D | S | Sparse tiles: empty space costs nothing |
| Field guide and safe areas | TV TB | D | S | A 12-field guide, action-safe and title-safe frames |

## Drawing tools

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Line, rectangle and ellipse with constraints | all | D | S | Shift: 15° lines, squares, circles |
| Polyline, polygon and curve tools | all | M | S | |
| Filled shapes and lasso fill | all | M | S | |
| Eraser | all | D | S | |
| Eyedropper | all | D | S | |
| Gradient tool (linear, radial) | all | M | S | |
| Symmetry and mirror drawing (axes, radial) | TV CS PD | M | M | |
| Stroke rulers: straight, curve, ellipse, concentric | TV TB CS | M | M | |
| Perspective rulers with vanishing points | TV CS PD | M | M | |
| Grid display and snapping strokes to it | TV TB CS | P | S | The field guide draws a grid; strokes do not snap |
| Snap the end of a stroke to a clean line or ellipse when held | CS PD | M | S | |
| Text tool | TV TB CS | M | M | |
| Clone stamp | TV CS | M | S | |
| Reference image viewer with eyedropper | TV CS PD | M | S | |
| Vector stroke layers (strokes kept as editable curves) | TB CS | M | XL | Harmony and Clip Studio draw on vector layers by default |
| Vector line editing: reshape, width, erase to the intersection | TB CS | M | L | Needs the vector stroke layers |
| 3D reference models and posable figures | TB CS | M | XL | |

## Selection and transform

Weight: 5

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Lasso and rectangle selection | all | D | S | |
| Ellipse selection | all | M | S | |
| Select by colour with tolerance (magic wand) | all | M | S | |
| Add, subtract and intersect selections | all | M | S | |
| Invert, grow, shrink and feather a selection | all | M | S | Select all and deselect only |
| Painted selection masks (quick mask) | TV CS PD | M | M | |
| Move, scale and rotate a selection with a live preview | all | D | S | One undo step |
| Flips and 90° turns | all | D | S | |
| Cut, copy and paste pixels between drawings | all | D | S | |
| Free distort, perspective and skew | all | M | S | |
| Mesh warp | TV CS PD | M | M | |
| Liquify (push, twirl, pinch) | TV CS PD | M | M | |
| Transform across several drawings or layers at once | TV TB CS | M | M | |
| Resampling choice (nearest, bilinear, bicubic) | TV CS | P | S | Bilinear only |

## Colour and paint

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Flood fill with tolerance | all | D | S | |
| Close gaps of a set size | all | D | S | |
| Fill reference: current layer or all visible layers | TV TB CS | D | S | |
| Fill under lines (into the antialiasing) | TV TB CS | D | S | |
| Fill every drawing of a layer at once | TV TB | D | S | One undo step |
| Linked line and paint layers | TB TV CS | P | M | Paint on a layer below the ink with the all-visible reference; no linked pair |
| Paint the same regions across drawings automatically | TV TB | M | L | |
| Colour swatches | all | P | S | Up to 64 swatches in the Colour panel; no named palettes |
| Palette files: save, load, import common formats | all | M | S | |
| Linked colour styles (change a swatch, every fill using it updates) | TB CS | M | L | |
| HSV picker with keyboard fields | all | P | S | No behaviour test yet |
| Colour wheel, sliders and harmony modes | TV CS PD | M | S | |
| Recent colours | TV CS PD | M | S | |
| Lock transparency (paint only where there is paint) | all | M | S | |
| Paint behind existing paint | TV CS | M | S | |
| Recolour line art across drawings | TV TB CS | M | M | |
| Colour management: working space, profiles, soft proof | TV CS | M | M | |
| 16-bit colour drawings | TV CS | M | L | |

## Layers

Weight: 6

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Animation layers with their own exposures | all | D | S | |
| Add, delete, rename and reorder layers | all | D | S | |
| Layer visibility, lock and opacity | all | D | S | |
| Duplicate a layer | all | M | S | |
| Merge down, merge visible and flatten | all | M | S | |
| Blend modes | all | P | S | Normal, multiply, screen, overlay, darken, lighten, add; the references have about 25 |
| Nested folders with show, lock, fold and opacity | all | D | S | |
| Clipping layers | TV CS PD | D | S | Several clipping layers on one base |
| Colour labels with a layer filter | TV TB | D | S | |
| Painted layer masks | TV TB CS PD | M | M | |
| Adjustment layers acting on everything below | CS PD | M | M | Effects act on their own layer only |
| Reference layers chosen per tool | TB CS | M | S | The fill reference is current or all visible |
| Layer properties copied between layers | TV CS | M | S | |

## Timeline and exposure sheet

Weight: 9

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Timeline grid with keys, holds and blanks | all | D | S | |
| Holds: a drawing exposed several frames | all | D | S | |
| Insert and delete frames, in one or all layers | all | D | S | |
| New blank drawing and duplicate drawing | all | D | S | |
| Extend and shorten a hold from the keyboard | TV TB | D | S | |
| Change a hold by dragging its cells | TB CS PD | D | S | |
| Select a range of cells across layers | all | D | S | |
| Cut, copy and paste cells | all | D | S | **Ours:** paste linked or as new drawings, in place or inserting |
| Thumbnails in timeline cells | TV PD | D | S | |
| Drawings kept when no longer exposed | TB | D | S | Purging them is undoable |
| Drawing substitution: expose any existing drawing by name | TV TB CS | P | S | Linked paste re-exposes a drawing anywhere; no pick-by-name in the cell |
| Vertical exposure sheet view | TV TB | M | L | |
| Frame markers and notes | TV TB | M | S | |
| Play range, loop and ping-pong | all | P | S | Loop and the selected range; no ping-pong |
| Frame rate change and timecode display | all | P | S | Frame rate per shot; frame numbers only |
| Timeline zoom and scroll | all | P | S | Scroll only |
| Graph editor for keyed values | TB CS PD | M | L | Per-key easing and a two-handle curve; no curve view |
| Several shots or scenes in one project | TV TB | M | L | |
| Nested sequences (a shot or symbol inside a shot) | TB CS PD | M | L | |
| Record transforms live while playing | PD | M | M | |

## Onion skin and light table

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Onion skin on and off | all | D | S | |
| Separate before and after counts | all | D | S | |
| Opacity with falloff | all | D | S | |
| Tint colours for before and after | all | D | S | |
| Onion by drawings or by frames | TV TB | D | S | |
| Onion on the current layer or on all layers | TV CS | P | S | Both work; only the current-layer path is tested |
| Onion skin as outlines | TV | D | S | Also: hide only the step before |
| Onion settings per layer | TV TB | M | S | |
| Pick the exact relative frames shown | TB CS | M | S | |
| Light table: pin any frame of any layer, with opacity | TB CS | D | S | |
| Shift and trace: move or turn a pinned frame without changing it | TB TV | M | M | |

## Animation tools

Weight: 9

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Key, breakdown and inbetween marks with a spacing chart | TV TB | D | S | |
| Flip and scrub through drawings | all | D | S | |
| Rolling and flipping between chosen drawings from the keyboard | TV TB | M | S | |
| Re-time a range, with presets and re-timing between keys | TV TB | D | S | **Ours:** non-destructive, one undo step |
| Reverse a range | TV TB | D | S | |
| Cycles (repeat a range of cells) | TV TB | D | S | |
| Pegs: nested parented transforms with pivots | TB | D | L | Move, rotate, scale and skew keys; loops refused |
| Keyed layer transforms with easing | TB CS PD | D | M | Linear, ease, hold or a custom curve per key |
| Motion path on the canvas | TB PD | D | S | |
| Inbetween guides from the spacing chart | — | X | S | **Ours:** new inbetweens placed along the chart, with blended guides on a locked layer |
| Generated inbetween drawings (morphing between keys) | TV TB | M | L | |
| Cut-out puppets: parts on pegs with drawing swaps | TB | P | L | Parts on pegs work; no swap library or rig templates |
| Deformers: bone, curve and envelope on drawings | TB | M | XL | |
| Inverse kinematics on peg chains | TB | M | L | |
| Master controllers driving poses | TB | M | L | |
| Pose and template library | TB CS | M | M | |
| Rotoscoping on a video layer | TV CS PD | P | M | Image sequences import; no video file import |
| Motion tracking of a point in footage | TV TB | M | L | |

## Camera

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Camera pan, zoom and rotate keys with easing | TV TB PD | D | M | Exports render through the camera |
| Multiplane depth with parallax | TV TB | D | M | |
| Camera shake | TV TB | D | S | Repeatable |
| Camera frame on the canvas, crop to it | all | D | S | |
| Camera path editing on the canvas | TB PD | M | S | |
| Depth of field by layer depth | TB | M | M | |
| Several cameras and camera cuts in a shot | TB | M | M | |
| Free 3D camera over a layered scene | TB | M | L | Depth trucks only |

## Effects and compositing

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Blurs: soft, motion and zoom | TV TB PD | D | S | |
| Glow, drop shadow and outline | TV TB PD | D | S | |
| Colour adjustments: levels, curves, hue and saturation, tint | all | D | S | |
| Non-destructive effect stack per layer | TV TB | D | M | Reorderable, switchable, saved in the shot |
| Keyframed effect settings | TV TB PD | D | S | |
| Blend mode and strength per effect | TV TB | D | S | |
| Effects in every export | TV TB | D | S | MP4, ProRes, GIF and PNG go through the same compositor |
| Film grain and light wrap | TV TB | D | S | |
| Matte layers (keep inside or outside) | TV TB | D | S | |
| Colour key | TV TB | D | S | Tolerance and soft edge |
| Soft masks from shapes | TV TB PD | P | S | Rectangle and ellipse, keyframable; no drawn mask shapes |
| Line thickening and smoothing | TV | P | S | Thicken and thin; no smoothing of jagged lines |
| Gradient map, posterise and threshold | TV TB PD | P | S | Alpha threshold only |
| Motion blur from movement | TV TB | M | M | |
| Distortions: ripple, wave, turbulence, displacement | TV TB | M | M | |
| Lens looks: vignette, chromatic aberration, flares | TV TB PD | M | S | |
| Tone and highlight from mattes, lighting from normals | TV TB | M | L | |
| Particles (rain, snow, sparks) | TV TB | M | L | |
| Time effects: trails, frame blending, time remap | TV TB PD | M | M | |
| Transitions between cuts | TV TB PD | M | S | |
| Effect presets and saved stacks | TV TB | M | S | |
| Node compositing graph | TB | M | XL | |

## Audio and lip-sync

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Import audio files | all | D | S | WAV, AIFF, MP3, AAC / M4A, FLAC, Ogg, Opus through craft-audio |
| Waveform in the timeline | all | D | S | |
| Scrub audio | all | D | S | |
| Several audio tracks and clips | all | D | M | Up to 16 tracks, 64 clips |
| Clip start, trim and fades | TB CS PD | D | S | |
| Volume keys on a clip | TB PD | D | S | |
| Mute and solo tracks | TB PD | D | S | |
| Export the sound mix | TV TB | D | S | WAV; MP4 carries the same mix as AAC |
| Split and duplicate clips | TB CS PD | M | S | |
| Record a voice track from the microphone | TB CS PD | M | M | craft-audio has no capture yet |
| Mouth-chart lip-sync detection | TB | P | M | Offline, model-free rules; checked on synthetic sounds, not on a speech corpus |
| Lip-sync from the typed line of dialogue | TB | M | M | |
| Lip-sync result as editable exposure keys | TB | D | S | |
| Mouth timing sheets in and out | TB | D | S | Our text sheet and the common switch-data `.dat` |

## Import, export and pipeline

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Native project save and open | all | D | S | |
| PNG sequence export | all | D | S | |
| GIF export | TV CS PD | D | S | |
| Animated PNG and WebP export | TV CS | P | S | Animated PNG; no WebP |
| MP4 (H.264) export | all | D | M | Through craft-video and craft-media; no external tools |
| ProRes with alpha | TV TB | D | M | 4444 with alpha, 422 HQ |
| Video export carries the soundtrack | all | D | S | |
| Per-layer PNG sequences | TV TB | D | S | |
| Layered PSD of a frame | TV CS | D | S | Through craft-psd |
| Multi-frame layered PSD and PSB | TV CS | M | S | |
| TGA, TIFF and JPEG sequences | TV TB CS | M | S | |
| OpenEXR sequences, with layers | TV TB | M | M | |
| More video codecs: H.265, VP9 or AV1, MOV H.264 | TV CS PD | M | M | |
| OpenRaster frame export | TV | D | S | |
| Transparent-background export | all | P | S | PNG and OpenRaster; no test |
| Export frame range and output size | all | P | S | Frame range; no output scaling |
| Image sequence import | all | D | S | PNG, JPEG, WebP, TIFF, BMP, GIF |
| Animated GIF import | TV PD | D | S | |
| Layers from PSD or OpenRaster | TV CS | D | S | |
| Video file import | TV CS PD | M | M | |
| XDTS exposure sheet exchange with drawing folders | TB CS | D | M | |
| Storyboard or animatic as the starting layout | TB | D | S | BoardCraft panels with timings and camera moves ([interop.md](interop.md)) |
| Scanned paper: peg-hole registration and clean-up | TV TB | M | L | |
| Camera capture for stop motion and pencil tests | TV | M | M | |
| Scene export for compositing apps (layers, camera, timing) | TV TB | M | M | |
| Printed exposure sheets and frames (PDF) | TV TB | M | S | |
| Shot list and edit decisions out (EDL or OTIO) | TB | M | S | |
| Asset library shared between shots | TB CS | M | M | |
| Reading the references' native project files | all | O | XL | Out of scope: the native project formats of TVPaint, Harmony, Clip Studio and Procreate are proprietary and undocumented; exchange goes through PSD, XDTS and image sequences |

## Scripting and automation

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Headless command line: create, script, export, check | TV TB | D | M | `tooncraft-cli`; every action is an engine command |
| Replay recorded actions (macros) | TV CS | P | M | JSON-lines command files replay; no recording in the app |
| Batch processing of many shots | TV TB | P | S | Through shell scripts around the CLI |
| Embedded scripting language with an object API | TV TB | M | L | |
| Control from other programs over a socket | TB | M | M | **Ours:** planned on craft-control with an MCP server for agents |
| Custom tool buttons that run scripts | TV TB | M | S | |
| Plugin interface for effects and tools | TV TB CS | M | L | |

## UI and workspace

Weight: 5

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Dense dark theme and a light theme | all | D | S | |
| Standard panels: tools, tool options, brush, colour, layers, timeline | all | D | S | |
| Dockable panels: move, tab, float | TV TB CS | P | M | Through craft-ui docking; moving panels is not tested here |
| Saved workspaces | TV TB CS | D | S | Named layouts per user, plus Drawing and Timing |
| Keyboard shortcuts for every tool | all | D | S | |
| Customisable shortcuts | all | D | S | Through craft-keymap, saved per user |
| Command search | TB CS | M | S | |
| Custom tool palettes and radial menus | TV CS PD | M | S | |
| Second view of the same shot and a navigator | TV TB CS | M | M | |
| Undo history panel | TV CS | M | S | |
| Preferences dialog | all | P | S | Settings live in menus; no single dialog |
| Translated interface | all | P | M | craft-i18n catalogs are validated; English is the only shipped language |
| Every action reachable from the keyboard | — | P | M | **Ours:** the accessibility rule; kittest-tested for the main flows |
| Screen reader names for the canvas and the timeline | — | X | S | **Ours:** AccessKit nodes for the canvas and every visible cell |

## Tablet and input

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Pen pressure and tilt on X11 and macOS | all | D | S | Shared photocraft-tablet reader, also under Xwayland |
| Pen pressure and tilt on Windows | all | P | S | Windows Ink touch force only; no tilt |
| Pen input in Wayland-native windows | all | M | M | |
| Pen eraser end switches tool | TV CS | D | S | |
| Pen side buttons mapped to actions | TV CS | M | S | |
| Touch gestures: pinch zoom and twist, taps for undo and redo | CS PD | P | S | Pinch and twist; no tap gestures |
| Palm rejection while the pen is near | CS PD | M | S | |
| Global pressure curve and calibration | TV CS PD | M | S | |
| End-to-end stroke latency measured | — | P | S | Engine latency per sample is measured; input-to-screen is not |
| iPad and Android tablet builds | CS PD | O | XL | Out of scope: the suite targets desktop Linux, macOS and Windows |

## Collaboration and review

Weight: 2

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Save new versions | TV TB | D | S | shot_v002, shot_v003 … next to the file |
| Review notes drawn over frames, kept out of exports | TV TB | M | S | |
| Comments on frames | TB PD | M | M | |
| Review movie with burn-ins (shot, frame, timecode) | TV TB | M | S | |
| Check-out and check-in of shots for several artists | TB | M | L | |
| Vendor cloud sharing and production-tracking services | TB CS PD | O | L | Out of scope: tied to the vendors' own cloud services |

## Stability and performance

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Unlimited undo within a memory budget | all | D | S | |
| Autosave and crash recovery | TV CS PD | D | S | Written from a copy-on-write snapshot on a background thread |
| Crash-safe saving and hostile-file loading | — | X | S | **Ours:** journaled atomic saves, property-tested loader, deny lints on every crate |
| Real-time playback at the shot's frame rate | all | D | S | |
| Playback cache of composited frames | TV TB | D | M | Rendered ahead of the playhead |
| Effects play back in real time | TV TB | P | M | 1080p, 4 effects: 24 fps; 4K drops frames ([performance.md](performance.md)) |
| Large shots stay real time (4K, 240 drawings) | TV TB | D | S | Measured: 24.1 fps, 0 dropped |
| Fast open and save of large shots | all | D | S | 4K 240-drawing shot loads in 0.12 s |
| Memory bounded on very long shots | TV TB | D | M | Drawings page to disk past a memory budget (1 GB by default); 2000-frame 4K shot scrubs at 317 MB peak ([performance.md](performance.md)) |
| Effects rendered on the GPU | TV TB PD | M | L | **Ours:** craft-gpu (RFC-0022) with the CPU path as the oracle |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First map, written before the vertical slice; statuses set when the slice landed |
| 2026-11-17 | major | Polish pass and second pass; rows added for the top gaps and interchange |
| 2026-11-17 | minor | Rows flipped by the slices merged on 2026-11-17 (pegs, effects, compositing, sound, lip-sync, folders, timing, canvas tools, BoardCraft layout, brush library, effects performance) |
| 2026-11-18 | major | Full-product map: 18 areas with weights, S/M/L/XL sizes, X and O statuses; every status re-checked against `main` at `a4ed418`; weighted parity becomes the headline |
