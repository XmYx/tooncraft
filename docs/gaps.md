# Where ToonCraft falls short of the reference animation apps

> **Last updated:** 2026-11-17 · **Change:** major · **Target:** see [target-app-parity.md](target-app-parity.md)

This is the work list: pick from the top. **B** marks beta blockers. Hours are agent-hour
estimates, not measurements. When a gap closes, delete its entry and update the parity table.

## Ranked: the top 15

| # | Gap | Category | Beta | Hours | Doc |
|---|---|---|---|---|---|
| 1 | Pen pressure and tilt on Linux X11, Wayland and macOS (winit drops them) | Input | B | 16 | below |
| 2 | Selection, transform (move, scale, rotate) and cut / copy / paste of pixels | Drawing | B | 30 | |
| 3 | Copy / paste / move of timeline cells and ranges | Timeline | B | 12 | |
| 4 | Video export (MP4 / MOV) through FilmCraft or craft-libs codecs | Export | B | 16 | |
| 5 | Audio track with waveform and scrubbing | Audio | B | 30 | |
| 6 | Camera keys (pan, zoom, rotate) per shot | Animation | B | 24 | |
| 7 | Image-sequence import and BoardCraft animatic import as a layout | Import | B | 14 | [interop.md](interop.md) |
| 8 | GPU dab rasterisation (RFC-0022 craft-gpu once on craft-libs main) | Performance | | 30 | below |
| 9 | Shape tools (line, rectangle, ellipse) and a ruler / perspective guide | Drawing | | 14 | |
| 10 | Layer blending modes and folders | Layers | | 12 | |
| 11 | Customisable shortcuts (craft-keymap) and saved workspaces | UI | | 10 | |
| 12 | Brush tips from images, brush editor with curves, preset library files | Brushes | | 24 | |
| 13 | Paging of drawings to disk for very long shots | Stability | | 20 | below |
| 14 | APNG, WebP and PSD-layers export (craft-libs codecs) | Export | | 8 | |
| 15 | Translations (craft-i18n catalogs) | Languages | | 6 | [localization-parity.md](localization-parity.md) |

## Input

### Pen pressure and tilt on every desktop
winit 0.30 reports pen force only as touch force on some platforms (Windows Ink, mobile). PhotoCraft
has `photocraft-tablet` (XInput2 on X11, AppKit on macOS, WinTab on Windows). Per RFC-0040 the right
move is to propose it to craft-libs (`craft-tablet`) and use it here rather than copy it. Until then
mouse and pens without touch force draw at the "Mouse pressure" setting.

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
