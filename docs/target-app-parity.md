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
| In-scope feature rows | 314 | counted |
| Done / exceeds / partial / missing | 213 / 3 / 30 / 68 | counted |
| Out of scope (not in any denominator) | 4 | counted |
| **Row parity**, (D + X + ½·P) / rows | **73.6 %** | counted |
| **Weighted parity**, area weights × size-weighted rows | **56.1 %** | counted |
| Size-weighted parity over all rows, no area weights | 57.2 % | counted |
| **UI parity**, rows of the reference-layout area | **88.3 %** | counted |

| Area | Weight | Rows | D | X | P | M | Out | Row parity | Effort parity |
|---|---|---|---|---|---|---|---|---|---|
| Brush engine | 9 (9 %) | 29 | 19 | 0 | 2 | 8 | 1 | 69 % | 56 % |
| Paper and textures | 3 (3 %) | 10 | 7 | 0 | 1 | 2 | 0 | 75 % | 77 % |
| Drawing tools | 7 (7 %) | 17 | 12 | 0 | 2 | 3 | 0 | 76 % | 58 % |
| Selection and transform | 5 (5 %) | 14 | 9 | 0 | 1 | 4 | 0 | 68 % | 53 % |
| Colour and paint | 7 (7 %) | 18 | 14 | 0 | 1 | 3 | 0 | 81 % | 60 % |
| Layers | 6 (6 %) | 13 | 10 | 0 | 0 | 3 | 0 | 77 % | 67 % |
| Timeline and exposure sheet | 9 (9 %) | 20 | 16 | 0 | 1 | 3 | 0 | 82 % | 64 % |
| Onion skin and light table | 4 (4 %) | 11 | 8 | 0 | 2 | 1 | 0 | 82 % | 75 % |
| Animation tools | 9 (9 %) | 18 | 9 | 1 | 2 | 6 | 0 | 61 % | 37 % |
| Camera | 4 (4 %) | 8 | 4 | 0 | 0 | 4 | 0 | 50 % | 40 % |
| Effects and compositing | 7 (7 %) | 22 | 13 | 0 | 3 | 6 | 0 | 66 % | 41 % |
| Audio and lip-sync | 4 (4 %) | 14 | 11 | 0 | 1 | 2 | 0 | 82 % | 72 % |
| Import, export and pipeline | 7 (7 %) | 28 | 20 | 0 | 2 | 6 | 1 | 75 % | 62 % |
| Scripting and automation | 4 (4 %) | 7 | 1 | 0 | 2 | 4 | 0 | 29 % | 22 % |
| UI and workspace | 5 (5 %) | 14 | 8 | 1 | 3 | 2 | 0 | 75 % | 67 % |
| UI parity with the reference layout | 5 (5 %) | 47 | 40 | 0 | 3 | 4 | 0 | 88 % | 74 % |
| Tablet and input | 4 (4 %) | 9 | 3 | 0 | 3 | 3 | 1 | 50 % | 45 % |
| Collaboration and review | 2 (2 %) | 5 | 2 | 0 | 0 | 3 | 1 | 40 % | 22 % |
| Stability and performance | 4 (4 %) | 10 | 7 | 1 | 1 | 1 | 0 | 85 % | 69 % |
<!-- parity:end -->

## How to read the map

**Status** per row, checked against the code on `main` (2026-11-18, commit `62978c5`):

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
| UI parity with the reference layout | 5 | How close the screens come to the studio layout in the reference captures |
| Scripting and automation | 4 | Studios automate (TVPaint George, Harmony scripting) |
| UI and workspace | 5 | Workspace quality decides whether people stay |
| Tablet and input | 4 | Pen and touch input quality is felt in every stroke |
| Collaboration and review | 2 | Studio feature; most of it is in Harmony's server |
| Stability and performance | 4 | Everyone depends on it; the references are mature here |

## Readiness by audience

