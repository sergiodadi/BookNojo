# User manual — BookNojo application

> 🇫🇷 [Lire en français](MANUAL.fr.md)

This document explains how to use the macOS **BookNojo** application
once installed. To download it, see [`README.md`](README.md); to
compile it from source, see [`BUILD.md`](BUILD.md).

## 1. Opening a collection

- **File > Import a collection…** opens the native macOS file picker to
  choose a collection — a `.bkcp` folder (Book Collector) or `.col`
  (BookNojo). The choice is remembered automatically: on the next
  launch, BookNojo reopens the same collection without asking anything.
- A `.bkcp` collection from a Book Collector version other than the one
  BookNojo was built against (20.3.1) is rejected at import, with a
  clear message rather than a crash or silent corruption.
- On the very first launch, before any import, BookNojo opens an
  entirely fictional sample collection (10 invented books, no personal
  data) — so there's always something to see, without ever exposing
  real data by default.
- The header always displays the name of the open file and the
  application's version.

## 2. Browsing the library

- **Three zones**: the grouping panel on the left, the list of books
  from the selected group at the top right, the detail card of the
  chosen book at the bottom right. The dividers between the three zones
  can be resized with the mouse, and the layout is remembered from one
  session to the next.
- **Grouping** by Author (default), Genre, Publisher, Format, Language
  or Country, with an A→Z alphabetical bar to jump straight to a
  letter.
- Clicking the number next to "All" toggles the group list below it
  between alphabetical order and descending book-count order.
- **Book list**: text search, sort, pagination, toggle between list and
  grid view (cover thumbnails).
- **Multiple sub-collections** in a single file (e.g. "Books" /
  "Comics") — tabs at the top let you switch between them without
  reloading anything by hand.
- **Statistics**: an aggregated view (genres, formats, languages,
  publishers, decades) for the currently displayed sub-collection.
- **Book detail card**: cover, title/authors/publisher and other main
  information, then — when filled in — Product Details (dimensions,
  Dewey/LCCN, first printing, limited edition...), Original Edition,
  Personal Details (condition, price, purchase date, personal rating,
  read or not...), Credits, Characters, Contents (stories/novellas
  inside an anthology), Links, Plot and Notes. Empty fields or sections
  with no value at all are not displayed.

## 3. Adding, editing, deleting a book

- **Add a book**: a full form organized into sub-tabs (Main, Details,
  Personal, Credits & Characters, Contents, Plot & Notes, Links), with
  autocomplete on values already used in the collection (author,
  publisher, genre, format, language, country, series, etc.). Before
  writing anything, a confirmation box resolves the book against the
  open collection and lists every value that would be created for the
  first time (a "new" badge) — to catch a typo before it becomes a
  permanent entry in the collection. The write only happens after
  confirmation.
- **Edit a book**: from its detail card, the same form opens pre-filled
  with the current values; saving replaces the old values with the new
  ones, with the same confirmation step.
- **Delete a book**: from its detail card, after confirmation. Only the
  rows specific to that book are removed — a shared value (a publisher,
  a genre...) is never deleted, even if that book was its only user.
- After any add, edit or delete, the library is automatically reloaded:
  the book appears (or disappears) immediately in the list and in
  search, with no manual action needed.

## 4. Bilingual interface

- An **FR / EN** selector in the header switches the whole interface —
  grouping, list, detail card, statistics, add/edit form — between
  French and English. The choice is remembered from one session to the
  next.
- Messages returned by the application itself after an action
  (confirmations, errors) stay in French regardless of the chosen
  language — only the browser-generated interface is translated for
  now.

## 5. Data safety

- The user's real Book Collector collection is never modified by
  accident: every write (add, edit, delete) is preceded by a timestamped
  backup of the file, and is refused outright if Book Collector is
  running at the same time (to avoid a write conflict).
- BookNojo can open the real collection in **read-only** mode to browse
  it, but none of the write actions above have any exception for it from
  the application — unlike the command-line tools, which are meant for
  more advanced, technical use (see [`CLI.md`](CLI.md)).

## 6. What the application doesn't do yet

- **Online lookup** to pre-fill a record from an ISBN (OpenLibrary/
  Google Books/BnF) — deferred, not started.
- Not yet code-signed/notarized with an Apple developer account — no
  consequence for personal use, but needed before distributing the
  application to anyone else.

## 7. Going further

- [`BUILD.md`](BUILD.md): compiling the application from source.
- [`CHANGELOG.md`](CHANGELOG.md): history of versions published on
  GitHub.

For everyday use, the application alone is enough — you don't need to
know any terminal commands. For the command-line tools
(`bc_export`/`bc_add`), meant for more technical use (CSV/JSON/HTML
export, scripts, transition away from Book Collector), see
[`CLI.md`](CLI.md).
