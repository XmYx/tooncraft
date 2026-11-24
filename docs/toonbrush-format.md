# The `.toonbrush` format

> **Last updated:** 2026-11-17 · **Format version:** 2

A `.toonbrush` file holds one brush and the images it needs, so a brush can move between shots and
machines. It is a ZIP archive (written and read through craft-zip) with these entries:

| Entry | Required | Content |
|---|---|---|
| `mimetype` | yes | `application/vnd.storyteller.tooncraft-brush`, stored uncompressed and first, so a reader can recognise the file from its first bytes |
| `brush.json` | yes | The brush settings, UTF-8 JSON (at most 1 MiB) |
| `tip.png` | no | The image tip: black ink whose alpha is the stamp's coverage |
| `grain.png` | no | The grain image: an opaque grey picture whose brightness is the surface height (light is a peak, dark a valley) |

Both images are scaled down to fit 1024 px when read. The writer is `tooncraft_engine::preset::Preset`;
`tooncraft-cli brush info` prints a file's settings and `brush make` / `brush preview` write a built-in
brush and render its preview stroke.

## `brush.json`

```json
{
  "format": "tooncraft-brush",
  "version": 2,
  "name": "Wash",
  "brush": { "kind": "watercolour", "size": 34.0, "...": "..." },
  "tip_name": "leaf",
  "grain_name": "weave"
}
```

`format` must be `tooncraft-brush`. A reader refuses a `version` newer than it knows with the
message "this brush preset needs a newer ToonCraft" and reads older versions. `tip_name` and
`grain_name` name the images in the archive and are optional.

### `brush`

Every field is optional; a missing field takes the value of the built-in Pencil brush, and every value
is clamped on reading (files are untrusted: out-of-range numbers are clamped, not rejected, and `NaN`
or non-numbers fall back to the default).

| Field | Range | Meaning |
|---|---|---|
| `kind` | `pencil`, `ink`, `airbrush`, `chalk`, `eraser`, `watercolour`, `smudge` | The brush family: how coverage builds up and what the brush paints |
| `size` | 0.5 … 1000 | Diameter in canvas pixels |
| `opacity` | 0 … 1 | Ceiling of the whole stroke |
| `flow` | 0.01 … 1 | Coverage each dab adds |
| `hardness` | 0 … 1 | 0 is a soft edge, 1 a hard edge |
| `roundness` | 0.1 … 1 | Tip shape: minor over major axis |
| `angle` | -180 … 180 | Tip shape: rotation in degrees (values wrap) |
| `spacing` | 0.02 … 2 | Distance between dabs as a fraction of the dab's diameter |
| `grain` | 0 … 1 | How strongly the grain shapes the dabs |
| `grain_scale` | 0.5 … 64 | Grain feature size in canvas pixels (for an image: canvas pixels per image pixel) |
| `grain_angle` | -180 … 180 | Grain rotation in degrees (turns the texture, not the dab; values wrap); left out when 0 |
| `grain_contrast` | 0.1 … 4 | Grain contrast: above 1 sharpens peaks and valleys, below 1 flattens them; left out when 1 |
| `tip_mode` | `stamp`, `pattern`, `ribbon` | How the image tip is laid along the stroke: stamped at every dab; copies end to end turned along the stroke (dabs one diameter apart); or stretched along the stroke as one band, the image repeating every brush width. Left out when `stamp` |
| `tip_cells` | 1 … 64 | The image tip is this many pictures side by side (an animated tip). Left out when 1 |
| `tip_order` | `sequence`, `random`, `frame` | Which cell each dab shows: in turn, at random (replays exactly), or the cell of the timeline frame drawn on. Left out when `sequence` |
| `grain_kind` | `paper`, `canvas`, `speckle`, `clouds`, `image` | Which texture; `image` uses `grain.png` (and falls back to `paper` when the file has none) |
| `color` | `[r, g, b, a]` | Straight (not premultiplied) 8-bit RGBA |
| `smoothing` | `average` 0 … 1, `string` 0 … 500, `catch_up` bool | Moving-average strength, pulled-string length in canvas pixels, and whether the line finishes where the pen lifts |
| `wet` | `pickup` 0 … 1 | Wet mixing, see below |
| `dynamics` | see below | Response curves |
| `compose` | `normal`, `behind`, `keep_alpha`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `add` | Where the paint goes relative to the paint already there, or how it blends with it; absent means `normal` |
| `jitter` | `hue` 0 … 0.5, `saturation` 0 … 1, `value` 0 … 1, `per_stroke` bool | Random colour change: the most each may move either way (hue in turns), per dab or once per stroke; absent means none |
| `count` | 1 … 32 | Dabs laid at each spacing step (a spray, with scatter); absent means 1 |

