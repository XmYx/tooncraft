# Performance

> **Last updated:** 2026-11-17 · **Change:** minor

Measured on one development workstation (12-core / 24-thread desktop CPU, 64 GB RAM, discrete
GPU, Linux Wayland, 60 Hz display), release build. Re-run them with the commands shown; numbers on
other machines will differ.

## Stroke latency (engine)

`cargo xtask bench` (= `tooncraft-cli bench`). Time from one pen sample to its pixels being in the
drawing (stabilizer, dab spacing, dab rasterisation, coverage buffer), 1920×1080 canvas, 400
samples per stroke 4 px apart (a quick stroke from a 240 Hz pen), 20 strokes.

| Brush | Mean per sample | Median | 99th percentile |
|---|---|---|---|
| Ink 6 px | 0.008 ms | 0.007 ms | 0.020 ms |
| Pencil 4 px (paper grain) | 0.014 ms | 0.013 ms | 0.031 ms |
| Chalk 30 px (grain, tilt) | 0.043 ms | 0.045 ms | 0.067 ms |
| Airbrush 120 px | 0.145 ms | 0.242 ms | 0.310 ms |
| Eraser 40 px | 0.016 ms | 0.015 ms | 0.037 ms |

In the app, the status bar shows the time from the first pen sample of a UI frame to the changed
tiles being queued for upload ("stroke … ms"). Only tiles a stroke touched are re-uploaded
(16 KB each). The input-to-photon latency is then one display refresh plus the compositor, which
we have not measured with a camera yet (see [gaps.md](gaps.md)).

## Playback (app, GPU compositing)

`tooncraft --demo --bench-playback 8`: the 1280×720, 12 fps, 3-layer demo shot, looping for 8 s.

| Measure | Run 1 | Run 2 | Run 3 |
|---|---|---|---|
| UI redraws per second | 60 | 60 | 59 |
| UI frame CPU time, mean | 0.42 ms | 0.41 ms | 0.40 ms |
| UI frame CPU time, 99th percentile | 0.81 ms | 0.69 ms | 0.79 ms |
| Animation frames shown per second (target 12) | 12.1 | 12.1 | 12.1 |
| Dropped animation frames | 0 | 0 | 0 |
| Redraw interval, 99th percentile | 19.1 ms | 21.9 ms | 30.8 ms |

Playback follows the wall clock, so a slow redraw would drop frames rather than slow down; the
dropped count measures that. Two earlier runs, with the window on a hidden workspace, showed the
compositor throttling redraws to 29–48 per second and dropping frames; that is the compositor, not
ToonCraft (CPU time per frame stayed under 1 ms), but it means playback numbers are only valid with
the window visible.

## Large shots

`tooncraft-cli demo large.toon --large` builds a 3840×2160, 24 fps, 240-frame shot: a held
background and a different ink drawing on every frame (241 drawings).

| Measure | Value |
|---|---|
| Build the shot through commands and save it | 1.1 s, 206 MB peak memory |
| File size | 6.4 MB |
| Load and inspect (`tooncraft-cli info`) | 0.12 s, 156 MB peak memory |
| Pixel memory of the drawings (sparse tiles) | 147 MB (vs. 7.9 GB as full 4K frames) |
| App start to first rendered frame with the shot open (`--screenshot`) | 0.67 s |
| App playback, 8 s: animation frames shown per second (target 24) | 24.1, 0 dropped |
| App playback: UI frame CPU time mean / 99th percentile | 0.73 ms / 3.13 ms |
| MP4 export, 48 frames at 4K (H.264, CRF 18) | 8.0 s (6 frames per second), 131 KB |

## Long shots: paging to disk

`tooncraft-cli demo long.toon --large --frames 2000` builds a 3840×2160, 24 fps, 2000-frame shot
(a held background and a different ink drawing on every frame: 2001 drawings, 1.16 GB of tiles in
memory when all are loaded). `tooncraft-cli scrub long.toon` loads it and steps through every frame
as playback does: it reads the drawings shown and pages out the rest past the budget
(`TOONCRAFT_PAGE_BUDGET_MB`). Peak memory comes from `/usr/bin/time -v`. Three runs each, release
build, on the machine described above. Other workstreams were building on the same machine at the
time, so timings are noisy.

| Measure | Page budget 256 MB | No paging (budget above the shot) |
|---|---|---|
| Build the shot through commands and save it | 11.6 s, 1.30 GB peak (undo history included) | — |
| File size | 52 MB | 52 MB |
| Load | 1.09 / 1.39 / 1.14 s | 1.44 / 1.61 / 1.25 s |
| Peak memory, load and scrub all 2000 frames | 317 / 317 / 317 MB | 1232 / 1233 / 1232 MB |
| Scrub, mean per frame (read the drawings shown, page the rest) | 0.22 / 0.39 / 0.34 ms | 0.003 / 0.005 / 0.005 ms |
| Scrub, slowest frame | 0.93 / 1.45 / 1.38 ms | 0.02 / 0.04 / 0.06 ms |
| Drawings in memory at the end | 195 MB, 1777 drawings paged | 1164 MB |
| Page file on disk | 1028 MB (removed on exit) | — |
| Open, run one command, save (journal, backup check, verify) | 3.6 / 4.9 / 5.1 s, 375 MB peak | 2.6 s, 1.32 GB peak |

App playback of the same shot, `tooncraft --bench-playback 8 long.toon` (8 s from frame 1, GPU
tile textures; peak memory of the whole app process, textures included):