| Audience | Ready? | Why |
|---|---|---|
| Student or hobbyist doing pencil tests and short loops | Preview | Seven brushes with a brush editor, onion skin, light table, holds, sound and MP4 / GIF export work |
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
| Pen barrel rotation as an input | TV CS | D | S | The Pen rotation input drives any dab property through its curve; read from the tablet on X11 (also Xwayland) and macOS, turned into canvas space under a rotated or mirrored view |
| Response curves for every input and dab property | TV CS PD | D | M | A curve for each of five inputs (pressure, tilt, speed, direction, chance) on size, opacity, flow, scatter and angle |
| Random jitter of size, opacity, angle and scatter | TV CS PD | D | S | The chance input on each property, with quick sliders |
| Colour jitter per dab and per stroke (hue, saturation, value) | TV CS PD | D | S | Hue, saturation and brightness jitter, per dab or once per stroke; replays exactly |
| Stroke smoothing (moving average) | all | D | S | |
| Pulled-string stabiliser with catch-up at the end | TV CS PD | D | S | |
| Built-in families: pencil, ink, airbrush, chalk, eraser, watercolour, smudge | all | D | M | Each with its own coverage rule; the airbrush builds up with flow |
| Opacity and flow without darkening where dabs overlap | TV CS | D | S | |
| Image brush tips | TV CS PD | D | S | Tips stored in the shot |
| Tip shape: hardness, spacing, roundness and angle | all | D | S |  |
| Brush presets as files and a preset folder | all | D | S | `.toonbrush` files |
| Brush library: categories, favourites, recent brushes, search | TV CS PD | D | M | **Ours:** every list and the search work from the keyboard (Ctrl+B) |
| Brush editor panel with a live stroke preview | TV CS PD | D | M | **Ours:** every control, the curve editor included, works from the keyboard; [format](toonbrush-format.md) |
| Wet paint: colour pickup, mixing and dilution | TV CS PD | P | L | Watercolour brush and colour pickup on any brush; no dilution or drying |
| Watercolour edges: darkening and bleeding | TV CS PD | M | M | |
| Smudge brush | all | D | M | Drags the layer's colour along the stroke; one undo step |
| Blur and soften brush | TV CS PD | D | S | The Blur and Fine blur brushes (the smudge brush at full pickup): each pixel moves towards its neighbourhood's mean |
| Thick paint with depth and lighting (impasto) | TV CS PD | M | L | |
| Dual tips (a second tip or texture masking the first) | CS PD | M | M | |
| Scatter and spray brushes (many dabs per step) | TV CS PD | D | M | Dabs per step (1 to 32) with position jitter |
| Pattern and ribbon brushes (an image repeated along the stroke) | TV CS | M | M | |
| Animated brushes (a tip that changes per dab or per frame) | TV CS | M | M | |
| Brush blend modes (paint as multiply, behind, and so on) | TV CS PD | D | S | Behind, only on paint, multiply, screen, overlay, darken, lighten and add |
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
| Paper and grain library, grain images from files | TV CS PD | D | M | Four built-in grains and tileable grain images saved in the shot |
| Grain scale, rotation and contrast | TV CS | P | S | Scale only |
| Texture overlay on a layer or a fill | CS PD | M | S | |
| Canvas size presets | all | D | S | Eleven video, cinema, square and portrait presets with their frame rates in New Shot and Canvas Size, the aspect ratio shown, and the user's own saved presets |
| Canvas resize and crop with anchors | all | D | S | Drawings move whole pixels, never resampled; one undo step |
| Large canvases (up to 16384 px a side) | TV | D | S | Sparse tiles: empty space costs nothing |
| Field guide and safe areas | TV TB | D | S | A 12-field guide, action-safe and title-safe frames |

