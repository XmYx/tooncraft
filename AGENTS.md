<!-- craft-house-rules:start core source=279b2c09ef2b2f0b4f5e616a47d39e29db0a1eb3 sha256=bee0ffafad196b68c76425770e344e588463bbda685ab41581fcbabb85dfc930 -->
## House rules

These apply in every Craft repository. A later rule may be stricter, not weaker. The source is `standards/house-rules-core.md` in `storytold/craft-contrib`. Propose changes there, not in a copy.

1. **A person is accountable.** Whoever runs an agent answers for what it opens and for its review comments.
2. **Check, then claim.** Before writing code, read the issue comments, search pull requests for `#N` and the keywords, and check recent commits on `main`. Comment on the issue first. Release a claim you will not finish.
3. **Verify, don't guess.** Open a definition before using it. Confirm the cause in the code, even when someone names a location.
4. **Keep fixes narrow and tested.** Fix what the issue describes, where the bug is. Add a test that fails without the fix.
5. **Review the diff before opening a pull request.** Identifiers exist as used, the test fails without the fix, nothing unrelated changed, and the description is true.
6. **Look for a better pull request.** If another fixes the same thing, close yours when it is better or equal, and say why.
7. **Report only what ran.** Say what you tested and what you could not run. Without a local build or test, say so and rely on CI before marking ready.
8. **`main` changes through a pull request.** Do not push to it. Branch protection enforces this. There is no exception.
9. **Stop and ask only about** licensing outside the allowlist, publishing, releases, a new remote, secrets, spending money, destructive git, and anything else hard to undo. Otherwise keep working. `plan/` is a gitignored notebook, not rules. Without it, do not ask for it and do not stop.
10. **Keep secrets and personal details out** of the repository, logs and requests.
11. **CI stays shared.** Workflows call `storytold/craft-common-actions`. Do not change CI in a feature or fix pull request. If CI cannot cover a change, open an issue there.
12. **Guards say why.** A check that looks removable says what it prevents. Read that before changing it.
13. **Instructions must be readable by everyone.** No notes about one machine, and no links to files a reader cannot open. `craftrules` is retired. Do not link to it.
14. **Keep pull requests moving.** Concurrent pull requests are fine; unanswered actionable feedback is not. Respond in 7 days, a reminder at 14, closure eligible at 30, from when an actionable request waits on the author. Suggestions and unconfirmed bot findings do not start the clock. A fix, explanation or agreed plan resets it; unrelated activity does not. Waiting on a reviewer or maintainer pauses it and does not count against the author. A maintainer decides closure; it is never automatic. Reuse search results, do not poll, and stop at a rate limit.
15. **No agent attribution.** A pull request does not say an agent wrote it. A repository rule may not require that.
16. **Shared libraries first for Craft apps.** Before adding or changing reusable file-format, codec, protocol or UI code, check `storytold/craft-libs` and relevant open pull requests. Reuse its public APIs or extend the appropriate shared library instead of duplicating an implementation in an app. Keep app-specific policy and model mapping local; preserve behavior, security and resource limits, attribution, layering and regression coverage. Document API gaps and maintainer-approved exceptions rather than silently dropping behavior. This does not authorize dependencies on sibling application crates or override existing platform, licensing or dependency-source requirements.
<!-- craft-house-rules:end -->
<!-- craft-house-rules:start rust source=279b2c09ef2b2f0b4f5e616a47d39e29db0a1eb3 sha256=56ef2b873de68940cf903069bbd0927b119f5c9ec2717f7f627041d2ebe8e4f2 -->
## Rust Craft repositories

These apply to native Craft apps and to shared Rust libraries. Select them by repository type. A repository that merely contains some Rust does not take this block. Rules later in a repository's file may be stricter, but not weaker. The source is `standards/house-rules-rust.md` in [storytold/craft-contrib](https://github.com/storytold/craft-contrib).

