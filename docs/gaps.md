# Where ToonCraft falls short of the reference animation apps

> **Last updated:** 2026-11-17 · **Change:** major · **Target:** see [target-app-parity.md](target-app-parity.md)

This is the work list: pick from the top. **B** marks beta blockers. Hours are agent-hour
estimates, not measurements. When a gap closes, delete its entry and update the parity table.

## Ranked: the top 15

Ranked by impact for a working animator, using [user-research.md](user-research.md) (A = annoyance,
F = request).

| # | Gap | Category | Beta | Hours | Evidence |
|---|---|---|---|---|---|
| 1 | Pen input in Wayland-native windows (tablet-v2 protocol through winit) | Input | B | 16 | A5 |
| 2 | Effects: blur, glow, shadow, colour adjust as a non-destructive stack | Effects | | 30 | target rows |
| 3 | Lip-sync assist: mouth charts from the soundtrack | Audio | | 24 | F1 |
| 5 | Nested folders, clipping layers, folders in the timeline | Layers | | 12 | F15 |
| 6 | Brush editor with curves per dynamic; textured brush grain images | Brushes | | 20 | F7 |
| 7 | Key / breakdown / inbetween marks and a planning chart | Animation | | 10 | target rows |
| 8 | Field guide, safe areas, canvas resize / crop | Canvas | | 10 | target rows |
| 9 | Saved workspaces (panel layouts) | UI | | 6 | A13 |
| 10 | Onion skin as outlines; hide only the previous step | Onion | | 6 | F2 |
| 11 | BoardCraft animatic import as a layout | Import | | 10 | [interop.md](interop.md) |
| 12 | GPU dab rasterisation (craft-gpu, RFC-0022) | Performance | | 30 | below |
| 13 | Paging of drawings to disk for very long shots | Stability | | 20 | below |
| 14 | WebP export, multi-frame PSD / PSB, TGA / TIFF sequences | Export | | 8 | F10 |
| 15 | Translations (craft-i18n catalogs) | Languages | | 6 | [localization-parity.md](localization-parity.md) |

Closed in the second pass: pen pressure / tilt / eraser end on X11 and macOS, soundtrack with
waveform, scrubbing and sound in MP4, selection and transform, camera keys, remappable shortcuts,
shape tools, timeline thumbnails, blend modes, folders and layer colours, fill across frames,
image brush tips and preset files, and exchange with other apps (PSD / PSB and OpenRaster layers,
XDTS exposure sheets with drawing folders, GIF import, ProRes 4444 with alpha, per-layer
sequences).

## Input

### Pen input in Wayland-native windows
Pressure, tilt and the eraser end come from PhotoCraft's `photocraft-tablet` (pinned git
dependency) on X11 (also under Xwayland) and macOS, and from winit's touch force on Windows.
Wayland-native windows need the tablet-v2 protocol bound on winit's own connection. The shared
reader belongs in craft-libs (`craft-tablet`, RFC-0040 proposal to write).

### Input-to-photon latency
Engine latency per sample is measured ([performance.md](performance.md)); end-to-end latency on
screen is not. Needs a high-speed camera test or a frame-timing probe.

## Performance

### GPU brush engine
Dabs are rasterised on the CPU (fast enough: ink 0.008 ms per sample). The plan to exceed the
references is a GPU dab pipeline on craft-gpu (RFC-0022, draft craft-libs#663) with the CPU path
kept as the oracle for tests. Display and compositing are already on the GPU (tile textures).

### Tile seams at fractional zoom
Tiles are separate textures with clamp-to-edge sampling, so linear filtering does not blend across
tile edges. Not visible in normal use; a texture atlas with 1 px gutters would remove it.

## Stability

### Paging to disk
Everything lives in RAM (sparse, copy-on-write). A 2000-drawing 4K shot can need several GB; the
budgets refuse rather than crash, but paging cold drawings to a cache file would lift the limit.

### Autosave covers the project, not the open stroke
A crash mid-stroke loses that stroke (at most one autosave interval of 45 s otherwise).

## Shared libraries (RFC-0040)

- The tile engine (`tooncraft-raster`) and brush engine overlap with `photocraft-raster` /
  `photocraft-paint` and BoardCraft's brush. Candidate for a shared `craft-paint` crate; proposal to
  be written, nothing copied.
- Exposure-sheet and onion-skin logic overlaps with AniCraft's timeline (vector); candidate for a
  shared `craft-exposure` crate.
- Docking uses craft-ui; ZIP, atomic saves, PNG, GIF, OpenRaster and i18n use craft-libs.
