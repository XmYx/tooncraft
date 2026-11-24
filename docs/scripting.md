# Scripting

ToonCraft embeds [Rhai](https://rhai.rs) 1.27 (`MIT OR Apache-2.0`), a small scripting language
written in Rust, in the `tooncraft-script` crate. ModelCraft made the same choice after measuring
Rhai against Rune (see its `docs/scripting.md`). Rhai's 1.x API is stable. Its map literals are the
JSON form of a command, and every error carries a line and column. The crate has no UI.

```
tooncraft-cli run-script shot.toon examples/scripts/number_layers.rhai --arg prefix=Level
tooncraft-cli batch-script examples/scripts/mark_seconds.rhai a.toon b.toon c.toon --out-dir marked
```

```rhai
// Number every drawing exposure on the top layer as a note, on twos
let top = layer_count() - 1;
for f in range(0, length(), 2) {
    if drawing_at(top, f) != () {
        run(#{cmd: "set_marker", frame: f, text: `key ${f + 1}`});
    }
}
```

## Model

- **Changes are commands.** `run(#{cmd: …, …})` applies one `tooncraft_engine::Command`, written
  as its JSON form (the same form `tooncraft-cli run` reads, see
  [control-protocol.md](control-protocol.md)). A script can do whatever a command can and nothing
  else.
- **Reading.** `shot()`, `layers()`, `name()`, `width()`, `height()`, `fps()`, `length()`,
  `layer_count()`, `layer_name(i)`, `layer_visible(i)`, `layer_locked(i)`, `drawing_at(layer,
  frame)` and `note(frame)`. Frames count from 0 and layers from the bottom. The table is
  `tooncraft_script::API`.
- **Arguments.** `--arg NAME=VALUE` arrives in the constant map `ARGS` as strings.
- **One undo step.** A run that finishes is one undo step, named after the script. When it
  fails, through an error it does not catch or a limit, the shot and its undo history go back
  exactly as they were. A command the engine refuses is a script error that `try`/`catch` can
  handle. A failed command changes nothing (engine rule).

## Sandbox and limits

A script cannot read or write files, open sockets, `import` modules or `eval` text. `Limits` caps:

| Limit | Default |
|---|---|
| Interpreter operations | 10 million |
| Wall-clock time (checked as it runs and before every command) | 10 s (`--time-limit`) |
| Call depth, expression depth | 24, 48 |
| String bytes, array length, map size | 256 KiB, 20 000, 2 000 |
| Commands per run | 100 000 |
| Output lines kept | 5 000 |
| Script size | 1 MiB |

The tests run hostile scripts (endless loops, runaway recursion, string and array growth, too
many commands, `eval`, `import`) and a property test feeds arbitrary text to the parser. Each case
must stop with an error and leave the shot unchanged.

## Example scripts

`examples/scripts/` holds original scripts. Each one runs against the demo shot in the CLI tests,
and the test fails when a script has no test.

| Script | Does | Arguments |
|---|---|---|
| `number_layers.rhai` | Names the layers `<prefix> 1`, `<prefix> 2`, … from the bottom | `prefix` |
| `mark_seconds.rhai` | Notes the first frame of every second, keeping existing notes | |
| `exposure_report.rhai` | Prints drawings, blank frames and exposures per layer | |