| Measure | `TOONCRAFT_PAGE_BUDGET_MB=256` (3 runs) | Default budget, 1 GB (2 runs) |
|---|---|---|
| Animation frames shown per second (target 24) | 24.1 / 24.1 / 24.1, 0 dropped | 24.1 / 24.1, 0 dropped |
| UI frame CPU time, mean / 99th percentile | 1.33 / 4.59, 1.14 / 2.95, 1.55 / 5.40 ms | 1.09 / 1.84, 1.01 / 2.20 ms |
| Peak memory of the app | 557 / 557 / 557 MB | 1345 / 1346 MB |

The slowest paged frame (1.45 ms) is far inside the 41.7 ms a frame has at 24 fps. Saving reads
paged drawings back one at a time, and the save no longer decodes the whole old file to check it
before keeping it as the backup (`tooncraft_format::validate` drops each drawing after reading it).

## Engine (CPU)

| Measure | Value |
|---|---|
| Composite 1920×1080 frame with onion skin on the CPU (exports, fill reference) | 1.8 ms (≈ 550 fps) |
| Paint-bucket fill over 1920×1080 with 4 px gap closing | 36 ms |
| Save the demo shot (encode, journal, backup, atomic write, read-back verify) | 12 ms, 614 KB |
| Load the demo shot | 7 ms |
| Autosave snapshot (copy-on-write clone) | 0.02 ms |
| Demo shot pixel memory (25 drawings, sparse tiles) | 12.3 MB |

## Effects

The shot: `tooncraft-cli demo shot.toon --large --size 1920x1080 --frames 240` (a held background
and a different ink drawing on every frame), then `tooncraft-cli run shot.toon fx.jsonl` adding an
outline (3 px), a drop shadow (12 px offset, softness 8), a glow (radius 10) and levels to the ink
layer — the same stack `tooncraft-cli bench --effects` builds in memory. The machine was shared
with other builds while measuring (load average 20–32 on 24 hardware threads, noted per run);
numbers on a quiet machine will be better, and they vary from run to run.

CPU cost, `tooncraft-cli bench --effects [shot.toon]` (load ≈ 26):

| Measure | 1920×1080, 240 frames | 3840×2160, 240 frames |
|---|---|---|
| Composite one frame with the 4-effect stack, mean / median / 99th percentile | 18.4 / 19.4 / 24.0 ms | 42.2 / 41.1 / 52.3 ms |
| Frames composited per second by one thread | 54 | 24 |
| The same frames without effects | 0.31 ms | 1.26 ms |

Effects run on the drawn part of the layer plus how far the stack reaches (a property test checks
that this gives exactly the pixels of the whole canvas); before that change the same 1080p frame
took 176 ms. One effect on a whole 1920×1080 layer (the worst case: everything drawn): soft blur
10 ms, motion blur 12 ms, glow 15 ms, drop shadow 27 ms, light wrap 34 ms, outline 56 ms, thicken
56 ms, zoom blur 116 ms (it always covers the whole canvas); levels, curves, tint, grain, hard
edges, soft mask and colour key 0.4–0.8 ms. Large layers are split over threads; crops under
about half a megapixel run on one thread, which measured faster on the shared machine (one
thread: 18.5 ms per frame; all threads under load ≈ 50: 74–81 ms).

App playback, `tooncraft shot.toon --bench-playback S` with the window visible. While effects are
on, the canvas shows CPU-composited frames: they are cached per project version (1.5 GB budget)
and a worker thread renders the frames after the playhead, so playback shows finished frames.

| Run (240 frames, 24 fps, 4-effect stack) | Load / I/O wait | Frames shown per second | Dropped | Rendered by the worker / in the UI thread | UI frame CPU time, median / 99th percentile |
|---|---|---|---|---|---|
| 1920×1080, 10 s from a cold start | 17 / 0 % | 24.1 | 0 | 239 / 1 | 0.95 / 5.2 ms |
| 1920×1080, 15 s from a cold start | 21 / — | 24.1 | 0 | 239 / 1 | 0.73 / 4.7 ms |
| 1920×1080, 30 s (the first loop cold, then cached) | 16 / 1 % | 23.8 | 5 | 423 / 10 | 0.67 / 9.2 ms |
| 3840×2160, 30 s | 13 / 15 % | 23.3 | 21 | 211 / 29 | 0.82 / 60 ms |

Runs on this shared machine are not always this clean: while other builds filled the swap and
the disk (I/O wait near 30 %), the same 1080p shot played at 5–8 frames per second with a few
redraws a second. The plain demo shot (no effects, so no frames to upload) still redrew at 58
per second then; we have not separated swapping from texture uploads as the cause. In the 30 s
1080p run the worker rendered 423 frames for 240: the cache was refilled once during the run, and
we have not found why yet. At 4K the stack costs 42 ms a frame on one thread,
more than a 24 fps frame lasts, so a cold first loop drops frames until the worker is ahead. Turning a
composited frame into tiles for the GPU takes 2.2–2.6 ms at 1080p and about 8 ms at 4K.

## Memory bounds

- Drawings store only touched 64×64 tiles; an empty drawing costs nothing.
- Undo history is unlimited in steps and capped at 1.5 GB of tile data (oldest steps dropped).
- GPU tile textures are an LRU cache capped at 768 MB.
- The fill works on at most 40 million pixels (a window around the seed on bigger canvases).
- Files may expand to at most 4 GB when loaded; canvases are at most 16384 px a side, shots at most
  65 536 frames, 512 layers and 100 000 drawings.
