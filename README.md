<p align="center">
  <img src="assets/app-icon/tooncraft.svg" width="128" alt="ToonCraft icon">
</p>

<h1 align="center">ToonCraft</h1>

<p align="center"><b>Hand-drawn, frame-by-frame animation.</b></p>

<p align="center">
Open-source raster animation in pure Rust: draw with pressure-sensitive pencils, inks and chalk on
a GPU canvas, time your drawings on an exposure sheet, flip with onion skin and a light table,
colour with a gap-closing paint bucket and play it back in real time. One of the ArtCraft Crafting
Apps, next to AniCraft (vector animation), BoardCraft (storyboards) and FilmCraft (editing).
</p>

<p align="center">
  <img alt="Status: pre-alpha" src="https://img.shields.io/badge/status-pre--alpha-orange">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-b7410e">
</p>

![Drawing with onion skin](docs/images/tooncraft-onion-skin.png)

[What works today](#what-works-today) · [Getting started](#getting-started) ·
[Agents and CLI](#agents-and-cli) · [Roadmap](#roadmap) · [License](#license-and-credits)

## What works today

- **Canvas:** tiled, sparse, GPU-composited; pan, zoom about the pointer, rotate, mirror; up to
  16384 px a side.
- **Brushes:** pencil with paper grain, ink pen, airbrush, chalk, eraser, image tips and
  `.toonbrush` presets; pen pressure, tilt and the eraser end (Linux X11, macOS, Windows); speed
  dynamics, smoothing and a "steady line" stabilizer; strokes never darken where dabs overlap.
- **Selection and shapes:** lasso and rectangle selections you move, scale, rotate and flip live;
  lines, rectangles and ellipses.
- **Exposure sheet:** layers with keys, holds and blanks; new / duplicate drawing, insert / delete
  frame, drag a drawing's cells to change its hold, re-time on 1s–4s without losing drawings; select
  ranges across layers, copy / cut / paste them (linked or as new drawings), reverse and repeat them.
- **Layers:** blend modes, folders, label colours, timeline thumbnails.
- **Camera:** eased pan, zoom and rotation keys; exports render through it.
- **Sound:** a soundtrack with its waveform in the timeline, frame-by-frame scrubbing and sound in
  the MP4.
- **Onion skin and light table:** before / after counts, tints, falloff, by drawings or by frames,
  every layer or one; pin any frame of any layer under the drawing.
- **Colour:** paint bucket with gap closing and spread under lines, on the line layer or a paint
  layer underneath; HSV picker, swatches, eyedropper.
- **Playback** at the shot's frame rate, looping, clock-driven.
- **Safety:** unlimited undo (memory-bounded), journaled atomic saves with a backup, Save New
  Version, background autosave with crash recovery, a property-tested loader.
- **Works with other apps:** layered PSD / PSB and OpenRaster in and out, XDTS exposure sheets with
  their drawing folders in and out, ProRes 4444 with alpha and per-layer PNG sequences for
  compositing, MP4 (H.264 + AAC), GIF, animated PNG; all built in, no external tools
  ([docs/interop.md](docs/interop.md)).
- **Yours to keep:** open source, offline, no account, no tiers, no watermark; a 4K, 240-drawing shot
  plays in real time and loads in a tenth of a second.
- **Accessibility:** every control named for screen readers, the canvas and exposure-sheet cells
  included; everything works from the keyboard, including drawing (the keyboard pen).

![The timeline with the light table](docs/images/tooncraft-timeline.png)

## Getting started

```bash
cargo run --release -p tooncraft -- --demo
```

Keys: P pencil, N ink, A airbrush, C chalk, E eraser, F fill, I picker, S select, U shapes, H hand ·
Enter play · , . frames · Shift+N new drawing · Shift+D duplicate · + − hold · O onion skin · [ ] brush
size · Ctrl+[ ] rotate · F6 canvas ↔ sheet. Help ▸ Keyboard Shortcuts lists them all and remaps them.

`cargo xtask ci` runs every check (fmt, clippy, tests, layering, assets, progress, parity, strings,
licences).

## Agents and CLI

```bash
cargo run -p tooncraft-cli -- demo shot.toon
cargo run -p tooncraft-cli -- run shot.toon commands.jsonl
cargo run -p tooncraft-cli -- export shot.toon --mp4 shot.mp4
cargo run --release -p tooncraft-cli -- bench
```

Every action is an engine command ([docs/control-protocol.md](docs/control-protocol.md)).

## Roadmap

[ROADMAP.md](ROADMAP.md) · [docs/roadmap.md](docs/roadmap.md) · [docs/gaps.md](docs/gaps.md) ·
[docs/target-app-parity.md](docs/target-app-parity.md) · [docs/performance.md](docs/performance.md)

## License and credits

MIT OR Apache-2.0 ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)); see
[NOTICE](NOTICE) and [ATTRIBUTION.md](ATTRIBUTION.md). ToonCraft is a clean-room project and is not
affiliated with any other animation software vendor.
