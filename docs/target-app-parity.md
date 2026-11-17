# ToonCraft vs the reference animation apps: parity map

> **Last updated:** 2026-11-17 · **Change:** major · **Target:** raster frame-by-frame animation as in
> TVPaint Animation 12, the drawing and animation side of Toon Boom Harmony 24, Clip Studio Paint 4
> (animation features) and Procreate Dreams 1.x

This is the only document that names the reference apps. Everything here was learned from their
public manuals, tutorial videos and observed behaviour; no code, strings, icons or artwork were
read or copied (see [AGENTS.md](../AGENTS.md), clean room). Our own names for their features are in
[feature-names.md](feature-names.md).

## Headline

`cargo xtask parity` counts the status column of every table below and prints the score; the
numbers here and in [ROADMAP.md](../ROADMAP.md) are copied from its output, never estimated by hand.

<!-- parity:start -->
| | Value | Kind |
|---|---|---|
| Feature rows | 154 | measured |
| Done (D) | 123 | measured |
| Partial (P) | 19 | measured |
| Missing (M) | 12 | measured |
| Parity score, (D + ½·P) / rows | 86 % | measured |
| Brush engine and tools | 81 % (19 D, 1 P, 4 M) | measured |
| Paper and canvas | 91 % (9 D, 2 P, 0 M) | measured |
| Layers and timeline / exposure sheet | 86 % (19 D, 0 P, 3 M) | measured |
| Onion skin and light table | 90 % (8 D, 2 P, 0 M) | measured |
| Animation tools | 95 % (9 D, 1 P, 0 M) | measured |
| Colouring | 75 % (6 D, 3 P, 1 M) | measured |
| Effects | 83 % (9 D, 2 P, 1 M) | measured |
| Audio and lip-sync | 88 % (10 D, 1 P, 1 M) | measured |
| Import / export | 88 % (13 D, 2 P, 1 M) | measured |
| UI and workspace | 84 % (12 D, 3 P, 1 M) | measured |
| Stability and performance | 91 % (9 D, 2 P, 0 M) | measured |
<!-- parity:end -->

Status: **D** done and tested, **P** partial (works, but narrower than the references), **M**
missing. **Ours** marks where the plan is to do better than all four references, and how.

## How it was measured

Every row is a capability an animator uses in at least two of the references (or, for rows marked
—, a suite rule none of them meets). A row is D only when it works in the app *and* has a test (core
test, CLI test or kittest UI test); P when it works for the common case but lacks options the
references have; M otherwise. The rows were written before the code, so the list is not bent toward
what exists. The score weighs every row the same, so it overstates how close we are for heavy
features (video export, effects, audio) and understates it for small ones.

## Brush engine and tools

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Pressure → size and opacity | all | D | |
| Tilt → dab angle and flattening | TV, CSP, PD | D | |
| Velocity → size (thin when fast) | TV, CSP | D | |
| Stroke smoothing (moving average) | all | D | |
| Pulled-string stabilizer | TV, CSP, PD | D | |
| Hard pencil with paper grain | all | D | |
| Antialiased ink pen | all | D | |
| Soft airbrush with flow build-up | all | D | |
| Eraser | all | D | |
| Textured chalk / grain brush | TV, CSP, PD | D | |
| Opacity vs flow without dab overlap darkening | TV, CSP | D | |
| Brush presets | all | D | `.toonbrush` files and a preset library folder |
| Brush library: categories, favourites, recent brushes, search | TV, CSP, PD | D | **Ours:** a Brushes panel; every list and the search work from the keyboard (Ctrl+B); built-in variants; folders are the categories |
| Custom brush tips from images | TV, CSP, PD | D | |
| Brush editor with curves for each dynamic | TV, CSP | M | |
| Watercolour / wet mixing brushes | TV, CSP, PD | M | |
| Smudge / blur tool | all | M | |
| Shape tools (line, rectangle, ellipse) | all | D | Shift: 15° lines, squares, circles |
| Lasso / rectangle selection | all | D | |
| Transform selection (move, scale, rotate) | all | D | Live preview, one undo step; flips and 90° turns |
| Eyedropper | all | D | |
| Fill bucket | all | D | |
| GPU dab rasterisation | — | M | **Ours:** dabs on the GPU through craft-gpu (RFC-0022), CPU path kept as the oracle |
| Pen pressure and tilt from tablets on every desktop | all | P | X11 (incl. Xwayland) and macOS through the shared photocraft-tablet reader, Windows Ink as touch force; Wayland-native windows have no pen data yet |
| Pen eraser end switches tool | TV, CSP | D | |

