// Put a note on the first frame of every second ("1s", "2s", …), keeping notes already there.
let marked = 0;
for f in range(0, length(), fps()) {
    if note(f) == () {
        run(#{cmd: "set_marker", frame: f, text: `${f / fps() + 1}s`});
        marked += 1;
    }
}
print(`marked ${marked} seconds`);
