# Architecture

| Layer | Crate | What it does | UI? |
|---|---|---|---|
| L0 | `tooncraft-raster` | 64×64 premultiplied RGBA8 tiles, sparse copy-on-write surfaces, compositing, flood fill with gap closing | no |
| L1 | `tooncraft-brush` | samples → stabilizer → dabs → coverage → pixels; pencil, ink, airbrush, chalk, eraser | no |
| L1 | `tooncraft-doc` | project, layers, exposure sheets (key / hold / blank), drawing library, onion skin, light table, CPU compositing | no |
| L2 | `tooncraft-format` | `.toon` file (craft-zip), journaled atomic saves (craft-atomic), backup, autosave | no |
| L3 | `tooncraft-engine` | `Command` set, undo / redo, live strokes, fill, playback clock, exports (craft-gif, craft-ora, PNG), demo shot | no |
| app | `tooncraft-cli` | new, demo, info, run (JSON-lines commands), export, check, bench | no |
| app | `tooncraft` | egui app: craft-ui docking, GPU tile textures, timeline, panels, AccessKit | yes |

`cargo xtask layers` fails if any L0–L3 crate depends on egui, eframe, winit or rfd.

## Data flow while drawing

Pen events (every `PointerMoved`/`Touch` of the frame, not just the latest position) → canvas
coordinates through the inverse view transform → `Engine::stroke_to` → `StrokeEngine` (stabilizer,
spacing, dynamics) → `Stroke` (16-bit coverage per touched tile; pixels recomputed from the
pre-stroke tile so overlapping dabs never exceed the opacity) → the tiles' revisions change → the
canvas re-uploads only those tile textures. `end_stroke` turns the surface's tile capture into one
undo entry.

## Undo

Each entry is a list of self-inverse swaps (tile snapshots, the layer stack, a library drawing,
length, fps). Undo swaps them in and pushes the swapped-out state as the redo entry. Tiles are
`Arc`-shared, so a snapshot costs only the tiles that changed.

## Saving

`save`: write a journal → back up the previous file (if it loads) → atomic replace → read back and
compare → remove the journal. A journal found at open means an interrupted save; `check` reports
whether the file is good, else offers the backup. Autosave clones the project (pointer copy) and
encodes and writes on a background thread every 45 s while there are unsaved changes.
