# Compiling BookNojo from source

> 🇫🇷 [Lire en français](BUILD.fr.md)

This document explains how to compile the command-line tools and the
macOS application from source code, and how to reproduce the
development environment. To **use** the software once compiled, see
[`MANUAL.md`](MANUAL.md) (the application — enough for everyday use)
and [`CLI.md`](CLI.md) (the `bc_export`/`bc_add` command-line tools,
optional).

## Prerequisites

- **Rust** (`cargo`) — installable via Homebrew if needed:
  `brew install rust`.
- **Tauri CLI**, only for the macOS application (not needed for the CLI
  tools alone):
  ```bash
  cargo install tauri-cli --version "^2.0.0"
  ```

All the code and manifests live under **`project/`** — the repo root
only contains documentation (`README.md`, `BUILD.md`, `MANUAL.md`,
`CLI.md`, `CHANGELOG.md`, each with a `.fr.md` French counterpart). All
the commands below assume you are inside `project/`.

## Command-line tools (`bc_export`, `bc_add`)

```bash
cd booknojo/project
cargo build --release
```

The binaries are then `target/release/bc_export` and
`target/release/bc_add` (so `project/target/release/...` from the repo
root).

```bash
cargo test --release    # runs both the unit AND the integration tests
```

## macOS application

```bash
cd project                   # if not already there
cargo build --release        # if not already done
./app/generate-dev-data.sh   # syncs app/src/{index.html,app.js,style.css}
                              # from templates/ and app/src/index.template.html
cd app
cargo tauri dev      # development window, hot reload
cargo tauri build    # builds a real .app (and a .dmg), in
                      # app/src-tauri/target/release/bundle/
```

⚠️ **Important**: `./app/generate-dev-data.sh` must be re-run after
**any** change to `templates/app.js`/`style.css` or to
`app/src/index.template.html`, before `cargo tauri dev`/`build` —
otherwise the window runs against a stale copy in `app/src/`, with no
visible error. Check with
`diff <(md5 -q templates/app.js) <(md5 -q app/src/app.js)` (from
`project/`).

`cargo build --release` (from `project/`) does **not** produce an
installable application — only `cargo tauri build` (inside
`project/app/`) does. Remember to re-run it before shipping a new
version.

### Application icon

Can be regenerated from a 1024×1024 source image (`app/app-icon.png`):

```bash
cd project/app
cargo tauri icon app-icon.png
```

## Test data (development only)

These copies and scripts are only needed to contribute to the project —
not to use the software on your own collection. From `project/`:

```bash
./refresh-testdata.sh        # testdata/: read-only copy (chmod 444)
./refresh-testdata-rw.sh     # testdata-rw/: writable copy, for testing writes
./build-sample-testdata.sh   # testdata-sample/: extract (~100 records) for fast iteration
```

`sample-collection/` (committed to the repo, entirely fictional — see
[`MANUAL.md`](MANUAL.md) § 1) can be regenerated with:

```bash
cargo run --release --example generate_sample_collection
```

None of these scripts ever touch the user's real file
(`~/Documents/Collectorz/Book Collector/...`) — a project rule, checked
by dedicated tests.

## Repository structure

```
README.md, BUILD.md, MANUAL.md,     documentation (repo root), each
CLI.md, CHANGELOG.md                with a .fr.md French counterpart

project/
├─ src/            shared library + CLI binaries (bc_export, bc_add)
├─ app/            macOS application (Tauri) — reuses src/ via a path
│                  dependency
├─ templates/      HTML/JS/CSS page shared between the static export
│                  (bc_export --html) and the application
├─ examples/       generates the sample collection (sample-collection/)
├─ tests/          integration tests (add/edit/delete)
├─ Cargo.toml      root crate manifest (book_collector_export)
└─ refresh-testdata*.sh, build-sample-testdata.*   test-data scripts
```

## Appendix: Book Collector data model

> ⚠️ **Given for information only** — this field catalogue may be
> inaccurate or incomplete. It describes Book Collector's built-in XML
> export (Export menu, XML format), not the actual storage format used
> by the rest of this project.

Every book exported to XML declares, at the top of the file, the list
of fields it knows about (`<field name="..." id="..." label="...">`
tag). 108 fields in total, in the order Book Collector declares them:

| Internal ID | Label |
|---|---|
| `dfTitleSort` | Sort Title |
| `dfForewordAuthor` | Foreword Author |
| `dfDewey` | Dewey |
| `dfSubTitle` | Sub Title |
| `dfPhotographer` | Photographer |
| `dfAudioBookLinks` | Audio Book Files |
| `dfCountry` | Country |
| `dfPurchaseYear` | Purchase Year |
| `dfISBN` | ISBN |
| `dfNarrator` | Narrator |
| `dfBookLinks` | Book Files |
| `dfLanguage` | Language |
| `dfReadingYear` | Reading Year |
| `dfLastModified` | Last Modified |
| `dfFormat` | Format |
| `dfeBookLinks` | eBook Files |
| `dfGenre` | Genre |
| `dfSeries` | Series |
| `dfLCCN` | LoC Classification |
| `dfImageLinks` | Interior Art Files |
| `dfSubject` | Subject |
| `dfIssueNr` | Issue No. |
| `dfPageCount` | No. Of Pages |
| `dfPrinting` | Printing |
| `dfCollectionStatus` | Collection Status |
| `dfCharacters` | Characters |
| `dfBoxSet` | Box Set |
| `dfPublisher` | Publisher |
| `dfDimWidth` | Width |
| `dfOtherLinks` | Other Files |
| `dfIndex` | Index |
| `dfOrigPublicationYear` | Original Publication Year |
| `dfVolume` | Volume |
| `dfPublicationDate` | Publication Date |
| `dfDimHeight` | Height |
| `dfPublicationYear` | Publication Year |
| `dfQuantity` | Quantity |
| `dfReadTimes` | Times Read |
| `dfCredits` | Credits |
| `dfAudioLength` | Length |
| `dfOrigTitle` | Original Title |
| `dfSubmissionDate` | Last Submission Date |
| `dfMyRating` | My Rating |
| `dfReadingDate` | Reading Date |
| `dfEdition` | Edition |
| `dfOrigSubTitle` | Original Sub Title |
| `dfBPBookID` | Clz Book ID |
| `dfReadIt` | Read It |
| `dfFirstEdition` | First Edition |
| `dfCondition` | Condition |
| `dfLoaner` | Loaned To |
| `dfOrigPublisher` | Original Publisher |
| `dfOwner` | Owner |
| `dfPrintedBy` | Printed By |
| `dfCurrentPrice` | Current Value |
| `dfLoanDate` | Loan Date |
| `dfOrigPublicationDate` | Original Publication Date |
| `dfLocation` | Location |
| `dfPaperType` | Paper Type |
| `dfDueDate` | Due Date |
| `dfOrigCountry` | Original Country |
| `dfPlot` | Plot |
| `dfDedication` | Dedication |
| `dfIsOverdue` | isOverdue |
| `dfOrigLanguage` | Original Language |
| `dfNotes` | Notes |
| `dfPurchaseDate` | Purchase Date |
| `dfReturnDate` | Return Date |
| `dfExtras` | Extras |
| `dfLinks` | Local Links |
| `dfPurchasePrice` | Purchase Price |
| `dfSyncFrontCover` | Custom Image |
| `dfLoanNotes` | Loan Notes |
| `dfOnlineLinks` | Online Links |
| `dfStore` | Store |
| `dfAddedDate` | Added Date |
| `dfAuthor` | Author |
| `dfTag` | Tags |
| `dfLoanEmail` | Loaned To Email |
| `dfAbridged` | Abridged |
| `dfCoverPrice` | Cover Price |
| `dfCollection` | Collection |
| `dfTranslator` | Translator |
| `dfCoverFront` | FroBMnt Cover |
| `dfUserRole1` | User Role 1 |
| `dfLoanAddress` | Loaned To Address |
| `dfRare` | Rare |
| `dfSyncBackCover` | Custom Back Cover |
| `dfCoverArtist` | Cover Artist |
| `dfCoverBack` | Back Cover |
| `dfUserRole2` | User Role 2 |
| `dfLoanTitle` | Loan Title |
| `dfStartPage` | Start Page |
| `dfEditor` | Editor |
| `dfBackdrop` | Backdrop |
| `dfUserRole3` | User Role 3 |
| `dfFrequency` | Frequency |
| `dfIllustrator` | Illustrator |
| `dfConnectHash` | ConnectHash |
| `dfAuthorFirstLetter` | Author First Letter |
| `dfTitleFirstLetter` | Title First Letter |
| `dfTitle` | Title |
| `dfGhostwriter` | Ghostwriter |
| `dfLoCNr` | LoC Control Number |
| `dfInCollectionBit` | In Collection (0/1) |
| `dfThumbFilePath` | Thumbnail |
| `dfBPBookLastReceivedRevision` | BPBookLastReceivedVersion |
| `dfDynRating` | My Rating |

Beyond the simple fields above, each book also contains repeatable
lists (a book can have several authors, several credits, etc.):

- `<authors>` → `<author>` (role + `<person>`: display name, sort name,
  first name, last name, birth/death date)
- `<credits>` → credits with a role, same `<person>` structure
- `<characters>` → characters
- `<genres>`, `<subjects>`, `<tags>` → lists of plain values
- `<links>` → `<link>` (url, description, type)
- `<original>` → "Original edition" block (title, subtitle, publisher,
  country, language, publication date)