## Drawing tools

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Line, rectangle and ellipse with constraints | all | D | S | Shift: 15° lines, squares, circles |
| Polyline, polygon and curve tools | all | D | S | Shapes tool: click to place points, Backspace removes one, Enter or double-click finishes; curves pass through every point |
| Filled shapes and lasso fill | all | D | S | Outline, filled or both for rectangles, ellipses, polygons and closed curves; the paint bucket's lasso fill; one undo step each |
| Eraser | all | D | S | |
| Eyedropper | all | D | S | |
| Gradient tool (linear, radial) | all | D | S | Gradient tool (G): drag start to end, to transparent or to white, kept inside the selection |
| Symmetry and mirror drawing (axes, radial) | TV CS PD | D | M | Left and right, top and bottom, four ways, radial and mirrored radial (2 to 32 copies) with the axes drawn on the canvas; a mirrored stroke is one undo step and the setting is saved in the shot |
| Stroke rulers: straight, curve, ellipse, concentric | TV TB CS | D | M | Rulers above the canvas: a stroke started near a straight, curve or ellipse ruler follows it, a concentric ruler takes every stroke on the ellipse through its start; handles drag on the canvas or are typed in the Rulers menu; saved with the shot |
| Perspective rulers with vanishing points | TV CS PD | D | M | One, two or three vanishing points; after a few pixels of travel the stroke runs on the line through the closest point in that direction |
| Grid display and snapping strokes to it | TV TB CS | D | S | The field guide draws a grid; shape corners, polyline points and gradient ends snap to it, and with Rulers ▸ Snap strokes to grid lines a freehand stroke started near a grid line runs along it |
| Snap the end of a stroke to a clean line or ellipse when held | CS PD | D | S | Hold to straighten: hold the pen still for half a second and a rough line or loop becomes a clean line or ellipse |
| Text tool | TV TB CS | D | M | Text tool (T): click or Enter at the keyboard pen places a box, type, Ctrl+Enter paints it as one undo step; font, size, alignment, line and letter spacing; shaped through craft-font-catalog, so kerning and joined scripts follow the font; `text` command for scripts (`tooncraft-cli run --font`). Text is pixels once placed |
| Clone stamp | TV CS | M | S | |
| Reference image viewer with eyedropper | TV CS PD | M | S | |
| Vector stroke layers (strokes kept as editable curves) | TB CS | P | XL | The Layers panel's Vector lines switch: pen strokes on that layer are kept as centre lines with a width per point (`tooncraft-vector`), painted into the drawing's tiles so fill, onion skin and export read them; saved in the shot. Not yet: shapes, symmetry copies, duplicate drawing keeping the lines, a vector view of the line |
| Vector line editing: reshape, width, erase to the intersection | TB CS | P | L | Commands move a point, set a width, smooth, delete, recolour and cut a line back to its crossings (one undo step each); the eraser on a vector layer cuts the line it touches back to its crossings. No point-dragging tool on the canvas yet |
| 3D reference models and posable figures | TB CS | M | XL | |

## Selection and transform

Weight: 5

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Lasso and rectangle selection | all | D | S | |
| Ellipse selection | all | D | S | The select tool's Ellipse mode |
| Select by colour with tolerance (magic wand) | all | D | S | Magic Wand mode: tolerance, this drawing or all visible layers, the touching area or that colour everywhere |
| Add, subtract and intersect selections | all | D | S | A mode option, or Shift (add), Alt (subtract) and both (intersect) while dragging; holes and separate parts stay one selection |
| Invert, grow, shrink and feather a selection | all | P | S | Invert (Ctrl+Shift+I), grow and shrink by up to 64 px; no feather (selections are hard-edged) |
| Painted selection masks (quick mask) | TV CS PD | M | M | |
| Move, scale and rotate a selection with a live preview | all | D | S | One undo step |
| Flips and 90° turns | all | D | S | |
| Cut, copy and paste pixels between drawings | all | D | S | |
| Free distort, perspective and skew | all | D | S | Distort: drag four corner handles (perspective), then Apply; skew by degrees; each one undo step |
| Mesh warp | TV CS PD | M | M | |
| Liquify (push, twirl, pinch) | TV CS PD | M | M | |
| Transform across several drawings or layers at once | TV TB CS | M | M | |
| Resampling choice (nearest, bilinear, bicubic) | TV CS | D | S | For moving, scaling, turning, skewing and distorting a selection |

## Colour and paint

