// Name the layers from the bottom up: "<prefix> 1", "<prefix> 2", …
// Arguments: prefix (default "Layer").
let prefix = if "prefix" in ARGS { ARGS.prefix } else { "Layer" };
for i in 0..layer_count() {
    run(#{cmd: "rename_layer", layer: i, name: `${prefix} ${i + 1}`});
}
print(`named ${layer_count()} layers`);
