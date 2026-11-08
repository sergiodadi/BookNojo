# Command-line tools — `bc_export` / `bc_add`

> 🇫🇷 [Lire en français](CLI.fr.md)

> For everyday use, these commands are **not needed**: the application
> (see [`MANUAL.md`](MANUAL.md)) is enough to view, add, edit and
> delete books. This document is for anyone who wants to export a
> collection (CSV/JSON/HTML), script additions, or bootstrap a database
> independent from Book Collector — a more technical, optional use.

## 1. Overview

- **`bc_export`** reads a [Book Collector](https://www.collectorz.com/book)
  (Collectorz) collection read-only and produces a **CSV** export, a
  **JSON** export, and optionally a standalone **HTML page**
  reproducing the application's look (§ 3).
- **`bc_add`** adds, plans or transfers books from the command line
  (§ 4), with the same safety gates as the application.

`bc_export` always opens the database read-only
(`SQLITE_OPEN_READ_ONLY`): it can never modify the original collection.

The commands below assume you are in the folder containing the compiled
binaries (`project/target/release/` if compiled from source — see
[`BUILD.md`](BUILD.md)).

## 2. `bc_export` — basic export

```bash
./bc_export \
  --db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --csv book_collection.csv \
  --json book_collection.json
```

`--db` accepts either the `.bkcp`/`.col` folder, or directly the
`Book_data` file it contains.

### Sub-collections

A `.bkcp`/`.col` file can contain **several sub-collections**.

```bash
# List the sub-collections and their counters
./bc_export --db "<path>" --list-collections

# Export only one sub-collection
./bc_export --db "<path>" --collection "<name>" \
  --csv books.csv --json books.json
```

The counter shown by the application is the **sum of quantities** for
the active sub-collection, not the number of records — the two values
can legitimately differ.

## 3. The interactive HTML page

```bash
./bc_export --db "<path>" \
  --csv /tmp/bc.csv --json /tmp/bc.json \
  --html ~/Documents/book_collection_preview.html
```

Opens in any browser (double-click, or `open`), no server needed.
Three zones, like the application:

- **on the left**: grouping (Author by default, or Genre / Publisher /
  Format / Language / Country via the dropdown menu), with an A→Z bar.
  Clicking the number next to "All" toggles the value list between
  alphabetical order and descending book-count order.
- **top right**: the list of books in the selected group — search,
  sort, pagination, list/grid view toggle.
- **bottom right**: the detail card (cover, metadata, clickable
  genres, summary, notes).

The tabs at the top switch sub-collection without re-running the
command. The **Statistics** button shows an aggregated view (genres,
formats, languages, publishers, decades) for the active sub-collection.
An **FR / EN** selector in the header switches the whole interface
between French and English. The layout, the last navigation position
and the language are remembered from one session to the next (browser
local storage).

The **➕ Add a book** button opens a form (field details in § 4); in
this static export, it **contacts no server and writes nothing** —
"Generate new_book.json" simply downloads the file, to be passed to
`bc_add` afterwards (§ 4). In the macOS application, the same form
writes directly into the open collection (see
[`MANUAL.md`](MANUAL.md)).

## 4. `bc_add` — adding a book from the command line

> ⚠️ **Always test on a copy of your collection first, never directly
> on the original.** By default, `bc_add` refuses to write to a path
> that looks like the real Book Collector collection
> (`~/Documents/Collectorz/...`) — see § "Safety gates" below. If you
> are developing this project, `BUILD.md` explains how to generate
> dedicated test copies.

### Common usage — `bc_add add`

```bash
./bc_add add \
  --db "<path to your collection>" \
  --input new_book.json
```

Minimal example of a `new_book.json` (fields from the "Main" tab of
"Add Book"):

```json
{
  "title": "The Name of the Wind",
  "authors": ["Patrick Rothfuss"],
  "publisher": "Bragelonne",
  "format": "Paperback",
  "language": "french",
  "country": "France",
  "genres": ["Fantasy"],
  "collection": "<sub-collection name>"
}
```

The sub-collection (`collection`) must already exist — unlike other
fields, `bc_add` cannot create a new one.

`bc_add add` chains resolution, planning and writing in a single
command (see the step-by-step detail below if you need to control each
step separately, for example to review the SQL before running it).

It also accepts `--also-db` to repeat the whole pipeline against a
**second** database — typically the real Book Collector collection, for
as long as it still exists (see § "Why an independent database"). Each
database is resolved and planned **separately**: never the same SQL
copied from one database to the other, since primary keys and
already-known values (publishers, genres...) diverge as soon as two
databases evolve independently.

```bash
./bc_add add \
  --db "<your BookNojo database>" \
  --also-db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --input new_book.json \
  --force
```

`--force` applies to both databases if either one looks like the real
collection; the `YES` confirmation (see "Safety gates") is still
required on every run. If the primary database fails, nothing is
attempted on the second one. If it's the second one that fails, the
first stays committed — the `resolved`/`plan` files from the attempt
are kept in `bc_add_runs/<timestamp>/`, replayable later with
`bc_add commit` without starting over.

### All available fields

Beyond the "Main" tab (example above), all fields from the other tabs
of "Add Book" are available in the input JSON, all optional — only the
ones provided are taken into account. Covers (file management) are not
covered.

- **Details**: `edition`, `printed_by`, `series`, `volume`,
  `paper_type` (lists), `height`, `width`, `dewey`, `lccn`,
  `dedication` (free text), `first_edition`, `printing`, `abridged`,
  `issue_number`, `box_set`, `audio_length_minutes`, `original_title`,
  `original_subtitle`, `original_publisher`, `original_country`,
  `original_language`, `original_publication_year/month/day`.
- **Personal**: `condition`, `owner` (person), `store`,
  `current_value`, `purchase_price`, `cover_price`,
  `purchase_year/month/day`, `rare`, `read_it`, `read_times`,
  `read_year/month/day`, `my_rating`, `location_number`,
  `user_lookup1/2`, `user_text1/2`.
- **Plot & Notes**: `plot`, `notes`.
- **Credits & Characters**: `credits` (list of `{"name": ...}`) and
  `characters` (list of `{"name": ..., "alias": ...}`).
- **Contents**: `contents`, a list of
  `{"title": ..., "subtitle": ..., "number_of_pages": ..., "start_page": ..., "authors": [...]}`
  — several stories/novellas in the same book, each with its own
  authors.
- **Links**: `links`, a list of `{"url": ..., "description": ...}`.

```json
{
  "...": "... fields from the Main tab ...",
  "plot": "Story summary.",
  "notes": "Signed copy.",
  "condition": "Mint",
  "credits": [{ "name": "Translator's name" }],
  "characters": [{ "name": "Character's name", "alias": "Nickname" }],
  "contents": [{ "title": "Story title", "authors": ["Author's name"] }],
  "links": [{ "url": "https://...", "description": "Link description" }]
}
```

Fields deliberately not implemented (listed in detail by
`bc_add plan`): a credit's **role** (`ZCREATOR.ZROLEID`) — no matching
label exists in `Book_data`, left NULL; `ZUSERDEFINEDVALUESXML` and
`ZSTORY` fields other than `start_page` (Series/Plot/Notes/Original
edition per content) — no real data allowed confirming their exact
behaviour.

### Step by step (`resolve`, `plan`, `guard`, `commit`)

To control each step separately instead of `bc_add add`:

```bash
# 1. Resolves list-type values (author, publisher, genre...) against
#    the collection: already exists, or would be created.
./bc_add resolve --db "<path>" --input new_book.json --output resolved-book.json

# 2. Computes and prints the exact insert SQL, without ever running it.
./bc_add plan --db "<path>" --resolved resolved-book.json --output plan.sql

# 3. Checks that Book Collector isn't running, and backs up the
#    database (timestamped) before any write.
./bc_add guard --db "<path>"

# 4. Runs the SQL from `plan`, in a single transaction (all or nothing).
./bc_add commit --db "<path>" --plan plan.sql
```

`plan` ends its output with the explicit list of what it doesn't cover
yet (e.g. the `ZSEARCHSTRING` companion row, title sorting, CLZ Cloud
sync identifiers) — read it before `commit`.

### Safety gates

Before writing, `commit` (and so `add`) checks, in order:

1. the database path **does not look like** the real Book Collector
   collection (`~/Documents/Collectorz/...`) — immediate refusal
   otherwise;
2. Book Collector **is not running** (writing while it's open would
   risk an inconsistent WAL file, or it overwriting your changes on
   its next save);
3. a timestamped backup of `Book_data` (and `-wal`/`-shm` if present)
   is created in `backups/`, next to the database;
4. the SQL runs in a single transaction.

`--force` lifts refusal #1, but **still doesn't write directly**: a
confirmation typed by hand in the terminal takes over:

```
⚠️  WARNING: .../MyCollection.bkcp/Book_data is located under
.../Documents/Collectorz — this is the REAL Book Collector collection,
not a test copy.
   A timestamped backup will be taken before writing, but this
   operation directly modifies your real data.
   Type exactly YES to continue (anything else cancels):
```

Any answer other than `YES` (exact capital letters) cancels the
operation — no write happens, even with `--force`.

Before using `--force` on the real collection:
1. Close Book Collector.
2. Make a manual backup in addition to the automatic one that `commit`
   makes (e.g. copy the whole `.bkcp` folder).
3. Review the `plan.sql` produced by the "plan" step — the last chance
   to spot a problem before a real write.
4. Run `commit --force` (or `add --force`) and type `YES` only if
   everything looks right.

### Why an independent database, separate from Book Collector

Book Collector for Mac is an Intel app that depends on Rosetta 2 to run
on Apple Silicon. Apple has announced the planned end of Rosetta 2:
full support only through macOS 27 "Golden Gate" (fall 2026), largely
removed starting with macOS 28 (fall 2027). Book Collector therefore
has a limited remaining lifespan on an up-to-date Mac, unless its
publisher ships a native Apple Silicon update.

The long-term goal is a database **independent** from the application —
bootstrapped once from the real collection (§ "`bc_add import`"), then
fed only by BookNojo, never depending on Book Collector to keep
growing. `--also-db` serves the transition period: keep feeding the
real application for as long as it still runs, alongside the new
database. The chosen extension for this persistent database is
**`.col`** — it works exactly like a `.bkcp` (only the presence of a
`Book_data` file inside matters, never the folder's name).

### `bc_add import` — the BC to BookNojo transfer

Bootstraps a new independent `.col` database from an existing
collection (database + `Images/` folder). Only meant to be run
**once**: refuses to overwrite a destination that already exists.

```bash
./bc_add import --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp"
# -> collection/MyCollection.col (default destination)

# Or with an explicit destination:
./bc_add import \
  --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --dest ~/MyCollectionBookNojo.col
```

### Message language (`--lang`)

```bash
./bc_add --lang en import --source ... --dest ...
```

An i18n starting point: for now, only `import` has messages in both
French and English (`fr` by default). The rest of the tool stays in
French.

## 5. Troubleshooting

- **`--db "~/Documents/..."` fails**: if the path is quoted, the shell
  doesn't expand `~` itself — the tool does it for you.
- **Covers don't show up in the browser** (static HTML export): the
  first time a browser accesses a protected macOS folder (e.g.
  `~/Documents`), a system permission is requested once — accept it,
  then reload the page.
- **SQLite lock error**: close Book Collector before running an export
  or an add.

## 6. Going further

- [`MANUAL.md`](MANUAL.md): using the application — enough for everyday
  use, without these commands.
- [`BUILD.md`](BUILD.md): compiling the tools and the application from
  source (see also its appendix on the data model).
- [`CHANGELOG.md`](CHANGELOG.md): history of versions published on
  GitHub.
- `./bc_export --help`: full list of options.