Weight: 7

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Flood fill with tolerance | all | D | S | |
| Close gaps of a set size | all | D | S | |
| Fill reference: current layer or all visible layers | TV TB CS | D | S | |
| Fill under lines (into the antialiasing) | TV TB CS | D | S | |
| Fill every drawing of a layer at once | TV TB | D | S | One undo step. Also: fill, empty areas only, or repaint, and gap presets with the reach shown around the pointer |
| Linked line and paint layers | TB TV CS | D | M | Add Colour Layer (a layer's menu): a layer below that follows the line layer's exposures, so new drawings, holds, re-timing and deleted frames act on both; painting there makes the colour drawing paired with the line drawing, and the bucket sees the lines through the all-visible reference; saved with the shot. No line-only onion and no automatic copy of line regions |
| Paint the same regions across drawings automatically | TV TB | M | L | |
| Colour swatches | all | D | S | Named palettes kept with the shot, plus the built-in Basic one; swatch names on hover; a swatch's right-click menu (also Shift+F10) uses, edits, renames, duplicates or deletes it, or selects the pixels of its colour |
| Palette files: save, load, import common formats | all | D | S | GIMP .gpl open and save (read through craft-palette), Photoshop .aco open (RGB, HSB, grey) |
| Linked colour styles (change a swatch, every fill using it updates) | TB CS | P | L | Edit Swatch: the swatch's paint on every unlocked layer takes the new colour (matched by colour at each pixel's opacity, so soft edges follow), one undo step; paint is not linked to the swatch, so unrelated paint in exactly that colour changes too |
| HSV picker with keyboard fields | all | D | S | Hue, saturation and brightness typed from the keyboard, kittest-tested |
| Colour wheel, sliders and harmony modes | TV CS PD | D | S | A hue wheel, and opposite, neighbour, three-way, split-opposite and four-way harmonies to pick from or add to a palette |
| Recent colours | TV CS PD | D | S | The last 16 colours painted with, newest first, in the Colour panel |
| Lock transparency (paint only where there is paint) | all | D | S | Per layer; brushes (also wet ones) only recolour paint, fills are refused |
| Paint behind existing paint | TV CS | D | S | Brush option "Behind" (not for wet brushes); the fill already paints under |
| Recolour line art across drawings | TV TB CS | D | M | Recolour Layer: one colour becomes another in every drawing of the layer, edges keep their softness, one undo step |
| Colour management: working space, profiles, soft proof | TV CS | M | M | |
| 16-bit colour drawings | TV CS | M | L | |

## Layers

Weight: 6

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Animation layers with their own exposures | all | D | S | |
| Add, delete, rename and reorder layers | all | D | S | |
| Layer visibility, lock and opacity | all | D | S | |
| Duplicate a layer | all | D | S | With private copies of its drawings, one undo step |
| Merge down, merge visible and flatten | all | D | S | Rendered through the compositor, so blend, opacity keys, pegs, mattes and effects are baked in |
| Blend modes | all | D | S | 24 modes: the separable ones and hue, saturation, colour, luminosity (W3C formulas) |
| Nested folders with show, lock, fold and opacity | all | D | S | |
| Clipping layers | TV CS PD | D | S | Several clipping layers on one base |
| Colour labels with a layer filter | TV TB | D | S | |
| Painted layer masks | TV TB CS PD | M | M | |
| Adjustment layers acting on everything below | CS PD | M | M | Effects act on their own layer only |
| Reference layers chosen per tool | TB CS | M | S | The fill reference is current or all visible |
| Layer properties copied between layers | TV CS | D | S | Opacity and keys, blend, colour, folder, peg, depth, effects |

## Timeline and exposure sheet

