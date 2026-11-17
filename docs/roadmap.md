# ToonCraft roadmap: milestones

> **Last updated:** 2026-11-17 · **Change:** major

**Current focus:** the vertical slice is in (drawing, timeline, onion skin, light table, fill,
playback, undo, crash-safe saves, PNG/GIF/ORA export). Next: input and editing depth (M1).

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

## M1: alpha (editing depth), about 120 h

Pen pressure everywhere (shared tablet crate), selection and transform, cell copy / paste, shape
tools, keyboard panning, cycles and reverse range, layer folders and blend modes, image-sequence
import, customisable shortcuts.

## M2: beta (production), about 160 h

Camera keys, audio track with waveform and scrub, video export through FilmCraft codecs, BoardCraft
animatic import, APNG / WebP / PSD export, saved workspaces, translations, paging to disk.
**Ours:** MCP / control channel on craft-control so agents can draw and time a shot.

## M3: beyond the references

GPU dab rasterisation (craft-gpu), AI in-between assist and AI cleanup through
craft-ai-integrations (labelled "AI …"), lip-sync from audio, effects through EffectCraft.
