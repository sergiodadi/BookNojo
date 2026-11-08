# BookNojo

> 🇫🇷 [Lire en français](README.fr.md)

Book collection manager — imports a [Book Collector](https://www.collectorz.com/book)
(Collectorz) collection's database into its own database, and is
gradually evolving toward an **independent** database, so it keeps
working the day Book Collector no longer can.

> **Status: v1.0.0b1, beta.** Actively developed project, publicly
> available since 2026-11-08.

## Why this project

Book Collector for Mac is an Intel app that depends on Rosetta 2. Apple
has announced the planned end of Rosetta 2 (full support only through
macOS 27, largely removed starting with macOS 28, ~fall 2027) — so Book
Collector has a limited remaining lifespan on an up-to-date Mac. BookNojo
bootstraps an independent database from an existing collection, then
keeps feeding it with its own tools, without depending on Book Collector
to grow.

## What it does today

- **macOS application** (icon, window, native menus) to view, add, edit
  and delete books in a collection — with confirmation before any write
  and automatic backups.
- Grouping by author/genre/publisher/format/language/country, search,
  full detail card (credits, characters, contents, links...).
- Bilingual French/English interface.
- Command-line export to **CSV**, **JSON**, and a standalone **HTML
  page** reproducing the application's interface.

➡️ Full guide to using the application: [`MANUAL.md`](MANUAL.md).
Command-line tools also exist (CSV/JSON/HTML export, scripts) —
optional, not needed for everyday use: see [`CLI.md`](CLI.md).

## Installation

Download the `.dmg` from the [latest release](https://github.com/sergiodadi/BookNojo/releases/latest),
open it, and drag BookNojo.app into Applications.

> ⚠️ **"BookNojo is damaged and can't be opened"?** BookNojo isn't signed
> with an Apple Developer certificate, so macOS Gatekeeper quarantines it
> and shows this misleading message — the app isn't actually damaged.
> Fix it by running this once in Terminal, then open the app normally:
> ```bash
> xattr -cr /Applications/BookNojo.app
> ```
> If that doesn't work, right-click (or Control-click) the app and choose
> **Open** instead of double-clicking.

To compile from source instead of using the `.dmg`, see
[`BUILD.md`](BUILD.md).

> Tested only on **macOS Tahoe (26)** — not verified on other macOS
> versions.

## Version history

See [`CHANGELOG.md`](CHANGELOG.md) — one entry per version published on
GitHub.

## License

[MIT](LICENSE).

---

These packages were built, tested and published with the help of Claude, the AI assistant by Anthropic.

Book Collector and Collectorz/CLZ are trademarks of their respective owners. BookNojo is an independent project, not affiliated with these companies.
