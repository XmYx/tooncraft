# User research: what animators complain about and ask for

> **Last updated:** 2026-11-17 · **Change:** major

Method: about 25 web searches and page reads in November 2026 across app-store review roll-ups,
vendor forums and feature boards, the KDE bug tracker, krita-artists, mailing lists, Reddit mirrors,
review sites and release notes, for TVPaint Animation, Toon Boom Harmony, Clip Studio Paint, Procreate
Dreams, Krita (animation), OpenToonz, Rough Animator, FlipaClip and Callipeg. Reviews only: no source
code of any app was read. Everything is paraphrased; quotes are under 15 words.

How to read it: "N sources" is the number of distinct pages or threads; a roll-up page of many reviews
counts once (its internal count in brackets). Reddit, G2 and the Toon Boom forum often refused automated
reads, so those came through mirrors and search snippets; many forum threads date from 2016–2023, and
problems since fixed by the vendors are noted. Forums over-represent problems and store reviews
over-represent hobbyists.

Status: **Avoided** (by design, with evidence), **Have** (built and tested), **Built now** (this pass),
**Partial**, **Planned** (in [roadmap.md](roadmap.md) / [gaps.md](gaps.md)), **Rejected** (and why).

## Top annoyances

| # | Annoyance (paraphrased) | Seen in | Example sources | ToonCraft |
|---|---|---|---|---|
| A1 | Playback stutters and memory runs out as frames, canvas size or layers grow | ~10 sources (Krita strongest; Callipeg, FlipaClip, Dreams, OpenToonz) | [KDE 446472](https://bugs.kde.org/show_bug.cgi?id=446472), [FlipaClip help](https://support.flipaclip.com/article/85-playback-preview-slow-or-skipping-frames) | **Avoided:** sparse copy-on-write tiles and GPU tile textures; a 4K, 240-drawing shot plays at 24.1 fps with 0 dropped frames and loads in 0.12 s ([performance.md](performance.md)) |
| A2 | Crashes that take unsaved work with them | ~8 (FlipaClip roll-up [~7/30], Callipeg, OpenToonz, Dreams) | [OpenToonz group](https://groups.google.com/g/opentoonz_en/c/TTQC4Bb-gRg), [Dreams reviews](https://apps.apple.com/us/app/id1456172656) | **Avoided:** never-panic lints in every crate, property-tested loader, brush and fill; autosave every 45 s on a background thread with recovery on start |
| A3 | Exports stall, fail with cryptic external-tool errors or lose transparency | ~8 (Dreams, Krita, OpenToonz, Rough Animator) | [KDE 485860](https://bugs.kde.org/show_bug.cgi?id=485860), [Rough Animator reviews](https://justuseapp.com/en/app/954718244/roughanimator-animation-app/reviews) | **Built now:** every encoder is built in (no external ffmpeg): MP4 (H.264), GIF, animated PNG, PNG sequence and OpenRaster, all tested; PNG and APNG keep transparency |
| A4 | Subscription and online-licence lock-in | ~8 (Clip Studio 2.0, Harmony, TVPaint activation) | [CSP licence change](https://piunikaweb.com/2023/06/26/clip-studio-paint-users-irked-as-celsys-v2-0-license-deactivates-v1/) | **Avoided:** MIT OR Apache-2.0, works offline, no account |
| A5 | Pen lag, missing pressure, tablet driver fights | ~10 (Harmony forum dominant; OpenToonz, FlipaClip, Callipeg) | [Harmony forum](https://forums.toonboom.com/t/is-there-a-solution-for-lag-with-wacom-tablets/13268) | **Have:** 0.008 ms per pen sample for ink, latency does not grow with drawing size (tiles); pressure, tilt and the eraser end on X11, macOS and Windows Ink. **Partial:** no pen data in Wayland-native windows yet (gap 1) |
| A6 | Paywalls and hard caps (24-frame limit, layer caps, trial cannot save) | ~6 | [CSP FAQ](https://support.clip-studio.com/en-us/faq/articles/20190136) | **Avoided:** no tiers; only documented technical limits (65 536 frames, 512 layers, 16384 px) |
| A7 | Timeline clumsy for frame-by-frame: changing holds, selecting ranges, moving blocks | ~6 (Dreams, CSP, Krita, OpenToonz, FlipaClip) | [CSP ask](https://ask.clip-studio.com/en-us/detail?id=106671), [KDE 454716](https://bugs.kde.org/show_bug.cgi?id=454716) | **Built now:** drag a drawing's cells to change its hold, range selection (Shift+click, Shift+arrows), copy / cut / paste (linked or as new drawings), insert-paste, reverse, repeat, delete range — all one undo step each and keyboard-driven |
| A8 | Audio second-class: no waveform, poor scrubbing, sync drift | ~7 (Krita, CSP, Rough Animator, FlipaClip, Callipeg) | [KDE 393140](https://bugs.kde.org/show_bug.cgi?id=393140) | **Built:** soundtrack (WAV, AIFF, MP3, AAC, FLAC, Ogg, Opus) saved in the shot, waveform in the timeline, frame-by-frame scrub, playback kept in sync, AAC in the MP4 |
| A9 | Undo that removes the wrong thing or cannot be trusted | ~4 | [OpenToonz thread](https://redlib.astrial.org/r/OpenToonz/comments/1hsilh2/problem_with_opentoonz) | **Avoided:** every change is one command with a self-inverse undo; a property test applies random command sequences and undoes back to the exact start |
| A10 | Onion skin too limited (cap of a few frames) or confusing | ~4 | [OpenToonz wishlist](https://groups.google.com/g/opentoonz_en/c/hNm5fP-J7rM) | **Have:** up to 10 before and 10 after, per-side tint, falloff, by drawings or by frames, one or all layers, saved in the file |
| A11 | The animation app lacks basic drawing tools (selection, transform, lines, shapes) | ~5 | [Dreams "missing" thread](https://redlib.hbubli.cc/r/ProCreate/comments/181in7d/procreate_dreams_everything_its_missing_and) | **Built:** lasso and rectangle selection with live move / scale / rotate / flip, shape tools, Shift+click lines |
| A12 | Paint bucket leaks through tiny gaps and leaves halos | 2 sources, but [~10/30] reviews in one roll-up | [FlipaClip reviews](https://justuseapp.com/en/app/1101848914/flipaclip-create-2d-animation/reviews) | **Avoided:** gap closing up to 32 px and spread under lines, property-tested |
| A13 | Steep learning curve, dated or dense UI | ~4 | [SelectHub](https://www.selecthub.com/p/animation-software/opentoonz/) | **Partial:** familiar layout (tools left, timeline bottom, panels right), every action in a menu with its shortcut, a shortcut sheet; onboarding planned |
| A14 | Save model that silently drops drawings | 2 | [r/OpenToonz](https://pol1.lr.ggtyler.dev/r/opentoonz) | **Avoided:** one `.toon` file holds every drawing; saves are journaled, read back and verified, with a backup |
| A15 | Project files balloon because history is stored | 2 | [Dreams storage thread](https://reddit.birdcat.cafe/r/ProcreateDreams/comments/1hyln2w/storage_concern/m6iumac/?context=3) | **Avoided:** history is never saved; the 4K, 240-drawing stress shot is 6.4 MB |
| A16 | Hidden device-dependent limits (tracks stop rendering) | 2 | [Dreams help](https://help.procreate.com/articles/drvekr-track-render-limits) | **Avoided:** limits are documented and refusals show a message; memory budgets refuse instead of silently dropping |
| A17 | Missing shortcuts, cannot remap | 2 | [Harmony thread](https://forums.toonboom.com/t/flipping-marked-drawings-via-keyboard-non-drawing-hand/13374) | **Built:** every tool and timeline key remappable (craft-keymap), saved per user |
| A18 | Touch-gesture conflicts and palm marks | 2 | [Callipeg reviews](https://apps.apple.com/us/app/id1456172656) | **Rejected for now:** desktop-first; pinch zoom and twist rotate work, palm rejection depends on tablet support |
| A19 | Ads and watermarks in free tiers | 3 | [Filmora review](https://filmora.wondershare.com/video-editor-review/flipaclip-review.html) | **Avoided:** none, ever |
| A20 | Animation treated as a side feature of a paint app | 2 | [CSP ask](https://ask.clip-studio.com/en-us/detail?id=106671) | **Avoided:** a dedicated animation app |

## Most-requested features

| # | Request | Seen in | Example sources | ToonCraft |
|---|---|---|---|---|
| F1 | Audio waveform on the timeline with clean scrubbing for lip sync | ~7 | [krita-artists](https://krita-artists.org/t/seeing-the-audio-file-in-the-timeline/19987) | **Built** (A8); lip-sync assist planned (gap 3) |
| F2 | Richer onion skin (more frames, per-side colours, drawing-based mode, saved) | ~5 | [OpenToonz wishlist](https://groups.google.com/g/opentoonz_en/c/hNm5fP-J7rM) | **Have**; "hide only the previous frame" and outline mode planned |
| F3 | Lasso / selection / transform inside the animation workflow | ~5 | [Dreams thread](https://redlib.hbubli.cc/r/ProCreate/comments/181in7d/procreate_dreams_everything_its_missing_and) | **Built** (A11) |
| F4 | Straight-line and shape assist | ~4 | [review](https://thoughtsonthrees.substack.com/p/procreate-dreams-actually-works-now) | **Built:** line, rectangle, ellipse tools; Shift snaps lines to 15° and makes squares / circles |
| F5 | Perpetual, offline licence | ~5 | [CSP offline demand](https://piunikaweb.com/blog/clip-studio-paint-perpetual-license-users-demand-offline-access) | **Have** (open source) |
| F6 | Drag to change holds, ripple edit, move several blocks | ~3 | [CSP ask](https://ask.clip-studio.com/en-us/detail?id=106671) | **Built now** (see A7) |
| F7 | Custom and editable brushes | ~4 | [Dreams 1.0.7 notes](https://alternativeto.net/news/2023/12/procreate-dreams-1-0-7-brings-enhanced-layer-management-brush-import-and-more) | **Built:** image tips and `.toonbrush` preset files with a library; curve editor planned |
| F8 | Smart fill: gap closing, one object, fill across frames | 2–3 | [FlipaClip reviews](https://justuseapp.com/en/app/1101848914/flipaclip-create-2d-animation/reviews) | **Built:** gap closing and fill across every drawing of a layer |
| F9 | Faster long scenes, lower memory | ~3 | [krita-artists](https://krita-artists.org/t/animation-playback-lag/4017) | **Have** (A1) |
| F10 | Reliable export choices (sequences, transparency, video) | ~3 | [OpenToonz wishlist](https://groups.google.com/g/opentoonz_en/c/hNm5fP-J7rM) | **Built now** (A3); ProRes possible later through craft-video |
| F11 | Remappable shortcuts, one-handed flipping | 2 | [Harmony thread](https://forums.toonboom.com/t/flipping-marked-drawings-via-keyboard-non-drawing-hand/13374) | **Built** (A17) |
| F12 | Drawing thumbnails in the timeline | 2 | [KDE forum](https://forum.kde.org/viewtopic.php%3Ff=288&t=129608.html) | **Built:** thumbnail mode in the timeline |
| F13 | Recovery points, auto-backup, versioned saves | ~3 | [OpenToonz thread](https://redlib.astrial.org/r/OpenToonz/comments/1hsilh2/problem_with_opentoonz/m56cs2y/?context=3) | **Built now:** Save New Version (`shot_v002.toon`, Ctrl+Alt+S); **Have:** autosave recovery and a verified backup on every save |
| F14 | Sync projects across devices | 2 | [FlipaClip reviews](https://justuseapp.com/en/app/1101848914/flipaclip-create-2d-animation/reviews) | **Rejected:** out of scope; a shot is one portable file any sync tool can carry |
| F15 | Blend modes and clipping on animated layers | 2 | [review](https://thoughtsonthrees.substack.com/p/procreate-dreams-actually-works-now) | **Built:** blend modes and layer colours; clipping planned |
| F16 | Transform layers / keyframed moves over drawings | 2–3 | [Callipeg reviews](https://apps.apple.com/us/app/id1456172656) | **Partial:** camera keys built; pegs / transform layers planned |
| F17 | Multi-layer export for compositing | 1–2 | [TVPaint forum](https://www.tvpaint.com/forum/viewtopic.php?p=52517) | **Built:** layered PSD, OpenRaster, per-layer PNG sequences, ProRes 4444 with alpha, XDTS sheets with drawing folders |
| F18 | Named, coloured tracks | 2 | [Dreams 2.0.2 notes](https://help.procreate.com/articles/yoSWfU-what-s-new-in-procreate-dreams-2-0-2) | **Built:** named, coloured layers in folders |
| F19 | Animated brushes and 2D lighting effects | 1 each | [TVPaint forum](https://www.tvpaint.com/forum/viewtopic.php?p=117251) | **Rejected for now:** seen once; revisit with effects (M3) |
| F20 | Selections that persist across tools; add/subtract modes | 1 | [OpenToonz wishlist](https://groups.google.com/g/opentoonz_en/c/hNm5fP-J7rM) | **Partial:** the selection stays across tool switches; add / subtract modes planned |

## Takeaways that set the polish pass

1. Performance at scale and never losing work come first; both are measured and tested now.
2. No licence friction is free with open source; say so in the README.
3. Timing tools separate paint apps from animation tools: this pass built drag-to-hold and range
   editing; the waveform is next.
4. The drawing kit must be complete inside the animation workflow: selection and transform are the
   top remaining drawing gap.
