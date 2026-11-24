# Where ToonCraft falls short of the references

> **Last reviewed:** 2026-11-18 · **Last updated:** 2026-11-18 · **Change:** major (ranked plan toward 90 % parity) · **Target:** see [target-app-parity.md](target-app-parity.md)

This is the work list toward the program goal of **90 % weighted parity** with TVPaint Animation,
Toon Boom Harmony, Clip Studio Paint EX and Procreate Dreams. Every item is a group of rows from
the [parity map](target-app-parity.md); when an item lands, flip its rows there (the generated
block is refreshed separately with `cargo xtask parity --write`), and delete or shrink the item
here (the progress log records what landed).

## Where we stand

| | Value | Kind |
|---|---|---|
| Weighted parity today | 39.0 % | counted by `cargo xtask parity` |
| Row parity today | 51.7 % of 267 in-scope rows | counted |
| Weighted parity once the open PR #10 merges as described | about 39.3 % | counted from the rows they say they flip, not yet merged |
| Hours to 90 % weighted parity, in the order below | ≈ 965 | estimated from row sizes |
| Hours to 100 % | ≈ 1,335 | estimated from row sizes |

**How the numbers are made.** *Hours* come from the row sizes in the map (S ≈ 3 h, M ≈ 8 h,
L ≈ 25 h, XL ≈ 60 h of agent time; a partial row counts half). *Gain* is the rise in weighted
parity, in percentage points, if every row of the item becomes D; it is computed with the same
formula `cargo xtask parity` uses, not guessed. The plan orders items by gain per hour, taking an
item only once the items it needs are done; *Parity after* and *Hours so far* are cumulative along
that order. Every P and M row of the map is in exactly one item.

**What it means.** 90 % is reached after 44 of the 51 items (≈ 965 hours). The seven that can
stay undone at 90 % are A1 generated inbetweens, B4 GPU brushes, E3 lighting and particles, A3
deformers, D4 vector stroke layers, D5 3D reference models and E4 the node compositing graph
(≈ 370 hours together): large features with a small share of their area's weight. The cheapest
points are breadth in areas that already work (layer operations, selections, onion and timeline
polish, pen input, palettes); the large remaining blocks are cut-out rigs (A2), ink-and-paint
(C3), brush depth (B3), shots and nesting (T3) and the camera (K1). The hours are effort, not
calendar time; the workstreams are independent enough to run in parallel.

## Open pull requests and the items they cover

