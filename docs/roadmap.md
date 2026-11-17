# ToonCraft roadmap: milestones

> **Last updated:** 2026-11-17 · **Change:** major

**Current focus:** after the polish pass (timing tools, video export, versioned saves, large-shot
performance measured), the next items are pen input everywhere, the audio track and selection /
transform, ranked in [gaps.md](gaps.md) from [user-research.md](user-research.md).

Items marked **Ours** are where we plan to do better than the reference apps.

## M0: vertical slice (done, 2026-11-17)

- Tiled sparse canvas, GPU-composited, pan / zoom / rotate / mirror.
- Brush engine: pressure, tilt, velocity, smoothing and pulled-string stabilizer; pencil, ink,
  airbrush, chalk (paper grain), eraser; no overlap darkening.
- Layers with exposure sheets: keys, holds, blanks; insert / delete / duplicate / re-time.
  **Ours:** re-timing never deletes drawings.
- Onion skin (before / after, tint, falloff, by drawings or frames) and light table pins.
  **Ours:** onion silhouettes cached as GPU textures.
- Paint bucket with gap closing and paint-under-lines.
- Unlimited undo bounded by memory; **Ours:** journaled atomic saves with backup and background
  autosave recovery; property-tested loader and fill.
- PNG sequence, GIF, OpenRaster export. **Ours:** every action is a JSON command (`tooncraft-cli run`).

## M0.5: polish pass (done, 2026-11-17)

User research ([user-research.md](user-research.md)); exposure-sheet range selection, copy / cut /
paste (linked or as new drawings), drag-to-hold, reverse, cycles; image-sequence import; MP4 (H.264)
and animated PNG export with no external tools; Save New Version; Alt+arrow panning; Shift+click
straight lines; a 4K, 240-drawing stress shot measured at 24.1 fps with 0 dropped frames.

## M1: alpha (editing depth), about 110 h

Pen pressure everywhere (shared tablet crate), audio track with waveform and scrub, selection and
transform, remappable shortcuts, shape tools, timeline thumbnails.

## M2: beta (production), about 140 h

Camera keys and transform layers, layer blend modes / folders / colours, fill across frames, brush
tips and preset files, BoardCraft animatic import, PSD / WebP export, saved workspaces, translations,
paging to disk.
**Ours:** MCP / control channel on craft-control so agents can draw and time a shot.

## M3: beyond the references

GPU dab rasterisation (craft-gpu), AI in-between assist and AI cleanup through
craft-ai-integrations (labelled "AI …"), lip-sync from audio, effects through EffectCraft.
