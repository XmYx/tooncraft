# The `.toon` file

A ZIP archive (PKWARE APPNOTE), written and read with craft-zip.

| Entry | Contents |
|---|---|
| `mimetype` | `application/vnd.storyteller.tooncraft`, first, stored |
| `project.json` | `{"format":"tooncraft","version":1,"app","saved_at","project":{name,width,height,fps,length,paper,layers,onion,light_table},"drawings":[{id,tiles}]}` |
| `drawings/<id>.tiles` | `TCT1`, u32 LE tile count, then per tile u32 LE tile index (row-major, 64 px tiles) and 16384 bytes of premultiplied RGBA8 |
| `preview.png` | frame 1, at most 256 px |

`project` may also hold `pegs` (`id`, `name`, `parent`, `pivot`, `keys` with `frame`, `x`, `y`,
`angle`, `scale_x`, `scale_y`, `skew`, `ease`) and `camera.shakes`; a layer may name its `peg` and its
`depth`. Older readers ignore these fields; loaders drop unknown parents, break loops and clamp values.

Layers list bottom to top; each has `id`, `name`, `visible`, `locked`, `opacity` and `cells`, one per
frame: `"blank"`, `"hold"` or `{"key": <drawing id>}`. A layer may have `effects`: a list, run in order, of
`{"enabled", "kind": {"type": "blur", "radius": 4, …}, "keys": [{"frame", "value": {…}}], "blend", "mix"}`; the
kinds and their parameters are `tooncraft_fx::EffectKind`, and loading clamps every parameter
and drops keys of another kind. `opacity_keys` (`[{"frame", "opacity"}]`) animate the opacity, and
`matte` (`"inside"` / `"outside"`) makes the layer a matte for the layers right below it. `saved_at` is RFC 3339 UTC (chrono).

Readers must refuse a higher `version` with a clear message, cap entries and expanded size, and
repair rather than reject inconsistent cells (holds with nothing to hold and keys to missing
drawings become blanks). ToonCraft's loader is property-tested with random corruption.