## Paper and canvas

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Tiled sparse canvas (empty space costs nothing) | TV, CSP | D | |
| Pan, zoom, rotate view | all | D | |
| Pan, zoom and rotate from the keyboard, with reset | all | D | Alt+arrows pan |
| Flip view horizontally | all | D | |
| Fit to window | all | D | |
| Paper colour | all | D | |
| Paper grain texture | TV, CSP | P | Grain shapes pencil and chalk dabs; no visible paper texture layer |
| Canvas presets (HD, 4K, square) | all | P | |
| Field guide / safe areas | TV, TB | D | A 12-field guide with grid and centre, action-safe (90 %) and title-safe (80 %) frames; View menu |
| Canvas resize / crop | all | D | Nine anchors, or crop to the camera frame; drawings are moved whole pixels, never resampled; peg pivots and camera keys follow; one undo step brings cut paint back |
| Canvas up to 16384 px per side | TV | D | |

## Layers and timeline / exposure sheet

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Animation layers with their own exposures | all | D | |
| Add, delete, rename, reorder layers | all | D | |
| Layer visibility, lock, opacity | all | D | |
| Layer opacity keys (fades) | TV, TB, PD | D | Linear between keys; the canvas and every export follow them |
| Holds (a drawing exposed several frames) | all | D | |
| Insert / delete frame | all | D | |
| Duplicate drawing | all | D | |
| New blank drawing at frame | all | D | |
| Extend / shorten hold by keyboard | TV, TB | D | |
| Timeline grid with keys, holds and blanks | all | D | |
| Vertical exposure sheet view | TB, TV | M | |
| Frame numbers and current-frame marker | all | D | |
| Change a hold by dragging its cells | TB, CSP, PD | D | Keyboard: + / − |
| Select a range of cells across layers | all | D | |
| Layer blending modes | all | D | Multiply, screen, overlay, darken, lighten, add |
| Layer groups / folders | all | D | Nested folders with show, lock, fold, opacity and a label colour; the timeline shows each layer's folder path with a colour stripe per level and hides folded rows |
| Clipping layers (show only over the layer below) | TV, CSP | D | Any number of clipping layers on one base, with their own blend modes; the base's opacity carries the group |
| Colour labels with a layer filter | TV, TB | D | Labels on layers and folders; the layer list and timeline can show only one label |
| Thumbnails in timeline cells | TV, PD | D | |
| Markers / notes on frames | TV, TB | M | |
| Several shots in one project | TV, TB | M | |
| Drawings kept when no longer exposed | TB | D | **Ours:** re-timing never deletes a drawing; unexposed drawings stay until purged, and the purge is undoable |

## Onion skin and light table

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Onion skin on/off | all | D | |
| Separate before / after counts | all | D | |
| Opacity with falloff | all | D | |
| Tint colours for before / after | all | D | |
| Onion by drawings or by frames | TV, TB | D | |
| Onion on current layer or all layers | TV, CSP | P | Both work; only the current-layer path has a test |
| Light table: pin any frame of any layer | TB, CSP | D | |
| Light table opacity | TB, CSP | D | |
| Onion skin as outlines only | TV | D | Also: hide only the previous step |
| Onion skin cached on the GPU | — | P | **Ours:** tinted onion tiles are textures cached per tile revision, so scrubbing with onion skin costs no CPU compositing |

## Animation tools

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Key / breakdown / inbetween marking | TV, TB | D | Letters on the timeline cells and in their screen-reader names; a spacing chart of the stretch around the current frame next to the chart an ease would give |
| Flip through drawings (scrub) | all | D | |
| Re-time a range (on 1s, 2s, 3s) | TV, TB | D | **Ours:** non-destructive, one undo step; presets (ones to fours, slow start, slow end, both) and re-timing between keys that keeps keys and breakdowns on their frames |
| Reverse a range | TV, TB | D | |
| Camera pan / zoom / rotate keys | TV, TB, PD | D | Eased keys, exports render through the camera; layers at a depth for multiplane parallax when the camera pans or trucks in; repeatable camera shakes |
| Pegs (parented transforms) | TB | D | Nested pegs (a peg can hang from a peg, loops refused); pivot, move, rotate, scale and skew keys; motion path on the canvas; drawings are never resampled, only rendered through the chain |
| Keyed layer transforms with easing | TB, CSP, PD | D | Linear, ease in / out / both, hold, or a custom two-handle timing curve per key |
| Cut / copy / paste cells | all | D | **Ours:** paste linked or as new drawings, in place or inserting |
| Cycles (loop a range of cells) | TV, TB | D | |
| In-between assist | — | P | New inbetween drawings placed along a spacing chart, with blended guides on a locked layer above; **Ours:** AI in-betweening later through craft-ai-integrations |