Checked with `gh pr list -R XmYx/tooncraft` on 2026-11-18 (#17 paging to disk and #7 the brush editor merged while this map was written; their rows are flipped here). The remaining one was written against the
previous map, so after this map lands it needs a `git merge origin/main` and its row flips redone
against the new rows.

| PR | Covers | Rows it should flip in the new map |
|---|---|---|
| #10 Export breadth | most of **F1** | Animated PNG and WebP → D; multi-frame PSD / PSB → D; TGA, TIFF and JPEG sequences → P (no JPEG) |
| #1 Dependency bump | none | — |

## By workstream

| Workstream | Items | Rows | Hours | Gain (points) | Gain per 100 h |
|---|---|---|---|---|---|
| [Timeline and animation](#timeline-and-animation) | 10 | 27 | 364 | 15.5 | 4.3 |
| [Drawing tools](#drawing-tools) | 7 | 24 | 245 | 10.1 | 4.1 |
| [Colour and layers](#colour-and-layers) | 6 | 20 | 142.5 | 8.8 | 6.2 |
| [Pipeline](#pipeline) | 9 | 24 | 195 | 8.7 | 4.4 |
| [Effects and sound](#effects-and-sound) | 7 | 18 | 199.5 | 7.3 | 3.7 |
| [Brushes](#brushes) | 6 | 17 | 132.5 | 5.5 | 4.1 |
| [UI and input](#ui-and-input) | 6 | 16 | 55 | 5.1 | 9.3 |

## Top 10 by gain per hour

| Rank | Item | Workstream | Hours | Gain (points) | Gain per 100 h | Open PR |
|---|---|---|---|---|---|---|
| 1 | **I3** Input-to-screen latency measured | UI and input | 1.5 | 0.20 | 13.3 | — |
| 2 | **I2** Touch: tap gestures and palm rejection | UI and input | 4.5 | 0.60 | 13.3 | — |
| 3 | **L1** Layer operations: duplicate, merge and flatten, the full blend-mode set, copy properties, reference layers | Colour and layers | 13.5 | 1.80 | 13.3 | — |
| 4 | **I1** Pen everywhere: Windows tilt, Wayland-native pen, side buttons, barrel rotation, pressure curve | UI and input | 18.5 | 1.98 | 10.7 | — |
| 5 | **L2** Painted layer masks and adjustment layers | Colour and layers | 16 | 1.60 | 10.0 | — |
| 6 | **O1** Onion depth: all-layer test, per-layer settings, exact frames, shift and trace, rolling flipping | Timeline and animation | 18.5 | 1.70 | 9.2 | — |
| 7 | **B6** Paper: visible paper, grain controls, texture overlay, canvas presets | Brushes | 9 | 0.82 | 9.1 | — |
| 8 | **T1** Timeline polish: substitution by name, frame markers, ping-pong, timecode, timeline zoom | Timeline and animation | 9 | 0.82 | 9.1 | — |
| 9 | **S1** Selection depth: ellipse, magic wand, add/subtract/intersect, invert/grow/feather, quick mask, resampling choice | Drawing tools | 21.5 | 1.81 | 8.4 | — |
| 10 | **X1** UI polish: command search, tool palettes and radial menus, undo history panel, preferences dialog, docking tests, keyboard reach | UI and input | 18.5 | 1.53 | 8.3 | — |

## Ranked plan

| # | Item | Workstream | Rows | Hours | Gain (points) | Gain per 100 h | Needs | Parity after | Hours so far |
|---|---|---|---|---|---|---|---|---|---|
| 1 | **I3** Input-to-screen latency measured | UI and input | 1 | 1.5 | 0.20 | 13.3 | — | 39.2 % | 1.5 |
| 2 | **I2** Touch: tap gestures and palm rejection | UI and input | 2 | 4.5 | 0.60 | 13.3 | — | 39.8 % | 6 |
| 3 | **L1** Layer operations: duplicate, merge and flatten, the full blend-mode set, copy properties, reference layers | Colour and layers | 5 | 13.5 | 1.80 | 13.3 | — | 41.6 % | 19.5 |
| 4 | **I1** Pen everywhere: Windows tilt, Wayland-native pen, side buttons, barrel rotation, pressure curve | UI and input | 5 | 18.5 | 1.98 | 10.7 | — | 43.6 % | 38 |
| 5 | **L2** Painted layer masks and adjustment layers | Colour and layers | 2 | 16 | 1.60 | 10.0 | — | 45.2 % | 54 |
| 6 | **O1** Onion depth: all-layer test, per-layer settings, exact frames, shift and trace, rolling flipping | Timeline and animation | 5 | 18.5 | 1.70 | 9.2 | — | 46.9 % | 72.5 |
| 7 | **B6** Paper: visible paper, grain controls, texture overlay, canvas presets | Brushes | 4 | 9 | 0.82 | 9.1 | — | 47.7 % | 81.5 |
| 8 | **T1** Timeline polish: substitution by name, frame markers, ping-pong, timecode, timeline zoom | Timeline and animation | 5 | 9 | 0.82 | 9.1 | — | 48.5 % | 90.5 |
| 9 | **S1** Selection depth: ellipse, magic wand, add/subtract/intersect, invert/grow/feather, quick mask, resampling choice | Drawing tools | 6 | 21.5 | 1.81 | 8.4 | — | 50.3 % | 112 |
| 10 | **X1** UI polish: command search, tool palettes and radial menus, undo history panel, preferences dialog, docking tests, keyboard reach | UI and input | 6 | 18.5 | 1.53 | 8.3 | — | 51.8 % | 130.5 |
| 11 | **C1** Palettes: named palettes in the shot, palette files, colour wheel and sliders, recent colours, picker test | Colour and layers | 5 | 12 | 0.93 | 7.8 | — | 52.8 % | 142.5 |
| 12 | **C2** Lock transparency and paint behind | Colour and layers | 2 | 6 | 0.47 | 7.8 | — | 53.2 % | 148.5 |
| 13 | **S2** Transform depth: free distort and perspective, mesh warp, liquify, transform many drawings at once | Drawing tools | 4 | 27 | 1.94 | 7.2 | — | 55.2 % | 175.5 |
| 14 | **X3** Shipped translations | UI and input | 1 | 4 | 0.28 | 6.9 | — | 55.5 % | 179.5 |
| 15 | **X2** Second view of the shot and a navigator | UI and input | 1 | 8 | 0.56 | 6.9 | — | 56.0 % | 187.5 |
| 16 | **R1** Review: drawn review notes, frame comments, review movie with burn-ins | Pipeline | 3 | 14 | 0.89 | 6.3 | — | 56.9 % | 201.5 |
| 17 | **G1** Control channel over a socket and an MCP server | Pipeline | 1 | 8 | 0.50 | 6.2 | — | 57.4 % | 209.5 |
| 18 | **D1** Shape tools II: polyline and curve, filled shapes and lasso fill, gradient, hold-to-snap, clone stamp, reference viewer, grid snapping | Drawing tools | 7 | 19.5 | 1.20 | 6.1 | — | 58.6 % | 229 |
| 19 | **U1** Audio: split and duplicate clips, microphone recording | Effects and sound | 2 | 11 | 0.67 | 6.1 | — | 59.3 % | 240 |
| 20 | **B1** Colour jitter per dab and per stroke | Brushes | 1 | 3 | 0.18 | 6.0 | — | 59.4 % | 243 |
| 21 | **E1** Effects finishing: drawn masks, line smoothing, gradient map and posterise, lens looks, transitions, presets | Effects and sound | 6 | 13.5 | 0.81 | 6.0 | — | 60.3 % | 256.5 |
| 22 | **F1** Export breadth: WebP, multi-frame PSD/PSB, TGA/TIFF/JPEG sequences, transparent-background test, output scaling | Pipeline | 5 | 10.5 | 0.61 | 5.8 | — | 60.9 % | 267 |
| 23 | **U2** Lip-sync: tune on recorded speech, lip-sync from the typed line | Effects and sound | 2 | 12 | 0.67 | 5.6 | — | 61.5 % | 279 |
| 24 | **K1** Camera depth: path editing, depth of field, several cameras and cuts, free 3D camera | Timeline and animation | 4 | 44 | 2.40 | 5.5 | — | 63.9 % | 323 |
| 25 | **T4** Graph editor for keyed values and live transform recording | Timeline and animation | 2 | 33 | 1.64 | 5.0 | — | 65.6 % | 356 |
| 26 | **G2** Embedded scripting with an object API, macro recording, batch runs, script tool buttons | Pipeline | 4 | 33.5 | 1.62 | 4.9 | — | 67.2 % | 389.5 |
| 27 | **D2** Drawing guides: symmetry, stroke rulers, perspective rulers | Drawing tools | 3 | 24 | 1.11 | 4.6 | — | 68.3 % | 413.5 |
| 28 | **D3** Text tool | Drawing tools | 1 | 8 | 0.37 | 4.6 | — | 68.7 % | 421.5 |
| 29 | **B5** ABR brush import | Brushes | 1 | 8 | 0.36 | 4.5 | — | 69.0 % | 429.5 |
| 30 | **E2** Motion effects: motion blur, distortions, time effects | Effects and sound | 3 | 24 | 1.08 | 4.5 | — | 70.1 % | 453.5 |
| 31 | **F2** Pro formats: OpenEXR, more video codecs, video file import, scene export for compositing | Pipeline | 4 | 32 | 1.40 | 4.4 | — | 71.5 % | 485.5 |
| 32 | **T2** Vertical exposure sheet view | Timeline and animation | 1 | 25 | 1.09 | 4.4 | — | 72.6 % | 510.5 |
| 33 | **T3** Several shots per project and nested sequences | Timeline and animation | 2 | 50 | 2.18 | 4.4 | — | 74.8 % | 560.5 |
| 34 | **F4** Production paperwork: printed exposure sheets, EDL or OTIO shot lists | Pipeline | 2 | 6 | 0.35 | 5.8 | T3 | 75.1 % | 566.5 |
| 35 | **E5** GPU effects and real-time effects at 4K | Effects and sound | 2 | 29 | 1.25 | 4.3 | — | 76.4 % | 595.5 |
| 36 | **C4** Colour management and 16-bit drawings | Colour and layers | 2 | 33 | 1.40 | 4.2 | — | 77.8 % | 628.5 |
| 37 | **C3** Ink and paint: linked line and paint layers, auto-paint across drawings, linked colour styles, recolour line art | Colour and layers | 4 | 62 | 2.57 | 4.1 | — | 80.3 % | 690.5 |
| 38 | **G3** Plugin interface for effects and tools | Pipeline | 1 | 25 | 1.00 | 4.0 | G2 | 81.3 % | 715.5 |
| 39 | **B3** Brush depth: impasto, dual tips, scatter, pattern and ribbon, animated tips, brush blend modes | Brushes | 6 | 60 | 2.34 | 3.9 | — | 83.7 % | 775.5 |
| 40 | **B2** Wet media depth: dilution and drying, watercolour edges, blur brush | Brushes | 3 | 23.5 | 0.90 | 3.8 | — | 84.6 % | 799 |
| 41 | **R2** Multi-artist: check-out and check-in, asset library shared between shots | Pipeline | 2 | 33 | 1.24 | 3.8 | T3 | 85.8 % | 832 |
| 42 | **A4** Rotoscoping on video layers and point tracking | Timeline and animation | 2 | 29 | 0.98 | 3.4 | F2 | 86.8 % | 861 |
| 43 | **A2** Cut-out rigs: drawing-swap library and rig templates, inverse kinematics, master controllers, pose library | Timeline and animation | 4 | 70.5 | 2.35 | 3.3 | — | 89.1 % | 931.5 |
| 44 | **F3** Paper pipeline: scanning with peg-hole registration, camera capture | Pipeline | 2 | 33 | 1.05 | 3.2 | — | 90.2 % ← 90 % | 964.5 |
| 45 | **A1** Generated inbetween drawings (morphing between keys) | Timeline and animation | 1 | 25 | 0.78 | 3.1 | — | 91.0 % | 989.5 |
| 46 | **B4** GPU brushes, measured real time at 500–1000 px | Brushes | 2 | 29 | 0.90 | 3.1 | — | 91.9 % | 1018.5 |
| 47 | **E3** Tone and highlight lighting, particles | Effects and sound | 2 | 50 | 1.44 | 2.9 | — | 93.3 % | 1068.5 |
| 48 | **A3** Deformers: bone, curve and envelope | Timeline and animation | 1 | 60 | 1.57 | 2.6 | A2 | 94.9 % | 1128.5 |
| 49 | **D4** Vector stroke layers with reshape, width and intersection erase | Drawing tools | 2 | 85 | 2.21 | 2.6 | — | 97.1 % | 1213.5 |
| 50 | **D5** 3D reference models and posable figures | Drawing tools | 1 | 60 | 1.47 | 2.5 | — | 98.6 % | 1273.5 |
| 51 | **E4** Node compositing graph | Effects and sound | 1 | 60 | 1.44 | 2.4 | E1 | 100.0 % | 1333.5 |

## Timeline and animation

- **O1** (rank 6, 18.5 h, +1.70 points) Onion depth: all-layer test, per-layer settings, exact frames, shift and trace, rolling flipping. Rows: Onion on the current layer or on all layers; Onion settings per layer; Pick the exact relative frames shown; Shift and trace: move or turn a pinned frame without changing it; Rolling and flipping between chosen drawings from the keyboard.
- **T1** (rank 8) Timeline polish: landed (frame markers, ping-pong and play selection, timecode, timeline zoom, drawing names and substitution).
- **K1** (rank 24, 44 h, +2.40 points) Camera depth: path editing, depth of field, several cameras and cuts, free 3D camera. Rows: Camera path editing on the canvas; Depth of field by layer depth; Several cameras and camera cuts in a shot; Free 3D camera over a layered scene.
- **T4** (rank 25, 33 h, +1.64 points) Graph editor for keyed values and live transform recording. Rows: Graph editor for keyed values; Record transforms live while playing.
- **T2** (rank 32, 25 h, +1.09 points) Vertical exposure sheet view. Rows: Vertical exposure sheet view.
- **T3** (rank 33, 50 h, +2.18 points) Several shots per project and nested sequences. Rows: Several shots or scenes in one project; Nested sequences (a shot or symbol inside a shot).
- **A4** (rank 42, 29 h, +0.98 points, needs F2) Rotoscoping on video layers and point tracking. Rows: Rotoscoping on a video layer; Motion tracking of a point in footage.
- **A2** (rank 43, 70.5 h, +2.35 points) Cut-out rigs: drawing-swap library and rig templates, inverse kinematics, master controllers, pose library. Rows: Cut-out puppets: parts on pegs with drawing swaps; Inverse kinematics on peg chains; Master controllers driving poses; Pose and template library.
- **A1** (rank 45, 25 h, +0.78 points) Generated inbetween drawings (morphing between keys). Rows: Generated inbetween drawings (morphing between keys).
- **A3** (rank 48, 60 h, +1.57 points, needs A2) Deformers: bone, curve and envelope. Rows: Deformers: bone, curve and envelope on drawings.

## Drawing tools

- **S1** (rank 9, 21.5 h, +1.81 points) Selection depth: ellipse, magic wand, add/subtract/intersect, invert/grow/feather, quick mask, resampling choice. Rows: Ellipse selection; Select by colour with tolerance (magic wand); Add, subtract and intersect selections; Invert, grow, shrink and feather a selection; Painted selection masks (quick mask); Resampling choice (nearest, bilinear, bicubic).
- **S2** (rank 13, 27 h, +1.94 points) Transform depth: free distort and perspective, mesh warp, liquify, transform many drawings at once. Rows: Free distort, perspective and skew; Mesh warp; Liquify (push, twirl, pinch); Transform across several drawings or layers at once.
- **D1** (rank 18, 19.5 h, +1.20 points) Shape tools II: polyline and curve, filled shapes and lasso fill, gradient, hold-to-snap, clone stamp, reference viewer, grid snapping. Rows: Polyline, polygon and curve tools; Filled shapes and lasso fill; Gradient tool (linear, radial); Snap the end of a stroke to a clean line or ellipse when held; Clone stamp; Reference image viewer with eyedropper; Grid display and snapping strokes to it.
- **D2** (rank 27, 24 h, +1.11 points) Drawing guides: symmetry, stroke rulers, perspective rulers. Rows: Symmetry and mirror drawing (axes, radial); Stroke rulers: straight, curve, ellipse, concentric; Perspective rulers with vanishing points.
- **D3** (rank 28, 8 h, +0.37 points) Text tool. Rows: Text tool.
- **D4** (rank 49, 85 h, +2.21 points) Vector stroke layers with reshape, width and intersection erase. Rows: Vector stroke layers (strokes kept as editable curves); Vector line editing: reshape, width, erase to the intersection.
- **D5** (rank 50, 60 h, +1.47 points) 3D reference models and posable figures. Rows: 3D reference models and posable figures.

## Colour and layers

- **L1** (rank 3, now 3 h) Layer operations. Landed: duplicate, merge down / visible / flatten, 24 blend modes, copy properties. Row left: Reference layers chosen per tool (the fill reference is current or all visible).
- **L2** (rank 5, 16 h, +1.60 points) Painted layer masks and adjustment layers. Rows: Painted layer masks; Adjustment layers acting on everything below.
- **C1** (rank 11, 12 h, +0.93 points) Palettes: named palettes in the shot, palette files, colour wheel and sliders, recent colours, picker test. Landed: recent colours. Rows: Colour swatches; Palette files: save, load, import common formats; HSV picker with keyboard fields; Colour wheel, sliders and harmony modes; Recent colours.
- **C4** (rank 36, 33 h, +1.40 points) Colour management and 16-bit drawings. Rows: Colour management: working space, profiles, soft proof; 16-bit colour drawings.
- **C3** (rank 37, 62 h, +2.57 points) Ink and paint: linked line and paint layers, auto-paint across drawings, linked colour styles, recolour line art. Rows: Linked line and paint layers; Paint the same regions across drawings automatically; Linked colour styles (change a swatch, every fill using it updates); Recolour line art across drawings.

## Pipeline

- **R1** (rank 16, 14 h, +0.89 points) Review: drawn review notes, frame comments, review movie with burn-ins. Rows: Review notes drawn over frames, kept out of exports; Comments on frames; Review movie with burn-ins (shot, frame, timecode).
- **G1** (rank 17, 8 h, +0.50 points) Control channel over a socket and an MCP server. Rows: Control from other programs over a socket.
- **F1** (rank 22, 10.5 h, +0.61 points) Export breadth: WebP, multi-frame PSD/PSB, TGA/TIFF/JPEG sequences, transparent-background test, output scaling. Landed: animated and still WebP, multi-frame PSD / PSB, TGA and TIFF sequences. Rows left: JPEG sequences (the row is P); Transparent-background export; Export frame range and output size.
- **G2** (rank 26, 33.5 h, +1.62 points) Embedded scripting with an object API, macro recording, batch runs, script tool buttons. Rows: Embedded scripting language with an object API; Replay recorded actions (macros); Batch processing of many shots; Custom tool buttons that run scripts.
- **F2** (rank 31, 32 h, +1.40 points) Pro formats: OpenEXR, more video codecs, video file import, scene export for compositing. Rows: OpenEXR sequences, with layers; More video codecs: H.265, VP9 or AV1, MOV H.264; Video file import; Scene export for compositing apps (layers, camera, timing).
- **F4** (rank 34, 6 h, +0.35 points, needs T3) Production paperwork: printed exposure sheets, EDL or OTIO shot lists. Rows: Printed exposure sheets and frames (PDF); Shot list and edit decisions out (EDL or OTIO).
- **G3** (rank 38, 25 h, +1.00 points, needs G2) Plugin interface for effects and tools. Rows: Plugin interface for effects and tools.
- **R2** (rank 41, 33 h, +1.24 points, needs T3) Multi-artist: check-out and check-in, asset library shared between shots. Rows: Check-out and check-in of shots for several artists; Asset library shared between shots.
- **F3** (rank 44, 33 h, +1.05 points) Paper pipeline: scanning with peg-hole registration, camera capture. Rows: Scanned paper: peg-hole registration and clean-up; Camera capture for stop motion and pencil tests.

## Effects and sound

- **U1** (rank 19, 11 h, +0.67 points) Audio: split and duplicate clips, microphone recording. Rows: Split and duplicate clips; Record a voice track from the microphone.
- **E1** (rank 21, 13.5 h, +0.81 points) Effects finishing: drawn masks, line smoothing, gradient map and posterise, lens looks, transitions, presets. Rows: Soft masks from shapes; Line thickening and smoothing; Gradient map, posterise and threshold; Lens looks: vignette, chromatic aberration, flares; Transitions between cuts; Effect presets and saved stacks.
- **U2** (rank 23, 12 h, +0.67 points) Lip-sync: tune on recorded speech, lip-sync from the typed line. Rows: Mouth-chart lip-sync detection; Lip-sync from the typed line of dialogue.
- **E2** (rank 30, 24 h, +1.08 points) Motion effects: motion blur, distortions, time effects. Rows: Motion blur from movement; Distortions: ripple, wave, turbulence, displacement; Time effects: trails, frame blending, time remap.
- **E5** (rank 35, 29 h, +1.25 points) GPU effects and real-time effects at 4K. Rows: Effects rendered on the GPU; Effects play back in real time.
- **E3** (rank 47, 50 h, +1.44 points) Tone and highlight lighting, particles. Rows: Tone and highlight from mattes, lighting from normals; Particles (rain, snow, sparks).
- **E4** (rank 51, 60 h, +1.44 points, needs E1) Node compositing graph. Rows: Node compositing graph.

## Brushes

- **B6** (rank 7, 9 h, +0.82 points) Paper: visible paper, grain controls, texture overlay, canvas presets. Rows: Visible paper texture under the drawing; Grain scale, rotation and contrast; Texture overlay on a layer or a fill; Canvas size presets.
- **B1** (rank 20, 3 h, +0.18 points) Colour jitter per dab and per stroke. Rows: Colour jitter per dab and per stroke (hue, saturation, value).
- **B5** (rank 29, 8 h, +0.36 points) ABR brush import. Rows: Import of Photoshop ABR brushes.
- **B3** (rank 39, 60 h, +2.34 points) Brush depth: impasto, dual tips, scatter, pattern and ribbon, animated tips, brush blend modes. Rows: Thick paint with depth and lighting (impasto); Dual tips (a second tip or texture masking the first); Scatter and spray brushes (many dabs per step); Pattern and ribbon brushes (an image repeated along the stroke); Animated brushes (a tip that changes per dab or per frame); Brush blend modes (paint as multiply, behind, and so on).
- **B2** (rank 40, 23.5 h, +0.90 points) Wet media depth: dilution and drying, watercolour edges, blur brush. Rows: Wet paint: colour pickup, mixing and dilution; Watercolour edges: darkening and bleeding. Landed: blur brush.
- **B4** (rank 46, 29 h, +0.90 points) GPU brushes, measured real time at 500–1000 px. Rows: Brush rendering on the GPU; Brushes at large sizes stay real time.

## UI and input

- **I3** (rank 1, 1.5 h, +0.20 points) Input-to-screen latency measured. Rows: End-to-end stroke latency measured.
- **I2** (rank 2) Touch: landed (tap gestures; palm rejection while the pen is seen). Row left as P: pen proximity on Windows and Wayland-native windows, which comes with item I1.
- **I1** (rank 4, 18.5 h, +1.98 points) Pen everywhere: Windows tilt, Wayland-native pen, side buttons, barrel rotation, pressure curve. Rows: Pen pressure and tilt on Windows; Pen input in Wayland-native windows; Pen side buttons mapped to actions; Pen barrel rotation as an input; Global pressure curve and calibration.
- **X1** (rank 10, 18.5 h, +1.53 points) UI polish: command search, tool palettes and radial menus, undo history panel, preferences dialog, docking tests, keyboard reach. Landed: undo history panel. Rows: Command search; Custom tool palettes and radial menus; Preferences dialog; Dockable panels: move, tab, float; Every action reachable from the keyboard.
- **X3** (rank 14, 4 h, +0.28 points) Shipped translations. Rows: Translated interface.
- **X2** (rank 15, 8 h, +0.56 points) Second view of the shot and a navigator. Rows: Second view of the same shot and a navigator.

## Notes outside the map

### Tile seams at fractional zoom
Tiles are separate textures with clamp-to-edge sampling, so linear filtering does not blend across
tile edges. Not visible in normal use; a texture atlas with 1 px gutters would remove it.

### Autosave covers the project, not the open stroke
A crash mid-stroke loses that stroke (at most one autosave interval of 45 s otherwise).

### Shared libraries (RFC-0040)
- The tile engine (`tooncraft-raster`) and brush engine overlap with `photocraft-raster` /
  `photocraft-paint` and BoardCraft's brush. Candidate for a shared `craft-paint` crate; proposal to
  be written, nothing copied.
- Exposure-sheet and onion-skin logic overlaps with AniCraft's timeline (vector); candidate for a
  shared `craft-exposure` crate. AniCraft's vector strokes are the natural source for item D4.
- **D4 core landed (2026-11-24):** `crates/vector` (`tooncraft-vector`) keeps strokes as centre lines
  with a width per point (Catmull-Rom through the points, Ramer–Douglas–Peucker thinning, smoothing,
  erase to crossings with a 4 000 000 segment-pair cap). It depends only on serde and thiserror, so
  it can move to craft-libs as a shared vector-stroke crate once AniCraft's strokes are compared with
  it (house rule 16); that proposal is still to be made. Left: a point-dragging contour tool on the
  canvas, shapes and symmetry copies as vector lines, Duplicate Drawing keeping the lines, a vector
  preview while drawing (the dabs are a raster preview today), closed shapes and fills that follow
  edited lines.
- The pen reader (`photocraft-tablet`) belongs in craft-libs as `craft-tablet` (item I1).
- Choosing Xwayland on a Wayland session with a pen attached (`apps/tooncraft/src/display_session.rs`:
  the pen found in `/proc/bus/input/devices`, winit's X11 backend through eframe's event-loop hook)
  is written for ToonCraft; PhotoCraft does the same in its own app code. It belongs beside the pen
  reader in a shared `craft-tablet`, so both apps use one copy.
- `craft-palette` reads GIMP palettes but has no writer and no Photoshop `.aco` reader. ToonCraft writes `.gpl` and reads `.aco` in `tooncraft-interop::palette` until craft-palette grows them; both are small and ready to move there.
