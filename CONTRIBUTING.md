# Contributing to ToonCraft

Read [AGENTS.md](AGENTS.md) first: the suite's house rules and ToonCraft's own (never crash, clean
room, assets, names, layering, commands).

1. Rust 1.95 or newer through `rustup`.
2. `cargo run -p tooncraft -- --demo` to try the app.
3. `cargo xtask ci` before every commit.
4. Changes to `main` go through pull requests; each adds a `docs/progress.d/` fragment.
5. New assets go in [ATTRIBUTION.md](ATTRIBUTION.md) in the same change.
6. Screenshots for `docs/images/`: `cargo run -p tooncraft -- --demo --screenshot docs/images/<name>.png`.
