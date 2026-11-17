# ToonCraft roadmap

**Stage: pre-alpha** · next: alpha (see [docs/roadmap.md](docs/roadmap.md))

> **Last updated:** 2026-11-17 · **Change:** major · **Target:** raster frame-by-frame animation in the
> class of the commercial hand-drawn animation tools ([docs/target-app-parity.md](docs/target-app-parity.md))

## Headline numbers

| | Value | Kind |
|---|---|---|
| Parity score, feature rows (D + ½·P) / rows | 65 % (75 D, 17 P, 36 M of 128); 64 % on the original 124 rows, up from 59 % | measured (`cargo xtask parity`) |
| Parity weighted by effort (depth of each area) | about 20 % | estimated |
| Tests | 83 (core and property tests, CLI end to end, 9 kittest UI flows) | measured |
| 4K, 240-drawing shot: playback / load | 24.1 fps at 24, 0 dropped / 0.12 s | measured |
| Ink stroke latency per pen sample, 1920×1080 | 0.008 ms mean, 0.020 ms 99th percentile | measured |
| Playback, 12 fps demo shot | 12.1 fps shown, 0 dropped, 0.4 ms UI CPU per frame | measured |

The row score counts every feature equally; video export, audio, effects and camera are each one
row but most of the remaining work, which is why the effort-weighted estimate is much lower.

## By dimension

| Dimension | Today | Next |
|---|---|---|
| Drawing | 5 brushes with pressure / tilt / velocity, smoothing, steady line, paper grain, eraser, picker | pen input everywhere, selection and transform, shape tools, brush editor |
| Timeline | keys / holds / blanks, drag-to-hold, range select, copy / paste, reverse, cycles, re-time | audio waveform, camera, thumbnails |
| Onion skin and light table | before / after, tint, falloff, by drawings or frames, pins | outline mode |
| Colouring | fill with gap closing, paint under lines, swatches | fill all frames, palette files |
| Files | journaled saves, versions, autosave; MP4 / GIF / APNG / PNG / ORA export; image import | PSD, WebP, BoardCraft import |
| Accessibility | AccessKit canvas and grid cells, keyboard pen, F6, kittest checks | keyboard panning, high-contrast theme |
| Languages | English, catalog ready (257 strings) | craft-i18n catalogs |

## Features

See [docs/target-app-parity.md](docs/target-app-parity.md) and the ranked [docs/gaps.md](docs/gaps.md).

## Languages

English only. Translations will come from [craft-i18n](https://github.com/storytold/craft-i18n)
(`CRAFT_I18N_DIR`), as in the other Craft apps.

## Agents: CLI and MCP

`tooncraft-cli` creates, scripts (JSON-lines commands), checks, exports and benchmarks shots
([docs/control-protocol.md](docs/control-protocol.md)). MCP is planned.

## Progress log

Fragments in [docs/progress.d/](docs/progress.d/) (RFC-0017); `cargo xtask progress` prints them.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-17 | major | First vertical slice |
| 2026-11-17 | major | Polish pass: user research, timing tools, video export, large-shot numbers |
