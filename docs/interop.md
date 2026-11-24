# Interop: working with other apps

ToonCraft reads and writes public formats, so a shot moves between it and other animation,
paint, compositing and editing apps without lock-in. Everything is built in (craft-libs codecs, no
external tools) and tested by round trips in `crates/engine/src/tests.rs` and `crates/interop`.

| To / from | Format | ToonCraft side | Notes |
|---|---|---|---|
| Paint apps (layers) | PSD / PSB | Import layers; export a layered PSD of a frame, or of many frames with a layer group per frame (PSB for huge canvases) | Names, opacity, visibility; groups flattened on import; blend modes, masks and effects are not carried |
| Paint apps (layers) | OpenRaster `.ora` | Import layers; export a frame | |
| Animation apps (timing) | XDTS exposure sheet + one folder of PNG drawings per layer | Import and export | The sheet's track names are the layer names, cell values the drawing numbers (`<Layer>/<Layer>_0001.png`); timing round-trips exactly |
| Compositing | ProRes 4444 with alpha (MOV), ProRes 422 HQ | Export | Through camera, transparent background with 4444 |
| Compositing | PNG sequence per layer, transparent | Export | `<Layer>/<Layer>_0001.png`, one per frame |
| Editing, review | MP4 (H.264 + AAC soundtrack) | Export | |
| Web, chat | GIF, animated PNG | Export; GIF import (timed by its delays) | |
| Any | PNG / JPEG / WebP / TIFF image sequences | Import as drawings; PNG, TGA and TIFF sequence export; animated and still WebP export | |
| Sound | WAV, AIFF, MP3, AAC / M4A, FLAC, Ogg Vorbis, Opus | Soundtrack import | Stored as the original file in the shot |
| BoardCraft | `.boardcraft` board or animatic | Import as the layout | See below |

XDTS note: the sheet layout used is the one written by several animation tools (a first line
`exchangeDigitalTimeSheet Save Data`, then JSON with time tables, fields, tracks and frames). No
formal public specification was found; the implementation follows the format's documented
structure and treats other `SYMBOL_*` marks as holds.

## BoardCraft boards as the layout

File ▸ Import from Other Apps ▸ BoardCraft Board or Animatic…, or the `import_boards` command,
reads a `.boardcraft` file as BoardCraft documents it (`docs/file-format.md` in BoardCraft: a ZIP
with `project.json`, format `boardcraft.project`, schema 1 or 2, and `layers/<id>.png`):

| BoardCraft | ToonCraft |
|---|---|
| each panel, scenes in order | one drawing on a new, locked "Boards: <name>" layer at the bottom (half opacity), marked as a key |
| panel layers (visible ones, with their opacity) | flattened into that drawing, scaled to fit the canvas and centred |
| `panel.duration` | the drawing's hold; the shot grows to fit the board |
| camera keys (`frame`, `center`, `zoom`, `rotation`) | camera keys at the panel's start plus `frame`, mapped through the same fit; the camera is switched on |
| annotations, sound clips, dialogue and notes | not read |

**Checked against BoardCraft:** the reader is tested on a board written by hand in that format
(`tooncraft_interop::boards::demo`), not yet on a file saved by BoardCraft itself.

## Photoshop brushes (`.abr`)

Brushes ▸ **Import Brushes (.abr)…**, or `tooncraft-cli brush import-abr <file.abr> <dir>`, reads a
Photoshop brush file with the shared `craft-psd` reader (legacy v1 and v2, and the sectioned v6 and
later layout, sampled and computed tips). Each brush becomes a ToonCraft brush:

* a sampled tip becomes an image tip (its value is paint amount; tips over 1024 px are scaled down
  by an integer factor), sized to the tip or to the preset's diameter, with the preset's spacing
  and angle;
* a computed tip becomes the round tip with the file's diameter, hardness, angle, roundness and
  spacing.

Dynamics, textures and dual brushes in a preset are not read: the brush starts from the ink pen's
settings. In the app the brushes join the library under a category named after the file (or the
category typed beside the button); the command line writes one `.toonbrush` file per brush.
Presets whose tip image is missing are skipped with a warning, and a damaged file is an error, never
a crash. At most 1000 brushes are taken from one file.

