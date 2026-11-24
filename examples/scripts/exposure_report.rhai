// Print, for every layer, how many drawings it shows, how many frames are blank, and its holds.
for l in 0..layer_count() {
    let seen = [];
    let blank = 0;
    let changes = 0;
    let last = ();
    for f in 0..length() {
        let d = drawing_at(l, f);
        if d == () { blank += 1; } else if !seen.contains(d) { seen.push(d); }
        if f == 0 || d != last { changes += 1; }
        last = d;
    }
    print(`${layer_name(l)}: ${seen.len()} drawings, ${blank} blank frames, ${changes} exposures`);
}
