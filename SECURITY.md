# Security policy

## Reporting a vulnerability

Please report security problems privately through GitHub:
**[Report a vulnerability](https://github.com/storytold/tooncraft/security/advisories/new)**
(Security tab › Advisories › Report a vulnerability).

Don't open a public issue or pull request for a vulnerability until a fix is released.

Useful to include:
- the ToonCraft version or commit, and the platform (macOS, Windows, Linux, BSD);
- what an attacker can do, and what they need (a crafted shot, image, sound or exposure-sheet file, a command file for `tooncraft-cli`, a local account…);
- steps or a sample file that reproduce it.

We'll acknowledge the report, keep you updated while we work on a fix, and credit you in the advisory unless you'd rather not be named.

## Supported versions

Fixes go into `main` and ship in the next release. Older releases don't get backported fixes.

## Scope

In scope: anything that lets a shot, imported file (PSD, OpenRaster, GIF, XDTS, images, sound), brush preset, command file or network response read or write files it shouldn't, run code, leak data, or crash ToonCraft and lose the user's work.

Crashes from hostile input count, because ToonCraft treats every panic as a bug (see [Never crash](AGENTS.md)).
