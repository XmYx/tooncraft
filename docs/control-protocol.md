# Commands (CLI and agents)

Every change to a project is an engine `Command`, serialised as one JSON object per line with a
`cmd` tag. `tooncraft-cli run shot.toon commands.jsonl` applies a file (or `-` for stdin) and saves;
`#` lines are comments. The desktop app runs the same commands, so anything an animator does by
hand a script can do.

| `cmd` | Fields |
|---|---|
| `stroke` | `layer`, `frame`, `brush` (`kind`: pencil / ink / airbrush / chalk / eraser, `size`, `opacity`, `flow`, `hardness`, `spacing`, `grain`, `dynamics`, `smoothing`, `color`), `samples` (`x`, `y`, `pressure`, `tilt_x`, `tilt_y`, `time`) |
| `fill` | `layer`, `frame`, `x`, `y`, `color`, `options` (`tolerance`, `gap`, `spread`), `reference` (`layer` / `all_layers`) |
| `new_drawing`, `duplicate_drawing`, `extend_hold`, `shorten_hold`, `clear_cell`, `clear_drawing` | `layer`, `frame` |
| `insert_frame`, `delete_frame` | `frame`, optional `layer` (all layers when absent) |
| `retime` | `layer`, `from`, `to`, `step` |
| `set_exposure` | `layer`, `frame`, `length` (hold the drawing exactly that long) |
| `paste_cells` | `layer`, `frame`, `block` (rows of cells, bottom layer first), `insert`, `duplicate` |
| `clear_range` | `layers`, `from`, `to`, `shift` (remove the frames instead of blanking) |
| `reverse`, `cycle` | `layer`, `from`, `to` (+ `times` for `cycle`) |
| `import_images` | `paths`, `frame`, `hold`, `name` |
| `import_layers`, `import_gif` | `path` (PSD / PSB / OpenRaster, or GIF), `frame` |
| `import_sheet` | `path` (an XDTS sheet; drawings are read from the folders beside it) |
| `set_sound` | `path` (replaces every clip with this file; absent removes all sound), `offset` frames |
| `add_sound`, `remove_sound`, `move_sound` | `path`, `offset`, `track`; `index`; `offset` and `index` (default 0) |
| `set_sound_clip` | `index`, optional `track`, `offset`, `trim_start`, `length` (seconds), `to_end`, `fade_in`, `fade_out`, `volume` (0…4), `muted` |
| `set_volume_key`, `remove_volume_key` | `index`, `frame` (+ `gain` 0…4) |
| `set_mouth` | `layer`, `mouth` (`rest`, `closed`, `open`, `wide`, `round`, `pucker`, `teeth`, `tongue`, `consonant`), `drawing` (absent: forget it) |
| `lip_sync` | `layer`, `from`, `settings` (`silence` 0…1, `min_hold` frames) |
| `import_mouth_sheet` | `layer`, `path`, `from` |
| `set_audio_track` | `track`, optional `name`, `muted`, `solo`, `volume` |
| `transform_selection` | `layer`, `frame`, `polygon`, `dx`, `dy`, `scale_x`, `scale_y`, `angle` (radians) |
| `clear_selection`, `paste_pixels` | `polygon`; or `x`, `y`, `width`, `height`, `rgba` |
| `shape` | `layer`, `frame`, `brush`, `shape` (line / rectangle / ellipse), `from`, `to`, `constrain` |
| `fill_all_frames` | as `fill`, without `frame` |
| `set_camera_key`, `remove_camera_key`, `set_camera` | `frame`, `x`, `y`, `zoom`, `angle` (degrees); `enabled` |
| `import_boards` | `path` (a `.boardcraft` file), `frame` (start, default 0), `camera` (default true) |
| `resize_canvas` | `width`, `height`, `anchor` ([x, y] in 0…1, default centre); drawings move, never resample |
| `crop_canvas` | `x`, `y`, `width`, `height` |
| `set_mark` | `layer`, `frame`, `mark` (`key` / `breakdown` / `inbetween`; absent removes it) |
| `apply_timing` | `layer`, `from`, `to`, `preset` (`{"kind":"even","frames":2}`, `slow_start`, `slow_end`, `slow_both`, `{"kind":"holds","holds":[…]}`) |
| `retime_between_keys` | `layer`, `from`, `to`, `ease` (keys and breakdowns keep their frames) |
| `add_inbetweens` | `layer`, `frame`, `count` (1–32), `ease`, `guides` (default true) |
| `add_folder`, `rename_folder`, `set_folder_parent`, `set_folder_lock`, `set_folder_color` | `name` (+ `parent`, `layers`; `to`; `parent`; `locked`; `color`) |
| `set_layer_clip` | `layer`, `clip` (show only where the layer below has paint) |
| `add_peg`, `delete_peg`, `rename_peg`, `set_peg_parent`, `set_peg_pivot` | `name`, optional `parent`; `peg` (+ `name`, `parent`, `x`/`y`) |
| `set_peg_key`, `remove_peg_key` | `peg`, `frame`, `x`, `y`, `angle` (degrees), `scale_x`, `scale_y`, `skew` (degrees), `ease` (`{"kind":"linear"}`, `in`, `out`, `in_out`, `hold`, or `{"kind":"curve","x1","y1","x2","y2"}`) |
| `move_peg_keys`, `set_peg_eases`, `remove_peg_keys` | `peg`, `frames` (the keys); `by_frames`, optional `channel` (`x`, `y`, `angle`, `scale_x`, `scale_y`, `skew`) and `by_value`; `ease`. Each is one undo step; a move that would leave the shot or land on a key that is not moving is refused whole |
| `set_layer_peg`, `set_layer_depth` | `layer`, `peg` (absent: off the peg); `depth` (0 the drawing plane, above 0 further away) |
| `add_camera_shake`, `remove_camera_shake` | `shake` (`from`, `to`, `amplitude` px, `rotation` degrees, `frequency` per frame, `fade` frames, `seed`); `index` |
| `set_layer_blend`, `set_layer_color`, `set_layer_folder`, `set_folder` | blend mode (24: `normal`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `add`, `color_dodge`, `color_burn`, `hard_light`, `soft_light`, `difference`, `exclusion`, `linear_burn`, `linear_light`, `vivid_light`, `pin_light`, `hard_mix`, `subtract`, `divide`, `hue`, `saturation`, `colour`, `luminosity`); colour; folder name; folder `visible`, `opacity`, `collapsed` |
| `duplicate_layer`, `merge_down`, `merge_visible`, `flatten` | `layer`; `layer`; none; none (merges bake blend, opacity, pegs, mattes and effects; they refuse locked, hidden or matte-split layers) |
| `copy_layer_properties`, `set_layer_lock_alpha` | `from`, `to` (opacity and keys, blend, colour, folder, peg, depth, effects); `layer`, `lock` |
| `add_tip`, `add_tip_from_image` | `id`, `name`, `width`, `height`, `alpha`; or `id`, `path` |
| `add_effect` | `layer`, `effect` (`enabled`, `kind` with a `type` such as `blur`, `drop_shadow`, `levels`, `mask`, `colour_key`, and its parameters, `keys`, `blend`, `mix`), optional `index` |
| `set_effect`, `edit_effect`, `set_effect_key`, `remove_effect_key` | `layer`, `index`; `effect`; `frame` and `value` (`edit_effect` keys the frame when the effect has keys); `value` optional for `set_effect_key` |
| `enable_effect`, `remove_effect`, `move_effect` | `layer`, `index` (+ `enabled`; `from` / `to` for `move_effect`) |
| `set_opacity_key`, `remove_opacity_key` | `layer`, `frame` (+ `opacity` 0…1) |
| `set_layer_matte` | `layer`, `matte` (`inside` / `outside`; absent: a normal layer) |
| `add_layer` | `name`, optional `index` |
| `delete_layer` / `rename_layer` / `move_layer` / `set_layer` | `layer` (+ `name`, `from`/`to`, `visible`/`locked`/`opacity`) |
| `set_marker`, `remove_marker` | `frame`, `text` (empty removes it, 500 characters at most); `frame` |
| `set_drawing_name`, `expose_named` | `layer`, `frame`, `name` (names the drawing shown there; names are unique ignoring case); `layer`, `frame`, `name` (show that drawing at the frame, only that frame changes) |
| `set_length`, `set_fps`, `set_paper`, `set_onion`, `toggle_pin`, `set_light_table`, `purge_unused` | as named |
| `save_to_library` | `layer`, `from`, `to` (0-based frames, either order), `name`: the drawings and timing of those frames become a drawing-library item with its own copies of the drawings |
| `insert_from_library` | `item`, `layer`, `frame`, `linked` (default false: new copies; true: the layer shows the item's drawings, so editing one changes every linked use); cut at the end of the shot |
| `rename_library_item`, `move_library_item`, `remove_library_item` | `item` (+ `name`; `to`) |
| `save_library_file`, `open_library_file` | `item`, `path` (write a `.toondrawings` file; not an undo step); `path`, `placement` (`fit` default, `actual`, `fill`: drawings from another canvas size are centred) |

Example:

```json
{"cmd":"stroke","layer":0,"frame":0,"brush":{"kind":"ink","size":5},"samples":[{"x":100,"y":60},{"x":160,"y":80,"pressure":1}]}
{"cmd":"extend_hold","layer":0,"frame":0}
{"cmd":"fill","layer":0,"frame":0,"x":125,"y":120,"color":[230,90,70,255]}
```

`tooncraft-cli export` writes `--png <dir>`, `--gif`, `--mp4` (with the soundtrack), `--apng`,
`--prores <file.mov> [--alpha]`, `--prores-layers <dir>` (ProRes 4444 with alpha per layer), `--wav <file>` (the sound mix), `--mouth-sheet <file> [--layer N]` (a mouth sheet; `.dat` writes switch data), `--psd <file> [--frame N]`, `--ora <file> [--frame N]`,
`--layers <dir>` (PNG sequence per layer) or `--levels <dir>` (XDTS sheet and drawing folders).
`tooncraft-cli bench --effects [shot.toon] [--size WxH] [--frames N] [--threads N]` prints the effect
costs recorded in [performance.md](performance.md). `tooncraft-cli import`, `import-layers`, `import-gif`, `import-sheet` and `set-sound` bring work in.

An MCP server on craft-control is planned (see [roadmap.md](roadmap.md)).
