# Changelog

> 🇫🇷 [Lire en français](CHANGELOG.fr.md)

History of **versions published on GitHub** (`sergiodadi/BookNojo`),
one entry per tag/release.

## [v1.0.0b7] - 2026-11-22

- New app icon: a flat blue-gradient square with a white "BN" monogram,
  replacing the previous open-book icon.

## [v1.0.0b6] - 2026-11-22

- The Add-book form now has an optional "Choose an image…" button: the
  cover is attached right after the book is added, instead of only
  afterward via the Statistics panel's "no cover" list. If adding the
  cover fails, the book is still added — fixable afterward from the "no
  cover" list or `bc_add set-cover`.
- A "📂 Open Collection…" button in the header, next to "Add a book" — the
  same action as File > Import a collection… in the native menu, just
  reachable without opening it.

## [v1.0.0b5] - 2026-11-21

- README: added a "Screenshots" section (two screenshots of the app) right
  after Installation.
- README: reworded the intro to lead with "Why this project", merging it
  with the opening paragraph.
- `tauri.conf.json`'s version field now tracks the published release tag
  instead of bumping on every dev commit — this was silently causing the
  `.dmg` filename (and the `.app` bundle's own version fields) to show an
  internal development version number instead of the actual release.

## [v1.0.0b4] - 2026-11-16

- `DISPLAY_VERSION` fixed to track the published release tag — the
  header badge and both "About BookNojo" surfaces (the custom dialog
  and the native macOS panel) previously showed the raw internal
  development version instead.
- Added `set-cover` (both the `bc_add set-cover` CLI command and a new
  entry point in the app: the Statistics panel's "no cover" count
  expands into a clickable list of books, each opening a native file
  picker).
- Added `bc_add edit-plan`/`delete-plan` — editing and deleting a book
  from the command line, previously only possible from the
  application.

## [v1.0.0b3] - 2026-11-08

- README/README.fr.md: removed the version number from the status
  line — it went stale every time the public tag moved. The line now
  just states the project is actively developed and public.

## [v1.0.0b2] - 2026-11-08

- The repository is now **public**.
- README: corrected the status line, which still said "not yet
  distributed to anyone other than its author" — no longer accurate.
- Commit author identity corrected to use GitHub's no-reply address
  instead of a personal email.

## [v1.0.0b1] - 2026-11-04

First public beta version of BookNojo, with a downloadable `.dmg`
(`BookNojo_0.65.0_aarch64.dmg`) — no need to compile from source to
install it.