## Colouring

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Flood fill | all | D | |
| Close gaps (configurable size) | all | D | |
| Fill tolerance | all | D | |
| Fill reference: current layer or all visible layers | TV, CSP, TB | D | |
| Fill under lines (spread into the line's antialiasing) | TV, CSP, TB | D | |
| Separate line and paint layers (paint under ink) | TB, CSP | P | Paint on a layer below the ink with reference = all visible; no linked line/paint pair yet |
| Fill every frame of a layer at once | TV, TB | D | One undo step |
| Colour swatches | all | P | |
| Colour models and palette files | TB | M | |
| Colour picker (hue, saturation, value) | all | P | Works with keyboard fields; no behaviour test yet |

## Effects

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Blur, glow, shadow on layers | TV, TB, PD | D | Soft, motion and zoom blur, glow, drop shadow, outline |
| Colour adjust (levels, hue) | all | D | Levels, tone curve, hue / saturation / lightness, tint |
| Line smoothing / thickening of drawn lines | TV | P | Thicken and thin lines keeping their colours (exact distance transform); no smoothing of jagged lines |
| Non-destructive effect stack | TV, TB | D | Per layer, reorderable, switchable, saved in the shot; one undo step per edit |
| Keyframed effect settings | TV, TB, PD | D | Linear between keys, held outside them |
| Masks with soft edges on a layer | TV, TB, PD | P | Rectangle and ellipse with feather and invert, keyframable; no drawn mask shapes |
| Effects in exports, including per-layer exports | TV, TB | D | MP4, ProRes, GIF, PNG and per-layer PNG sequences go through the same compositor |
| Film grain and light wrap | TV, TB | D | Grain fixed or new every frame; light wrap reads the layers below |
| Matte layers that cut the layers below (keep inside / outside) | TV, TB | D | The matte is not drawn; it clips the layers right below it in its folder, with its own effects and opacity |
| Colour key (drop a colour, e.g. scanned paper) | TV, TB | D | Tolerance and soft edge |
| Blend mode and strength per effect | TV, TB | D | Every blend mode of the layers, 0…1 strength |
| Effects on the GPU | — | M | **Ours:** a craft-gpu path (RFC-0022) with the CPU path as the oracle; skipped when there is no adapter |

## Audio and lip-sync

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Import audio track | all | D | WAV, AIFF, MP3, AAC/M4A, FLAC, Ogg, Opus through craft-audio; saved in the shot |
| Waveform in timeline | all | D | The mix of every clip; drag to slip the first clip |
| Scrub audio | all | D | Each frame step, and every frame passed while dragging the frame ruler, plays that frame's sound |
| Several audio tracks and clips | TV, TB, CSP, PD | D | Up to 16 tracks and 64 clips; clips of one file share it in the shot |
| Clip start, trim and fades | TB, CSP, PD | D | Start frame, trim start, length, fade in / out in seconds |
| Volume keys on a clip | TB, PD | D | Linear between keys; a clip and a track volume on top |
| Mute and solo tracks | TB, PD | D | |
| Export the sound mix | TV, TB | D | WAV (16-bit, 48 kHz stereo); MP4 carries the same mix as AAC |
| Record a voice track from the microphone | TB, CSP, PD | M | craft-audio has no capture yet |
| Mouth-chart lip-sync detection | TB | P | **Ours:** offline and model-free: loudness, noise and formant rules over the sound mix map to a nine-drawing mouth chart. Checked on synthetic vowels, silence, hiss and closures, not yet on a speech corpus |
| Lip-sync result as editable exposure keys | TB | D | One undo step; keys and holds like hand timing |
| Mouth timing sheets in and out | TB | D | Our documented text sheet and the common switch-data `.dat`; other frame rates are rescaled |

## Import / export

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Native project save / open | all | D | |
| PNG sequence export | all | D | |
| GIF export | TV, CSP, PD | D | |
| APNG / WebP export | TV, CSP | P | Animated PNG; WebP missing |
| Video export (MP4 / MOV) | all | D | H.264 MP4 through craft-video and craft-media, no external tools |
| PSD layers export | TV, CSP | D | Layered PSD of a frame through craft-psd; PSD / PSB layers import too |
| OpenRaster frame export | TV | D | Through craft-ora |
| Image sequence import | all | D | |
| BoardCraft animatic as a starting layout | — | D | **Ours:** panels become a locked reference layer with their timings, key marks and camera moves ([interop.md](interop.md)) |
| Transparent background export | all | P | PNG and OpenRaster; no test yet |
| ProRes MOV with alpha for compositing | TV, TB | D | ProRes 4444 with alpha or 422 HQ through craft-video |
| Video export carries the soundtrack | all | D | AAC in the MP4 |
| Per-layer PNG sequences for compositing | TV, TB | D | |
| Exposure sheet exchange (XDTS) with drawing folders | TB, CSP | D | Import and export, timing round-trips |
| Layers from PSD / OpenRaster import | TV, CSP | D | |
| Animated GIF import | TV, PD | D | Timed by its delays |

## UI and workspace

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Dense dark theme | all | D | |
| Dockable panels (move, tab, float) | TV, TB, CSP | P | Through craft-ui docking; moving panels is not tested here |
| Tool bar | all | D | |
| Tool options bar | all | D | |
| Brush settings panel | all | D | |
| Colour panel | all | D | |
| Layers panel | all | D | |
| Timeline docked at the bottom | all | D | |
| Keyboard shortcuts for every tool | all | D | |
| Customisable shortcuts | all | D | Through craft-keymap, saved per user |
| Every action reachable by keyboard | — | P | **Ours:** accessibility rule, kittest-tested |
| Screen reader names for canvas and timeline | — | D | **Ours:** AccessKit nodes for the canvas and every visible timeline cell |
| Saved workspaces | TV, TB, CSP | D | Named panel layouts saved per user, plus built-in Drawing and Timing layouts |
| Localised UI | all | P | craft-i18n ready, English only today |
| Command-line control | TV | D | **Ours:** every action is an engine command; `tooncraft-cli run` applies JSON-lines command files |
| MCP / control channel for agents | — | M | **Ours:** planned on craft-control |

## Stability and performance

| Feature | Refs | Status | Ours |
|---|---|---|---|
| Unlimited undo / redo | all | D | Bounded by a memory budget, never by a step count |
| Atomic saves (never a half-written file) | — | D | **Ours:** craft-atomic plus a save journal |
| Autosave and crash recovery | TV, CSP, PD | D | **Ours:** written on a background thread from a copy-on-write snapshot, no UI stall |
| Hostile-file safety (property-tested loader) | — | D | **Ours:** property tests of the loader and the fill |
| Never panics (lint-enforced) | — | D | **Ours:** deny lints on every crate |
| Memory bounded on huge canvases and long shots | — | P | Sparse tiles and budgets; paging to disk is planned |
| Real-time playback at project fps | all | D | |
| Playback cache | TV, TB | D | Tile textures cached; with effects, blend modes or mattes, composited frames are cached per version and a worker renders ahead of the playhead |
| Effects play back in real time | TV, TB | P | 1080p, 240 frames, 4 effects: 24.1 fps with 0 dropped (10 s, cold), 23.8 fps with 5 dropped (30 s); 4K drops frames; stalls when the machine swaps ([performance.md](performance.md)) |
| Stroke latency measured and published | — | D | **Ours:** [performance.md](performance.md) |
| Versioned saves (save new version) | TV, TB | D | |
| Large shots stay real-time (4K, 240 drawings) | — | D | **Ours:** measured, 0 dropped frames |

Refs: TV = TVPaint Animation, TB = Toon Boom Harmony, CSP = Clip Studio Paint, PD = Procreate Dreams.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First map, written before the vertical slice; statuses set when the slice landed |
| 2026-11-17 | major | Polish pass: re-measured after user research (docs/user-research.md); 4 rows added |
| 2026-11-17 | major | Second pass: the top 10 gaps and interchange with other apps; 6 rows added |
| 2026-11-17 | minor | BoardCraft boards as the layout |
| 2026-11-17 | minor | Canvas tools: field guide and safe areas, canvas resize and crop, saved workspaces, onion outlines |
| 2026-11-17 | minor | Timing: drawing marks, spacing chart, presets, re-timing between keys, inbetween guides |
| 2026-11-17 | minor | Nested folders, folder locks, clipping layers and a label filter; 2 rows added |
| 2026-11-17 | minor | Pegs and transform layers with easing, multiplane depth and camera shake; 1 row added |
| 2026-11-17 | minor | Effects stack: 4 effects rows re-scored, 5 rows added (keys, masks, exports, grain / light wrap, GPU) |
| 2026-11-17 | minor | Compositing basics: 4 rows added (opacity keys, mattes, colour key, effect blending) |
| 2026-11-17 | minor | Sound depth: 6 audio rows added (tracks and clips, trim and fades, volume keys, mute / solo, mix export, recording) |
| 2026-11-17 | minor | Lip-sync assist: lip-sync row re-scored (P), 2 rows added (editable keys, timing sheets) |
| 2026-11-17 | minor | Effects performance: playback cache re-scored (D), 1 row added (effects in real time, P) |