`tip` and `grain_image` (ids into a shot's tip table) are never written: the images travel in the
archive, and the id is assigned when the brush is put to use.

### `dynamics`

```json
"dynamics": {
  "tilt": 0.6,
  "min_size": 0.15,
  "response": {
    "size":    { "pressure": { "amount": 0.8, "curve": [[0, 0], [0.5, 0.3], [1, 1]] },
                 "speed":    { "amount": 0.25 } },
    "opacity": { "pressure": { "amount": 0.2 } },
    "scatter": { "random":   { "amount": 0.5 } }
  }
}
```

* `tilt` (0 … 1): how much the pen's lean flattens and turns the dab.
* `min_size` (0.01 … 1): the smallest size as a fraction of `size`, whatever the mappings say.
* `response`: for each **target** (`size`, `opacity`, `flow`, `scatter`, `angle`) a set of
  **sources** (`pressure`, `tilt`, `speed`, `rotation`, `random`, `barrel`), each with an `amount` (-1 … 1; 0 is off) and a
  `curve`. Only mappings that are on, or that have a curve other than the source's starting curve,
  are written. A missing `curve` is the source's starting curve: the identity for every source except
  `speed`, which starts as the reverse (`[[0, 1], [1, 0]]`, fast strokes give less).

Inputs are normalised to 0 … 1 before the curve is applied: `pressure` is the pen pressure, `tilt` is
how far the pen leans from upright, `speed` is the pointer speed (1 at 2500 px/s or more), `rotation` is
the stroke's heading in turns (0 is rightwards, 0.25 downwards), `barrel` is the pen's barrel rotation in
turns (0 … 1 clockwise, relative to the canvas; 0 for a mouse or a pen without a rotating barrel), and `random` is a fresh number for every
dab and target, taken from a hash of the stroke's first position and the dab's number, so replaying the
same stroke gives the same pixels.

`size`, `opacity` and `flow` are **scaled**: each source contributes the factor
`1 − amount · (1 − curve(input))` and the factors multiply, so amount 1 with the identity curve makes
the target follow the input and a negative amount reverses the effect. `scatter` and `angle` are
**added**: each source contributes `amount · curve(input)`, in brush diameters (scatter: a random
direction and that distance from the path) or turns (angle).

#### `curve`

A list of 2 to 6 control points `[x, y]`, both 0 … 1. The curve is a piecewise cubic Hermite
spline whose tangents are the weighted harmonic mean of the neighbouring slopes (Fritsch and Butland), so it
never overshoots its points. On reading, points are sorted, crowded points (closer than 0.02 in `x`) are
dropped, the first and last points are moved to `x = 0` and `x = 1`, extra points are thinned, and
fewer than two points give the identity.

Version-1 files wrote `pressure_size`, `pressure_opacity` and `velocity_size` (amounts) instead of
`response`; a reader maps them to the `size`/`pressure`, `opacity`/`pressure` and `size`/`speed`
mappings with the starting curves, so old files keep drawing the same way.

### `wet`

`pickup` (0 … 1) switches on **wet mixing** for every brush except the eraser. A wet brush carries a
load of paint and reads the colour of the layer under it (mean of 13 points inside the dab):

* On a painting brush the load starts as the brush colour and its colour mixes with the layer's colour
  as it goes. `pickup` is the fraction of the way to the layer's colour the load moves per brush width of
  travel; transparent canvas does not thin the paint.
* The `smudge` kind starts with the layer's colour and lets go of it gradually, so the colour is dragged
  along the stroke: `pickup` is how much it reloads per brush width (low values drag long smears). It
  paints nothing of its own.

In wet mode, flow and opacity apply per brush width of travel rather than per dab, so the look does not
depend on `spacing`, and the stroke is stamped onto the layer as it is (a dry brush is recomputed
from the layer as it was before the stroke, which is why overlapping dabs of a dry brush never darken a
line beyond its opacity). Wet brushes read the layer being painted on, not the other layers.

## Compatibility

* Readers ignore unknown fields in `brush.json`; writers add fields only with defaults that keep older
  brushes looking the same.
* A change that an older reader would draw wrongly raises `version`.
* `.toonbrush` files from version 1 (ToonCraft 0.1 before the brush editor) open in every later version.
