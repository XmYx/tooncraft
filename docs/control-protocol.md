# Commands (CLI and agents)

Every change to a project is an engine `Command`, serialised as one JSON object per line with a
`cmd` tag. `tooncraft-cli run shot.toon commands.jsonl` applies a file (or `-` for stdin) and saves;
`#` lines are comments. The desktop app runs the same commands, so anything an animator does by
hand a script can do.

| `cmd` | Fields |
|---|---|
| `stroke` | `layer`, `frame`, `brush` (`kind`: pencil / ink / airbrush / chalk / eraser, `size`, `opacity`, `flow`, `hardness`, `spacing`, `grain`, `dynamics`, `smoothing`, `color`), `samples` (`x`, `y`, `pressure`, `tilt_x`, `tilt_y`, `time`, `rotation` in turns, optional) |
| `fill` | `layer`, `frame`, `x`, `y`, `color`, `options` (`tolerance`, `gap`, `spread`), `reference` (`layer` / `all_layers` / `reference_layers`) |
| `auto_paint` | `layer`, `from` (the painted drawing's frame), `frames` (the frames whose drawings to paint; each drawing once), optional `options`, `reference`: each enclosed empty area takes the colour the reference drawing has most under it; one undo step |
| `new_drawing`, `duplicate_drawing`, `extend_hold`, `shorten_hold`, `clear_cell`, `clear_drawing` | `layer`, `frame` |
| `insert_frame`, `delete_frame` | `frame`, optional `layer` (all layers when absent) |
| `retime` | `layer`, `from`, `to`, `step` |
| `set_exposure` | `layer`, `frame`, `length` (hold the drawing exactly that long) |
| `paste_cells` | `layer`, `frame`, `block` (rows of cells, bottom layer first), `insert`, `duplicate` |
| `clear_range` | `layers`, `from`, `to`, `shift` (remove the frames instead of blanking) |
| `reverse`, `cycle` | `layer`, `from`, `to` (+ `times` for `cycle`) |
| `import_images` | `paths`, `frame`, `hold`, `name` |
| `import_layers`, `import_gif` | `path` (PSD / PSB / OpenRaster, or GIF), `frame` |
| `import_video` | `path` (MP4 or QuickTime, H.264 or H.265), `frame` (default 0), `options` (`from_seconds`, `every`, `max_frames` (0 = as many as fit), `reference` (default true: bottom, locked, half opacity), `grow` (default true: lengthen the shot to fit)) |
| `import_sheet` | `path` (an XDTS sheet; drawings are read from the folders beside it) |
| `set_sound` | `path` (replaces every clip with this file; absent removes all sound), `offset` frames |
| `add_sound`, `remove_sound`, `move_sound` | `path`, `offset`, `track`; `index`; `offset` and `index` (default 0) |
| `set_sound_clip` | `index`, optional `track`, `offset`, `trim_start`, `length` (seconds), `to_end`, `fade_in`, `fade_out`, `volume` (0…4), `muted` |
| `set_volume_key`, `remove_volume_key` | `index`, `frame` (+ `gain` 0…4) |
| `set_mouth` | `layer`, `mouth` (`rest`, `closed`, `open`, `wide`, `round`, `pucker`, `teeth`, `tongue`, `consonant`), `drawing` (absent: forget it) |
| `lip_sync` | `layer`, `from`, `settings` (`silence` 0…1, `min_hold` frames) |
| `lip_sync_text` | `layer`, `from`, `text`, `settings`: the typed line's mouths over the voiced frames of the sound, or `min_hold` frames each without sound |
| `import_mouth_sheet` | `layer`, `path`, `from` |
| `set_audio_track` | `track`, optional `name`, `muted`, `solo`, `volume` |
| `transform_selection` | `layer`, `frame`, `polygon`, `dx`, `dy`, `scale_x`, `scale_y`, `angle` (radians); optional `feather` (soft edge, pixels, up to 64; also on `distort_selection`, `clear_selection` and `gradient`) |
| `mesh_warp_selection` | `layer`, `frame`, `polygon` (+ `rings`), `cols`, `rows` (1…16), `points` ((cols + 1) × (rows + 1), row by row: where the grid over the selection's box goes), optional `resample`, `feather` |
| `clear_selection`, `paste_pixels` | `polygon`; or `x`, `y`, `width`, `height`, `rgba` |
| `shape` | `layer`, `frame`, `brush`, `shape` (line / rectangle / ellipse), `from`, `to`, `constrain` |
| `fill_all_frames` | as `fill`, without `frame` |
| `set_camera_key`, `remove_camera_key`, `set_camera` | `frame`, `x`, `y`, `zoom`, `angle` (degrees); `enabled` |
| `add_camera`, `rename_camera`, `remove_camera`, `set_camera_cut`, `remove_camera_cut` | `add_camera`: `name`, optional `from` (a camera to copy); `rename_camera`: `camera`, `name`; `remove_camera`: `camera` (not 0; its cuts go, later cameras move down one); `set_camera_cut`: `frame`, `camera` (from that frame on, film through it); `remove_camera_cut`: `frame`. `set_camera_key`, `remove_camera_key`, `set_camera`, `add_camera_shake` and `remove_camera_shake` take an optional `camera` (0, the main camera, by default). At most 32 cameras besides the main one and 2000 cuts |
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
| `save_pose`, `apply_pose`, `rename_pose`, `remove_pose` | `save_pose`: `peg`, `frame`, `name` (the peg and every peg below it, as they are on that frame); `apply_pose`: `index`, `frame` (keys the pose's pegs the shot still has; refused when it has none); `rename_pose`: `index`, `name`; `remove_pose`: `index`. At most 512 poses |
| `set_peg_keys` | `peg`, `keys` (a list of keys with the fields of `set_peg_key`, `frame` first): set them all as one undo step (a recorded take); refused whole when a key is past the end of the shot or the peg would hold more than 10 000 keys |
| `set_layer_vector`, `vector_stroke`, `edit_vector` | `layer`, `vector`; `layer`, `frame`, `color`, `points` (`{x, y, width}`), `tolerance` (px, default 0.6), optional `style` (`cap`: `round` / `flat` / `square`, `join`: `round` / `sharp` / `cut`; round and round when absent); `layer`, `frame`, `edit` (`{"edit":"move_point","stroke","point","x","y"}`, `set_width` with `stroke`, optional `point`, `width`; `smooth` with `stroke`, `strength`, `passes`; `erase_to_crossings` with `stroke`, `x`, `y`; `delete_stroke`; `set_color`; `set_style` with `stroke`, `style`). Each is one undo step |
| `move_peg_keys`, `set_peg_eases`, `remove_peg_keys` | `peg`, `frames` (the keys); `by_frames`, optional `channel` (`x`, `y`, `angle`, `scale_x`, `scale_y`, `skew`) and `by_value`; `ease`. Each is one undo step; a move that would leave the shot or land on a key that is not moving is refused whole |
| `add_peg_for_layer` | `layer`, `name`: a new peg with the layer hung from it, one undo step |
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
| `set_layer_reference` | `layer`, `reference` (the fill and the wand look at it with `reference: reference_layers`) |
| `set_layer_adjustment` | `layer`, `adjustment` (its effects act on everything below it) |
| `add_layer_mask` | `layer`, `invert` (optional): an empty painted mask; painting goes into it |
| `set_layer_mask` | `layer`, `enabled`, `invert`, `editing` (each optional) |
| `remove_layer_mask` | `layer` |
| `edit_comp` | `edit` with `op`: `from_layers`, `set_enabled` (`enabled`), `add_node` (`kind`, `pos`), `remove_node` (`node`), `link` (`from`, `to`, `input`), `unlink` (`to`, `input`), `move_node` (`node`, `pos`), `set_node` (`node`, `kind`); node kinds `layer` (`layer` id), `effect` (`effect`), `blend` (`mode`, `opacity`), `mask` (`invert`), `output` |
| `batch` | `label`, `commands` (at most 256, no batch inside): one undo step; if one fails, the ones before it are undone |
| `add_layer` | `name`, optional `index` |
| `delete_layer` / `rename_layer` / `move_layer` / `set_layer` | `layer` (+ `name`, `from`/`to`, `visible`/`locked`/`opacity`) |
| `set_marker`, `remove_marker` | `frame`, `text` (empty removes it, 500 characters at most); `frame` |
| `set_drawing_name`, `expose_named` | `layer`, `frame`, `name` (names the drawing shown there; names are unique ignoring case); `layer`, `frame`, `name` (show that drawing at the frame, only that frame changes) |
| `edit_swatch` | `palette`, `swatch` (0-based, a shot palette), `rgb`, optional `name`, `update_paint` (default true: paint in the old colour on every unlocked layer takes the new one) |
| `swatch_fill` | `palette`, `swatch` (a shot palette's swatch), `layer`, `frame`, `x`, `y`, optional `options`, `reference`, `all_frames`: fill with the swatch's colour and link the paint to it, so `edit_swatch` with `update_paint` recolours only linked paint |
| `clone_stamp` | `layer`, `frame`, `offset` (source minus destination, pixels), `points` (the path, at most 20 000), optional `source_layer` (another layer's drawing at the same frame), `tip` (`size` 1…1000 px, `hardness` and `opacity` 0…1) |
| `liquify` | `layer`, `frame`, `points` (the path, at most 20 000), `mode` (`push` default, `twirl_left`, `twirl_right`, `pinch`, `bloat`), `brush` (`radius` 2…500 px, `strength` 0…1) |
| `gradient` | `layer`, `frame`, `kind` (`linear` / `radial`), `from`, `to`, `start`, `end` (RGBA), optional `area` and `area_rings` (polygons); optional `stops` (`at` 0…1 and `rgba`, at most 16; with two or more they replace `start` and `end`) |
| `set_palettes` | `palettes`: each a `name` and `swatches` (`name`, `rgb`, optional `alpha` (255 when absent) and `gradient` stops as for `gradient`) |
| `set_length`, `set_fps`, `set_paper`, `set_paper_texture` (`texture`: `kind`, `image`, `scale`, `angle`, `contrast`, `strength`), `set_onion`, `toggle_pin`, `set_light_table`, `purge_unused` | as named |
| `shift_pin` | `index`, `x`, `y` (canvas pixels), `turn` (tenths of a degree): shift and trace a light-table pin; a setting, not an undo step |
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
costs recorded in [performance.md](performance.md). `tooncraft-cli import`, `import-layers`, `import-gif`, `import-video`, `import-sheet` and `set-sound` bring work in.

An MCP server on craft-control is planned (see [roadmap.md](roadmap.md)).
