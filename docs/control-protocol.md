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
| `set_sound`, `move_sound` | `path` (absent removes it), `offset` frames |
| `transform_selection` | `layer`, `frame`, `polygon`, `dx`, `dy`, `scale_x`, `scale_y`, `angle` (radians) |
| `clear_selection`, `paste_pixels` | `polygon`; or `x`, `y`, `width`, `height`, `rgba` |
| `shape` | `layer`, `frame`, `brush`, `shape` (line / rectangle / ellipse), `from`, `to`, `constrain` |
| `fill_all_frames` | as `fill`, without `frame` |
| `set_camera_key`, `remove_camera_key`, `set_camera` | `frame`, `x`, `y`, `zoom`, `angle` (degrees); `enabled` |
| `set_layer_blend`, `set_layer_color`, `set_layer_folder`, `set_folder` | blend mode; colour; folder name; folder `visible`, `opacity`, `collapsed` |
| `add_tip`, `add_tip_from_image` | `id`, `name`, `width`, `height`, `alpha`; or `id`, `path` |
| `add_layer` | `name`, optional `index` |
| `delete_layer` / `rename_layer` / `move_layer` / `set_layer` | `layer` (+ `name`, `from`/`to`, `visible`/`locked`/`opacity`) |
| `set_length`, `set_fps`, `set_paper`, `set_onion`, `toggle_pin`, `set_light_table`, `purge_unused` | as named |

Example:

```json
{"cmd":"stroke","layer":0,"frame":0,"brush":{"kind":"ink","size":5},"samples":[{"x":100,"y":60},{"x":160,"y":80,"pressure":1}]}
{"cmd":"extend_hold","layer":0,"frame":0}
{"cmd":"fill","layer":0,"frame":0,"x":125,"y":120,"color":[230,90,70,255]}
```

`tooncraft-cli export` writes `--png <dir>`, `--gif`, `--mp4` (with the soundtrack), `--apng`,
`--prores <file.mov> [--alpha]`, `--psd <file> [--frame N]`, `--ora <file> [--frame N]`,
`--layers <dir>` (PNG sequence per layer) or `--levels <dir>` (XDTS sheet and drawing folders).
`tooncraft-cli import`, `import-layers`, `import-gif`, `import-sheet` and `set-sound` bring work in.

An MCP server on craft-control is planned (see [roadmap.md](roadmap.md)).