1. **Never crash.** Non-test code does not panic and has no `unsafe`, except where the repository file names an exception. Return errors, treat input as hostile, and bound recursion and allocations. New production crate roots copy the `deny` attribute from an existing crate: `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::unimplemented`, `clippy::todo`, `clippy::unreachable`. Every crash fix comes with a test that panicked before the fix.
2. **Clean room.** Learn from observed behaviour and public specifications. Do not copy another product's code, assets, strings or artwork. Check a dependency's licence before adding it.
3. **Assets and fonts.** Every asset is original or suitably licensed, and is recorded in `ATTRIBUTION.md` in the same change. Font files live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts), not in the app repository.
4. **Rust only for native Craft apps.** No handwritten JavaScript or TypeScript, and no Tauri, Electron or webview shell, in those apps. Shared tooling follows its documented technical scope and maintainer-approved exceptions. Craft PR Review's existing Tauri and TypeScript UI is one such exception. It does not relax this rule for native apps. Preserve any existing web build.
5. **Time goes through `chrono`.** Do not hand-roll calendar conversion. For dates beyond what `chrono` can represent, follow [standards/time.md](https://github.com/storytold/craft-contrib/blob/main/standards/time.md).
6. **Contributor credits.** Add or edit only your own person's entry in `contributors/people.toml`, and only after asking them. Never guess a name or hand-edit `contributors.json`.

## Licence allowlist

Dependencies and assets use SPDX identifiers. An expression is allowed when every identifier is in this list. A repository may name a shorter list. It may not add an identifier here by editing its own copy of these rules.

- `ISC`
- `MIT`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `Apache-2.0`
- `Unicode-3.0`
- `OFL-1.1`
- `Ubuntu-font-1.0`
- `Bitstream-Vera`
- `CC0-1.0`
- `CC-BY-4.0`
- `CC-BY-SA-4.0`
- `Zlib`
- `BSL-1.0`
- `0BSD`
- `Unlicense`
- `IJG`
- `NCSA`
- `CDLA-Permissive-2.0`
- `Apache-2.0 WITH LLVM-exception`
- `LicenseRef-Public-Domain`

`MPL-2.0` is allowed for third-party dependencies used unmodified; vendoring, patching or copying its code needs a maintainer's approval. `MPL-2.0+` counts as `MPL-2.0`.

`LicenseRef-Storyteller-Trademark` is not in this list. It covers Storyteller brand marks only, where a repository already says so.
<!-- craft-house-rules:end -->

# ToonCraft — rules for agents and contributors

The generated blocks above are the suite-wide house rules from
[storytold/craft-contrib](https://github.com/storytold/craft-contrib) (`standards/house-rules-core.md`
and `standards/house-rules-rust.md`); never edit them by hand, regenerate them with the house-rules
action. The rules below are ToonCraft's own. When they disagree with the house rules, the house
rules win. `CLAUDE.md` only includes this file.

## 0. Never crash

Animators trust ToonCraft with weeks of drawing. A malformed file, an odd pen event, a huge canvas,
a long shot or a full disk must give an error the user can act on, never a crash and never lost
work.

1. Fail with `Result`; the UI reports errors in the status bar and carries on.
2. No `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or `unimplemented!` outside tests. Every
   crate root carries the `#![deny(clippy::unwrap_used, …)]` line; `clippy.toml` allows them in tests.
3. No `unsafe` (`unsafe_code = "forbid"` for the workspace).
4. Every input-derived number is hostile: file JSON, tile data, brush settings, pen samples
   (NaN, huge, negative), command files. Sanitize (`Brush::sanitized`, `Sample::sanitized`,
   `Project::from_parts`) and cap work and memory (`MAX_SIDE`, `MAX_FRAMES`, `MAX_FILL_PIXELS`,
   the zip limits, the undo and texture budgets).
5. Saves are journaled and atomic (`tooncraft-format::save`): a failed save leaves the previous file
   intact; autosave runs on a background thread from a copy-on-write snapshot.
6. Parsers, the fill and the brush have property tests with hostile input; a crash fix comes with a
   regression test.

## 1. Assets

No artwork, icons, brushes, paper textures, sample files or screenshots from TVPaint, Toon Boom,
Clip Studio Paint, Procreate Dreams or any other commercial tool, in any form. Every asset is openly
licensed or original work and is listed in [`ATTRIBUTION.md`](ATTRIBUTION.md) in the same change;
`cargo xtask assets` checks it. Fonts live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts).
Screenshots in `docs/images/` show only ToonCraft with the built-in demo shot (`--demo`).