Weight: 9

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Timeline grid with keys, holds and blanks | all | D | S | |
| Holds: a drawing exposed several frames | all | D | S | |
| Insert and delete frames, in one or all layers | all | D | S | |
| New blank drawing and duplicate drawing | all | D | S | |
| Extend and shorten a hold from the keyboard | TV TB | D | S | Also: keys 1 to 9 in the sheet set the hold, a Hold field in the transport, and Fill Empty Frames |
| Change a hold by dragging its cells | TB CS PD | D | S | |
| Select a range of cells across layers | all | D | S | |
| Cut, copy and paste cells | all | D | S | **Ours:** paste linked or as new drawings, in place or inserting |
| Thumbnails in timeline cells | TV PD | D | S | |
| Drawings kept when no longer exposed | TB | D | S | Purging them is undoable |
| Drawing substitution: expose any existing drawing by name | TV TB CS | D | S | Name a drawing, then show it at any frame of any layer by name; only that frame changes. Also a thumbnail grid of the layer's drawings (a cell's right-click menu, Frame ▸ Swap Drawing) that swaps the whole exposure; Ctrl+Alt+Up and Down swap to the previous or next drawing from the keyboard |
| Vertical exposure sheet view | TV TB | D | L | The timeline's Vertical switch, or the Exposure Sheet layout: layers as columns, frames down the side, the same keys, cell names and right-click menus |
| Frame markers and notes | TV TB | D | S | A note per frame: ruler flags, a list with timecodes, next / previous by keyboard, saved in the shot |
| Play range, loop and ping-pong | all | D | S | Loop, ping-pong, and "Play Selection" for the selected frames; the in and out range shows on the ruler with a handle at each end to drag (arrow keys with a handle focused) |
| Frame rate change and timecode display | all | D | S | Frame rate per shot; the position and the marker list show HH:MM:SS:FF (non-drop-frame) |
| Timeline zoom and scroll | all | D | S | Zoom buttons, Alt+= and Alt+-, Ctrl and the wheel or a pinch; scroll as before |
| Graph editor for keyed values | TB CS PD | P | L | The Curves panel plots each peg value against frames: drag keys in time and value (one undo step, Escape cancels), move them from the keyboard, ease many keys at once, drag the two ease handles. Camera, opacity and effect keys still edit in their own panels |
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
| Onion by drawings or by frames | TV TB | D | S | The reach shows in the before and after tints on the timeline ruler; right-click the Onion Skin toggle above the canvas for quick settings |
| Onion on the current layer or on all layers | TV CS | D | S | "Onion skin on every visible layer" in the Onion Skin panel and popover; hidden layers and layers left out of the onion skin stay out (tested on the CPU render) |
| Onion skin as outlines | TV | D | S | Also: hide only the step before |
| Onion settings per layer | TV TB | D | S | Own settings for a layer in the Onion Skin panel (steps, opacity, colours, outlines); an onion toggle in each timeline row leaves a layer out or takes it back; the shot's switch still turns all off |
| Pick the exact relative frames shown | TB CS | D | S | "Exact frames" in the Onion Skin panel: type the offsets (`-4 -2 +2`, up to 16, ±99 frames); also an opacity per step on each side and key drawings only; the ruler shows the picked reach |
| Light table: pin any frame of any layer, with opacity | TB CS | D | S | |
| Shift and trace: move or turn a pinned frame without changing it | TB TV | D | M | Each light-table pin in the Onion Skin panel has Across, Down and Turn fields and Reset Shift; the drawing is untouched (`shift_pin` command, saved in the shot, `--shift-trace`) |

## Animation tools

