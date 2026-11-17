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
| BoardCraft | OTIO + stills | Planned (gap 11) | An animatic as a held "Layout" layer |

XDTS note: the sheet layout used is the one written by several animation tools (a first line
`exchangeDigitalTimeSheet Save Data`, then JSON with time tables, fields, tracks and frames). No
formal public specification was found; the implementation follows the format's documented
structure and treats other `SYMBOL_*` marks as holds.
