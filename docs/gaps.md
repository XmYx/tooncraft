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
| 2 | Effects, the rest: drawn mask shapes, smoothing of jagged lines, a GPU path (RFC-0022) | Effects | | 14 | target rows |
| 3 | Lip-sync: check detection on recorded speech and tune it; microphone recording | Audio | | 12 | F1 |
| 12 | GPU dab rasterisation (craft-gpu, RFC-0022) | Performance | | 30 | below |
| 14 | WebP export, multi-frame PSD / PSB, TGA / TIFF sequences | Export | | 8 | F10 |
| 15 | Translations (craft-i18n catalogs) | Languages | | 6 | [localization-parity.md](localization-parity.md) |

Closed in the sound pass: several audio tracks and clips with trim, fades, volume keys, mute and
solo, scrubbing while dragging the playhead, and a WAV mix export. Still open: recording a voice
track from the microphone (craft-audio has no capture; the app's cpal output could take an input
stream, which belongs in craft-audio first).

Closed in the effects pass: a non-destructive, keyframable effect stack per layer (blurs, glow,
shadow, outline, line thickening, colour adjustments, grain, light wrap, soft masks) in the canvas,
playback and every export.

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

### Paging to disk: what is left
Drawings away from the playhead are paged to a scratch file once they pass the page budget
([performance.md](performance.md#long-shots-paging-to-disk)). Left: the page file only grows (a
drawing edited and paged again is written again; it is deleted when the shot is closed), undo
history keeps its own copies of changed tiles (bounded by the undo budget, not paged), and the
GPU tile cache is still bounded separately at 768 MB.

### Autosave covers the project, not the open stroke
A crash mid-stroke loses that stroke (at most one autosave interval of 45 s otherwise).

## Shared libraries (RFC-0040)

- The tile engine (`tooncraft-raster`) and brush engine overlap with `photocraft-raster` /
  `photocraft-paint` and BoardCraft's brush. Candidate for a shared `craft-paint` crate; proposal to
  be written, nothing copied.
- Exposure-sheet and onion-skin logic overlaps with AniCraft's timeline (vector); candidate for a
  shared `craft-exposure` crate.
- Docking uses craft-ui; ZIP, atomic saves, PNG, GIF, OpenRaster and i18n use craft-libs.