Weight: 9

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Key, breakdown and inbetween marks with a spacing chart | TV TB | D | S | A dot in the mark's colour on the drawing's cell, named in the cell for screen readers |
| Flip and scrub through drawings | all | D | S | |
| Rolling and flipping between chosen drawings from the keyboard | TV TB | D | S | Choose frames with Ctrl+Alt+Enter, flip round them with Ctrl+Alt+Left and Right (hold to roll); dots on the ruler |
| Re-time a range, with presets and re-timing between keys | TV TB | D | S | **Ours:** non-destructive, one undo step |
| Reverse a range | TV TB | D | S | |
| Cycles (repeat a range of cells) | TV TB | D | S | |
| Pegs: nested parented transforms with pivots | TB | D | L | Move, rotate, scale and skew keys; loops refused; dragged on the canvas with the Transform tool, through any parent pegs |
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
| Line thickening and smoothing | TV | D | S | Thicken and thin, and Smooth lines (jagged edges rebuilt smooth) |
| Gradient map, posterise and threshold | TV TB PD | D | S | Gradient map (dark to light colour), posterise (2 to 64 levels), black and white by brightness, and the alpha threshold |
| Motion blur from movement | TV TB | M | M | |
| Distortions: ripple, wave, turbulence, displacement | TV TB | P | M | Ripple, wave and turbulence, animated over frames; no displacement by another layer |
| Lens looks: vignette, chromatic aberration, flares | TV TB PD | P | S | Vignette and colour fringe; no flares |
| Tone and highlight from mattes, lighting from normals | TV TB | M | L | |
| Particles (rain, snow, sparks) | TV TB | M | L | |
| Time effects: trails, frame blending, time remap | TV TB PD | M | M | |
| Transitions between cuts | TV TB PD | M | S | |
| Effect presets and saved stacks | TV TB | D | S | Five built-in looks added as one undo step; any layer's stack saved to and opened from a .toonfx file |
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
| Split and duplicate clips | TB CS PD | D | S | Split a clip at the current frame, or copy it to start there; one undo step each |
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
| Animated PNG and WebP export | TV CS | D | S | Animated PNG; animated and still WebP, lossless or lossy, with alpha |
| MP4 (H.264) export | all | D | M | Through craft-video and craft-media; no external tools |
| ProRes with alpha | TV TB | D | M | 4444 with alpha, 422 HQ |
| Video export carries the soundtrack | all | D | S | |
| Per-layer PNG sequences | TV TB | D | S | |
| Layered PSD of a frame | TV CS | D | S | Through craft-psd |
| Multi-frame layered PSD and PSB | TV CS | D | S | One file with a group per frame (PSD, or PSB for large canvases) |
| TGA, TIFF and JPEG sequences | TV TB CS | D | S | TGA, TIFF and JPEG sequences through the Export dialog |
| OpenEXR sequences, with layers | TV TB | P | M | Flat RGBA OpenEXR sequences through craft-image-codecs; no layers in one file |
| More video codecs: H.265, VP9 or AV1, MOV H.264 | TV CS PD | M | M | |
| OpenRaster frame export | TV | D | S | |
| Transparent-background export | all | D | S | PNG, TGA, TIFF, OpenEXR, animated PNG and WebP, ProRes 4444 and OpenRaster without the paper; tested on PNG |
| Export frame range and output size | all | D | S | One Export dialog: whole shot, selected frames, this frame or from–to; full, half or quarter size; GIF loop and sequence digits; remembered per shot |
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
| Shot list and edit decisions out (EDL or OTIO) | TB | D | S | OpenTimelineIO: a track per layer, a clip per exposure, gaps and markers |
| Asset library shared between shots | TB CS | P | M | Drawings with their timing, through My Drawings (`.toondrawings` files in the user's folder, or anywhere); no shared sounds, templates or rigs |
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
| Dockable panels: move, tab, float | TV TB CS | D | M | Through craft-ui docking: drag a tab onto another stack to tab it or to a panel's side to split, float from the stack menu and move the window by its title, Move To and Reset Panel from the keyboard; the canvas stays docked and a panel dropped on it floats there (`dock_ui_tests`) |
| Saved workspaces | TV TB CS | D | S | Draw, Time, Ink & Paint and Sheet keep their own edits per user (Reset Workspace puts one back); Save Workspace As… keeps named layouts |
| Keyboard shortcuts for every tool | all | D | S | |
| Customisable shortcuts | all | D | S | Through craft-keymap, saved per user |
| Command search | TB CS | D | S | Find Command (Ctrl+Shift+P): by name or by key, keyboard or click, runs like the shortcut |
| Custom tool palettes and radial menus | TV CS PD | D | S | The Tool Palette panel (`--panel palette`): named palettes of any registry commands as buttons, put together from the keyboard (add by search, reorder, remove, rename, delete), kept in the preferences; the radial menu (V, `--open radial`) rings a chosen palette's first 12 commands round the pointer, picked by pointer or arrows and Enter (`tool_palettes_ui_tests`) |
| Second view of the same shot and a navigator | TV TB CS | M | M | |
| Undo history panel | TV CS | D | S | A panel listing every step; click one to go there (undo or redo as needed) |
| Preferences dialog | all | D | S | Edit ▸ Preferences: general, drawing, onion skin, sheet and export tabs, each with Restore Defaults; a damaged settings file gives the defaults |
| Translated interface | all | P | M | craft-i18n catalogs are validated; English is the only shipped language |
| Every action reachable from the keyboard | — | P | M | **Ours:** the accessibility rule; kittest-tested for the main flows |
| Screen reader names for the canvas and the timeline | — | X | S | **Ours:** AccessKit nodes for the canvas and every visible cell |

## UI parity with the reference layout

Weight: 5

How close ToonCraft's screens come to the studio layout studied in the Harmony reference captures
(`_reference/harmony`, local only): the rows follow that suite's screens, in our own words. Each
status was checked in the code and a test; a launch option (`--panel`, `--open`, `--layout`,
`--vertical`, `--tool`, with `--screenshot`) shows the state for a side-by-side comparison where one
exists. `cargo xtask parity` reports this area's row parity as the UI parity.

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Main window: tools left, canvas centre, sheet right, colour and properties at the side, timeline below | TB | P | M | Same family of layout; tool settings sit in a bar above the canvas, not in a properties panel |
| Start screen with recent shots and a new-shot form | TB | D | S | With no shot to open: a new-shot form with size presets, the recent shots (missing ones marked), Open… and the demo shot; waits for a recovery offer to be answered; turn it off in Preferences (`--open start` shows it) |
| Named workspaces picked from a bar | TB | D | S | Segments in the top bar (Draw, Time, Ink & Paint, Sheet; right-click one to reset it or save it as a named workspace); the same and the saved ones in Window ▸ Workspace (`--layout`) |
| Compositing workspace with a node view | TB | M | XL | No node view |
| Scripting workspace with a script editor and log | TB | M | L | No scripting panels |
| Panels that can be closed and added back | TB | D | S | Every panel but the canvas closes from its tab; the Window menu puts it back where it was (`panels_ui_tests`) |
| Many tabs in one stack stay readable | TB | D | S | Every tab shows its panel's glyph and name; a stack narrower than 220 px, or too crowded for whole names, shows glyphs only, each naming its panel on hover and to screen readers |
| Dark theme and a light theme | TB | D | S | Contrast tested |
| Status line: zoom, drawing, tool, frame, pointer position, colour | TB | D | S | Zoom and rotation, tool and layer, drawing, frame, colour, the canvas pixel under the pointer; notices on the right fade after a few seconds (a polite live region) |
| Top menus for play, drawing, scene and windows | TB | D | M | File, Edit, Frame, Layer, Drawing, Play, Scene, View, Window, Help |
| File menu with recent files | TB | D | S | File ▸ Open Recent: the last 12 shots opened or saved, newest first, missing ones marked, Clear Recent Shots |
| Edit menu with selection and transform items | TB | D | S | Undo and redo with names, Preferences, select, invert, grow and shrink, transform submenu |
| View menu: zoom, rotate, mirror, onion, light table, guides, workspaces | TB | D | S | Workspaces are in Window ▸ Workspace |
| Play commands with speed | TB | D | S | Frame menu and the Timeline header: play, step by frame and by drawing, in and out fields, speed, loop, Play Around with its reach |
| A Windows menu listing every panel | TB | D | S | Window: Workspace, every panel with a tick when open, and Reset Panels |
| Tool bar with flyouts for related tools | TB | D | S | The tool rail: select (lasso, rectangle, ellipse, wand), transform (selection, layer peg), draw, smudge, erase, fill, shapes, text, picker and, at the bottom, view (hand, zoom, rotate view); a corner mark on groups; hold, right-click, Menu or Shift+F10 opens the flyout with icon, name and key; the last-used variant is remembered in the preferences; colour chips swap back to the previous colour |
| Top bar with file buttons, undo, workspace picker, transport and frame fields | TB | D | M | One 32 px bar: the menus, the workspace segments, command search and undo/redo; New, Open and Save are in the File menu and on their keys; the transport and the frame, in, out, frame rate and hold fields are in the Timeline header (shot length in its ⋯ menu) |
| Canvas bar: onion, light table, guides, mirror toggles | TB | D | S | Icon toggles above the canvas (onion, light table, field guide, safe areas, mirror, symmetry, rulers) with zoom and rotation; right-click or Shift+F10 on a toggle opens its settings, for onion a slider per step from 5 before to 5 after |
| Brush properties: preset list with previews | TB | D | S | The tool options bar: the tool's glyph and name (its group's tools), a preset dropdown showing the current brush as a stroke and every preset as a stroke, Revert; size, opacity and smoothing as scrub fields (drag the name or bar, or type) |
| Brush editor | TB | D | S | The Brush panel, every control keyboard-reachable |
| Pencil properties: caps and joins | TB | M | S | Size, opacity, smoothing, steady line; no caps or joins |
| Paint bucket properties: modes and gap buttons | TB | D | S | Fill, empty only, repaint; gap presets with a reach ring |
| Select properties: modes, typed position, size and angle | TB | D | S | Lasso, rectangle, ellipse, wand; add, remove, overlap; X, Y, width, height (proportions kept) and angle typed and applied as one step; flips, skew, distort |
| Transform tool properties with typed values | TB | D | M | The Transform tool (Q) drags the current layer's peg on the canvas: move, a corner to scale, a knob to turn, the pivot cross; one peg key per drag, Escape cancels; arrows, [ ] and Page Up / Down nudge; its options type Move X/Y, Rotation, Scale X/Y, or add a peg for a layer without one |
| Contour, text, rigging and morph tool properties | TB | P | L | Text tool properties: font, size, alignment, line and letter spacing; the contour, rigging and morph tools are missing |
| Colour view: palette list, named swatches | TB | D | S | Palettes kept with the shot, named swatches, current and previous colour |
| Colour sliders with opacity | TB | D | S | Hue, saturation, brightness, red, green, blue, the colour's own opacity and the brush's opacity |
| Picker with alpha and a gradient | TB | P | S | A floating Colour Picker window (`--open colour-picker`) as well as the docked one; the colour has its own opacity; no gradient swatches |
| Timeline layer rows with toggles | TB | D | S | Show, lock and onion toggles in each row on the shared icons (named check boxes, keyboard reachable) and a bar in the label colour; a mark on every cell where the layer's pegs are keyed, named "peg key" for screen readers; the Layers panel rows have a thumbnail of the drawing on this frame and a split add button; the rest in the row's right-click menu |
| Timeline buttons | TB | D | S | One header row: icon transport, loop, Play Around, speed and inline fields; the rest in its ⋯ menu |
| Selected range highlight | TB | D | S | |
| Right-click menu on timeline cells | TB | D | S | Swap drawing, hold for 1 to 8, marks, new and duplicate drawing, insert and remove frames, fill empty frames, cut, copy, paste and paste reversed, go to; every item with its icon and key, disabled ones saying why; Shift+F10 or the Menu key opens it |
| Right-click menu on exposure-sheet cells | TB | D | S | The same menu in the vertical sheet |
| Panel menus (a ☰ button per panel) | TB | D | S | A `⋯` button at the end of every tab strip: the shown panel's own commands (timeline orientation, colour wheel and picker window, onion on/off), then Float, Move To and Close (`shell_chrome_ui_tests`) |
| Vertical exposure sheet | TB | D | M | Layers as columns, frames down the side, the same keys and menus (`--vertical`, `--layout sheet`) |
| Onion skin on the canvas, tinted before and after | TB | D | S | |
| Onion panel with a slider per step | TB | D | S | Also in the onion toggle's popover above the canvas |
| Light table on the canvas | TB | D | S | |
| Camera frame and safe areas | TB | D | S | |
| Separate drawing, reference and perspective views | TB | M | M | One canvas |
| Curve editor | TB | D | L | The Curves panel beside the timeline: value buttons, the graph with keys, other values faintly behind, an ease box with handles and typed values (`--panel curves`) |
| Layer properties window | TB | D | S | Inline in the Layers panel and the layer right-click menu |
| Library window for drawings | TB | D | L | The Drawing Library panel: the shot's reusable drawings with thumbnails, names and lengths; add the selected frames, insert as copies or linked, rename, reorder, remove; My Drawings keeps items in the user's folder for every shot |
| Preferences, shortcut editor with search, playback speed | TB | D | S | (`--open preferences`, `--open shortcuts`) |
| Scene settings in one window | TB | D | S | Scene ▸ Scene Settings: size with presets, frame rate, length and duration, loop, ping-pong and speed (`--open scene`) |
| Export dialog with range, size and options | TB | D | S | (`--open export`) |
| Import dialog with options | TB | D | S | After picking images: a new layer, one layer per file name, or the current layer; hold; fit, actual size or fill; premultiplied colours |

