# The `.toon` file

A ZIP archive (PKWARE APPNOTE), written and read with craft-zip.

| Entry | Contents |
|---|---|
| `mimetype` | `application/vnd.storyteller.tooncraft`, first, stored |
| `project.json` | `{"format":"tooncraft","version":1,"app","saved_at","project":{name,width,height,fps,length,paper,layers,onion,light_table},"drawings":[{id,tiles}]}` |
| `drawings/<id>.tiles` | `TCT1`, u32 LE tile count, then per tile u32 LE tile index (row-major, 64 px tiles) and 16384 bytes of premultiplied RGBA8 |
| `audio/sound-<n>` | a sound file as imported (any format craft-audio decodes), once per file |
| `preview.png` | frame 1, at most 256 px |

`project` may also hold `pegs` (`id`, `name`, `parent`, `pivot`, `keys` with `frame`, `x`, `y`,
`angle`, `scale_x`, `scale_y`, `skew`, `ease`) and `camera.shakes`; a layer may name its `peg` and its
`depth`, and `clip` (clipped to the layer below). Folders may carry `locked`, `color` and `parent`
(the folder they are in). `project.marks` lists `{drawing, mark}` (`key`, `breakdown`, `inbetween`).
Older readers ignore these fields; loaders drop unknown parents, break loops and clamp values.

Layers list bottom to top; each has `id`, `name`, `visible`, `locked`, `opacity` and `cells`, one per
frame: `"blank"`, `"hold"` or `{"key": <drawing id>}`. A layer may have `effects`: a list, run in order, of
`{"enabled", "kind": {"type": "blur", "radius": 4, …}, "keys": [{"frame", "value": {…}}], "blend", "mix"}`; the
kinds and their parameters are `tooncraft_fx::EffectKind`, and loading clamps every parameter
and drops keys of another kind. `opacity_keys` (`[{"frame", "opacity"}]`) animate the opacity, and
`matte` (`"inside"` / `"outside"`) makes the layer a matte for the layers right below it. `saved_at` is RFC 3339 UTC (chrono).

Readers must refuse a higher `version` with a clear message, cap entries and expanded size, and
repair rather than reject inconsistent cells (holds with nothing to hold and keys to missing
drawings become blanks). ToonCraft's loader is property-tested with random corruption.

Sound: `sounds` lists the clips (`name`, `offset` frame, `track`, `trim_start`, `length`, `fade_in`,
`fade_out` in seconds, `volume`, `volume_keys` `[{"frame", "gain"}]`, `muted`, and the `entry` of
its file); `audio_tracks` holds each track's `name`, `muted`, `solo` and `volume`. Files written
before clips have one `soundtrack` (`name`, `offset`, `entry: "audio/soundtrack"`), which loads as a
single clip.

A layer's `mouth_chart` maps mouths (`rest`, `closed`, `open`, `wide`, `round`, `pucker`, `teeth`,
`tongue`, `consonant`) to drawing ids.

## Mouth sheets

Plain UTF-8 text, one item per line, `#` starts a comment, blank lines are ignored.

- **ToonCraft mouth sheet** (`.txt`): the first line is `tooncraft-mouths 1`; an optional
  `fps N` line gives the frame rate the frames count in; then `FRAME MOUTH` lines, one per change,
  frames from 1, mouths by the names above. Example: `tooncraft-mouths 1` / `fps 24` / `1 rest` /
  `5 open` / `9 closed`.
- **Switch data** (`.dat`): the first line is `MohoSwitch1`, then `FRAME NAME` lines with frames
  from 1 and the phoneme-group names `AI`, `E`, `O`, `U`, `WQ`, `MBP`, `FV`, `L`, `etc`, `rest`
  (read case-insensitively; our names are accepted too).

Readers refuse a missing header, frame 0, frames past 2^20 and unknown mouths with the line
number, and read at most 200 000 lines.