## 2. Clean room

Behaviour may be learned from the reference apps' public manuals, videos and observed use; never
read, disassemble or copy their code, private file formats or assets. Do not read code from GPL
animation tools (OpenToonz, Krita, Pencil2D, Synfig, Blender Grease Pencil). Formats come from
public specifications (ZIP, PNG, GIF, OpenRaster). Only [`docs/target-app-parity.md`](docs/target-app-parity.md)
names the reference apps.

## 3. Names

User-visible feature names are our own (craft-contrib [`standards/naming.md`](https://github.com/storytold/craft-contrib/blob/main/standards/naming.md));
the list and check record is [`docs/feature-names.md`](docs/feature-names.md). AI features are
labelled "AI …".

## 4. Engineering

- **Everything is a command.** Every change goes through `tooncraft_engine::Command` (one undo
  step each); `tooncraft-cli run` applies JSON-lines command files. New features add a command and a
  test.
- **Layering.** `tooncraft-raster`, `-brush`, `-doc`, `-format` and `-engine` never depend on egui,
  eframe, winit or rfd; `cargo xtask layers` enforces it.
- **Shared libraries first** (house rule 16, RFC-0040): ToonCraft uses craft-zip, craft-atomic,
  craft-image-codecs, craft-gif, craft-ora, craft-i18n, craft-fonts-build and craft-ui (docking).
  The tile and brush engines are candidates for a shared crate; see [`docs/gaps.md`](docs/gaps.md).
- **Accessibility.** Every control has an AccessKit role and translated name; the canvas and every
  visible exposure-sheet cell publish nodes; everything works from the keyboard (F6 between canvas
  and sheet, the keyboard pen); kittest UI tests run with `with_accessibility_check(true)`.
- **Localization.** Every user-visible string goes through `tl!`/`tlf!`; `cargo xtask strings --write`
  refreshes `locales/tooncraft.tsv`.
- **Time** goes through `chrono`.
- **Gates.** `cargo xtask ci` (fmt, clippy `-D warnings`, tests, layers, assets, progress fragments,
  parity table, string catalog, licences) before every commit. Toolchain: Rust ≥ 1.95, egui 0.37 from
  the suite fork (`[patch.crates-io]`).
- **Progress docs** follow RFC-0017: a PR adds one `docs/progress.d/<issue>-<branch-slug>.md`; it
  never edits the `ROADMAP.md` status line. Parity numbers come from `cargo xtask parity`.
- **Parity table.** A feature PR flips rows in `docs/target-app-parity.md` (D/X/P/M/O; sizes and
  area weights are in the map, which is scored by row and weighted parity) and must not
  commit the generated block between the `parity:` markers. `cargo xtask ci` validates the rows
  strictly but only warns when the block is stale. A maintainer refreshes it with
  `cargo xtask parity --write` in a separate PR.
- **Build output** stays in the repository's `target/`.

## See also

[README.md](README.md), [ROADMAP.md](ROADMAP.md), [docs/gaps.md](docs/gaps.md),
[docs/roadmap.md](docs/roadmap.md), [docs/target-app-parity.md](docs/target-app-parity.md),
[docs/architecture.md](docs/architecture.md), [docs/file-format.md](docs/file-format.md),
[docs/control-protocol.md](docs/control-protocol.md), [docs/performance.md](docs/performance.md).