## Tablet and input

Weight: 4

| Feature | Refs | Status | Size | Notes |
|---|---|---|---|---|
| Pen pressure and tilt on X11 and macOS | all | D | S | Shared photocraft-tablet reader, also under Xwayland |
| Pen pressure and tilt on Windows | all | P | S | Windows Ink touch force only; no tilt |
| Pen input in Wayland-native windows | all | M | M | |
| Pen eraser end switches tool | TV CS | D | S | |
| Pen side buttons mapped to actions | TV CS | M | S | |
| Touch gestures: pinch zoom and twist, taps for undo and redo | CS PD | D | S | Pinch and twist; a two-finger tap undoes and a three-finger tap redoes (the stroke the first finger began is dropped) |
| Palm rejection while the pen is near | CS PD | P | S | Touches are ignored while the pen is seen and for 1.2 s after; the pen is seen through the tablet reader (X11, Xwayland, macOS), so not yet on Windows or Wayland-native windows |
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
| Review movie with burn-ins (shot, frame, timecode) | TV TB | D | S | An Export dialog option for every frame format and MP4: shot name, frame of total and timecode in a bar along the bottom |
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
| 2026-11-18 | major | Full-product map: 18 areas with weights, S/M/L/XL sizes, X and O statuses; every status re-checked against `main` at `62978c5`; weighted parity becomes the headline |
