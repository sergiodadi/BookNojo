#!/usr/bin/env python3
"""Builds a small, structurally-valid sample Book_data from testdata/'s copy
(never the real collection directly -- see build-sample-testdata.sh).

Strategy: pick a handful of books per sub-collection (preferring ones with a
cover, so the sample stays useful for visual testing), plus -- for the
largest sub-collection -- every book by 1-2 authors who have several, as a
deliberately better test case for author-grouping. Then copy the referential
closure of every table that pattern of books touches (lookups, persons,
genres/subjects/tags, credits/characters/contents, links), plus the small
system tables, so the result is a self-contained, independently valid
Core Data store -- not just a row dump.

Usage: build-sample-testdata.py <src_bkcp_dir> <dest_bkcp_dir>
(dest_bkcp_dir must already exist with an empty Images/ subfolder; invoked by
build-sample-testdata.sh, not meant to be run standalone.)
"""
import sqlite3
import shutil
import sys
from pathlib import Path

BOOKS_PER_COLLECTION = 5
EXTRA_AUTHORS_IN_MAIN_COLLECTION = 2

# "Lookup"-pattern tables: Z_PK, Z_ENT, Z_OPT, ZDELEGATE -> ZLOOKUPITEMDELEGATE.
LOOKUP_TABLES = [
    "ZPUBLISHER", "ZFORMAT", "ZLANGUAGE", "ZCOUNTRY", "ZCONDITION",
    "ZLOCATION", "ZSERIES", "ZEDITION", "ZPRINTEDBY", "ZVOLUME",
    "ZPAPERTYPE", "ZSTORE", "ZFREQUENCY", "ZGENRE", "ZSUBJECT", "ZTAG",
]
# "Person"-pattern tables: Z_PK, Z_ENT, Z_OPT, ZDELEGATE -> ZPERSONDELEGATE.
PERSON_TABLES = ["ZAUTHORPERSON", "ZCREDITPERSON", "ZCHARACTERPERSON", "ZOWNER", "ZLOANER"]
# Direct FK columns on ZCOLLECTIBLE pointing at one of the lookup tables above
# (column name == target table name, the convention used throughout).
COLLECTIBLE_LOOKUP_FKS = [
    "ZPUBLISHER", "ZFORMAT", "ZLANGUAGE", "ZCOUNTRY", "ZCONDITION",
    "ZLOCATION", "ZSERIES", "ZEDITION", "ZPRINTEDBY", "ZVOLUME",
    "ZPAPERTYPE", "ZSTORE", "ZORIGINALCOUNTRY", "ZORIGINALLANGUAGE",
    "ZORIGINALPUBLISHER",
]
COLLECTIBLE_PERSON_FKS = ["ZOWNER"]


def main():
    src_dir, dest_dir = Path(sys.argv[1]), Path(sys.argv[2])
    src_db, dest_db = src_dir / "Book_data", dest_dir / "Book_data"
    src_images, dest_images = src_dir / "Images", dest_dir / "Images"

    src = sqlite3.connect(f"file:{src_db}?mode=ro", uri=True)
    src.row_factory = sqlite3.Row

    dest_db.unlink(missing_ok=True)
    dest = sqlite3.connect(dest_db)

    print("Copie du schéma...")
    schema_rows = src.execute(
        "SELECT sql FROM sqlite_master WHERE sql IS NOT NULL AND type IN ('table', 'index')"
    ).fetchall()
    for (sql,) in schema_rows:
        dest.execute(sql)

    book_ent = src.execute("SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'Book'").fetchone()[0]
    story_ent = src.execute("SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'Story'").fetchone()[0]

    collections = src.execute("SELECT Z_PK, ZTITLE FROM ZCOLLECTION").fetchall()
    counts = dict(src.execute(
        "SELECT ZCOLLECTION, COUNT(*) FROM ZCOLLECTIBLE WHERE Z_ENT = ? GROUP BY ZCOLLECTION",
        (book_ent,),
    ).fetchall())
    main_coll_pk = max(counts, key=lambda pk: counts[pk])
    main_coll_title = next(t for pk, t in collections if pk == main_coll_pk)

    selected_books = set()
    for coll_pk, coll_title in collections:
        rows = src.execute(
            "SELECT Z_PK FROM ZCOLLECTIBLE WHERE Z_ENT = ? AND ZCOLLECTION = ? "
            "ORDER BY (ZFRONTCOVER IS NULL), Z_PK LIMIT ?",
            (book_ent, coll_pk, BOOKS_PER_COLLECTION),
        ).fetchall()
        picked = {r[0] for r in rows}
        selected_books |= picked
        print(f"  {coll_title}: {len(picked)} livre(s)")

    # Excludes generic "various authors" placeholder entries (e.g.
    # "Collectif") -- not useful as a test case for per-author grouping,
    # unlike a real named author with several books.
    author_rows = src.execute(
        """
        SELECT p.ZDISPLAYNAME, a.ZAUTHORPERSON, COUNT(*) c
        FROM ZAUTHOR a
        JOIN ZCOLLECTIBLE c2 ON c2.Z_PK = a.ZBOOK
        JOIN ZAUTHORPERSON ap ON ap.Z_PK = a.ZAUTHORPERSON
        JOIN ZPERSONDELEGATE p ON p.Z_PK = ap.ZDELEGATE
        WHERE c2.ZCOLLECTION = ? AND c2.Z_ENT = ?
          AND p.ZDISPLAYNAME NOT IN ('Collectif', 'Collectifs', 'Various', 'Divers')
        GROUP BY a.ZAUTHORPERSON
        HAVING c >= 2
        ORDER BY c DESC
        LIMIT ?
        """,
        (main_coll_pk, book_ent, EXTRA_AUTHORS_IN_MAIN_COLLECTION),
    ).fetchall()
    for name, author_pk, cnt in author_rows:
        books = src.execute(
            "SELECT ZBOOK FROM ZAUTHOR WHERE ZAUTHORPERSON = ?", (author_pk,)
        ).fetchall()
        added = 0
        for (bpk,) in books:
            row = src.execute("SELECT ZCOLLECTION FROM ZCOLLECTIBLE WHERE Z_PK = ?", (bpk,)).fetchone()
            if row and row[0] == main_coll_pk and bpk not in selected_books:
                selected_books.add(bpk)
                added += 1
        print(f"  + {name}: {added} livre(s) supplémentaire(s) ({cnt} au total dans '{main_coll_title}')")

    print(f"Total : {len(selected_books)} fiches sélectionnées.")

    # ---- Referential closure ----
    lookup_pks = {t: set() for t in LOOKUP_TABLES}
    person_pks = {t: set() for t in PERSON_TABLES}
    collection_pks = set()
    link_rows = []
    creator_rows = []
    character_rows = []
    story_pks = set()
    story_author_rows = []

    book_rows = []
    for pk in selected_books:
        row = src.execute("SELECT * FROM ZCOLLECTIBLE WHERE Z_PK = ?", (pk,)).fetchone()
        book_rows.append(row)
        if row["ZCOLLECTION"] is not None:
            collection_pks.add(row["ZCOLLECTION"])
        for col in COLLECTIBLE_LOOKUP_FKS:
            # Original/editon-time publisher etc. share ZPUBLISHER table.
            table = "ZPUBLISHER" if col in ("ZORIGINALPUBLISHER",) else (
                "ZCOUNTRY" if col == "ZORIGINALCOUNTRY" else (
                    "ZLANGUAGE" if col == "ZORIGINALLANGUAGE" else col
                )
            )
            val = row[col]
            if val is not None:
                lookup_pks[table].add(val)
        for col in COLLECTIBLE_PERSON_FKS:
            val = row[col]
            if val is not None:
                person_pks["ZOWNER"].add(val)

    # Authors of the selected books themselves.
    author_rows2 = []
    for pk in selected_books:
        for r in src.execute("SELECT * FROM ZAUTHOR WHERE ZBOOK = ?", (pk,)).fetchall():
            author_rows2.append(r)
            person_pks["ZAUTHORPERSON"].add(r["ZAUTHORPERSON"])

    # Genres / subjects / tags (many-to-many join tables).
    def junction_rows(table):
        # Book-side column naming is inconsistent across these join tables
        # (Z_7GENRES: Z_7BASEBOOKCOLLECTIBLES1, Z_8SUBJECTS: Z_8BOOKS1,
        # Z_6TAGS: Z_6COLLECTIBLES) -- "COLLECTIBLE"/"BOOKS" as a substring
        # covers all three actually seen in this schema.
        cols = [r[1] for r in src.execute(f"PRAGMA table_info({table})").fetchall()]
        book_col = next(c for c in cols if "COLLECTIBLE" in c.upper() or "BOOKS" in c.upper())
        other_col = next(c for c in cols if c != book_col)
        rows = []
        for pk in selected_books:
            rows.extend(src.execute(f"SELECT * FROM {table} WHERE {book_col} = ?", (pk,)).fetchall())
        return rows, book_col, other_col

    genre_joins, _, genre_col = junction_rows("Z_7GENRES")
    subject_joins, _, subject_col = junction_rows("Z_8SUBJECTS")
    for r in genre_joins:
        lookup_pks["ZGENRE"].add(r[genre_col])
    for r in subject_joins:
        lookup_pks["ZSUBJECT"].add(r[subject_col])

    tag_joins, tag_book_col, tag_col = [], None, None
    if src.execute("SELECT name FROM sqlite_master WHERE name = 'Z_6TAGS'").fetchone():
        tag_joins, tag_book_col, tag_col = junction_rows("Z_6TAGS")
        for r in tag_joins:
            lookup_pks["ZTAG"].add(r[tag_col])

    # Credits / characters / contents (may be empty -- fine either way).
    for pk in selected_books:
        for r in src.execute("SELECT * FROM ZCREATOR WHERE ZBASEBOOKCOLLECTIBLE = ?", (pk,)).fetchall():
            creator_rows.append(r)
            person_pks["ZCREDITPERSON"].add(r["ZCREDITPERSON"])
        for r in src.execute("SELECT * FROM ZCHARACTER WHERE ZBASEBOOKCOLLECTIBLE = ?", (pk,)).fetchall():
            character_rows.append(r)
            person_pks["ZCHARACTERPERSON"].add(r["ZCHARACTERPERSON"])
        for r in src.execute("SELECT * FROM ZSTORY WHERE ZBOOK = ?", (pk,)).fetchall():
            story_pks.add(r["Z_PK"])
        for r in src.execute("SELECT * FROM ZLINK WHERE ZBASEBOOKCOLLECTIBLE1 = ?", (pk,)).fetchall():
            link_rows.append(r)

    story_rows = [src.execute("SELECT * FROM ZSTORY WHERE Z_PK = ?", (pk,)).fetchone() for pk in story_pks]
    for spk in story_pks:
        for r in src.execute("SELECT * FROM ZAUTHOR WHERE ZSTORY = ?", (spk,)).fetchall():
            story_author_rows.append(r)
            person_pks["ZAUTHORPERSON"].add(r["ZAUTHORPERSON"])

    # Resolve lookup/person wrapper rows -> their delegate rows.
    lookupitemdelegate_pks = set()
    lookup_wrapper_rows = {}
    for table, pks in lookup_pks.items():
        rows = [src.execute(f"SELECT * FROM {table} WHERE Z_PK = ?", (pk,)).fetchone() for pk in pks]
        lookup_wrapper_rows[table] = rows
        for r in rows:
            if r["ZDELEGATE"] is not None:
                lookupitemdelegate_pks.add(r["ZDELEGATE"])

    persondelegate_pks = set()
    person_wrapper_rows = {}
    for table, pks in person_pks.items():
        rows = [src.execute(f"SELECT * FROM {table} WHERE Z_PK = ?", (pk,)).fetchone() for pk in pks]
        person_wrapper_rows[table] = rows
        for r in rows:
            if r["ZDELEGATE"] is not None:
                persondelegate_pks.add(r["ZDELEGATE"])

    lookupitemdelegate_rows = [
        src.execute("SELECT * FROM ZLOOKUPITEMDELEGATE WHERE Z_PK = ?", (pk,)).fetchone()
        for pk in lookupitemdelegate_pks
    ]
    persondelegate_rows = [
        src.execute("SELECT * FROM ZPERSONDELEGATE WHERE Z_PK = ?", (pk,)).fetchone()
        for pk in persondelegate_pks
    ]
    collection_rows = [
        src.execute("SELECT * FROM ZCOLLECTION WHERE Z_PK = ?", (pk,)).fetchone() for pk in collection_pks
    ]

    # ---- Write everything to dest ----
    def insert_rows(table, rows):
        rows = [r for r in rows if r is not None]
        if not rows:
            return
        cols = rows[0].keys()
        placeholders = ", ".join("?" for _ in cols)
        col_list = ", ".join(cols)
        dest.executemany(
            f"INSERT INTO {table} ({col_list}) VALUES ({placeholders})",
            [tuple(r[c] for c in cols) for r in rows],
        )

    insert_rows("ZCOLLECTION", collection_rows)
    insert_rows("ZCOLLECTIBLE", book_rows)
    insert_rows("ZLOOKUPITEMDELEGATE", lookupitemdelegate_rows)
    insert_rows("ZPERSONDELEGATE", persondelegate_rows)
    for table, rows in lookup_wrapper_rows.items():
        insert_rows(table, rows)
    for table, rows in person_wrapper_rows.items():
        insert_rows(table, rows)
    insert_rows("ZAUTHOR", author_rows2 + story_author_rows)
    insert_rows("Z_7GENRES", genre_joins)
    insert_rows("Z_8SUBJECTS", subject_joins)
    if tag_joins:
        insert_rows("Z_6TAGS", tag_joins)
    insert_rows("ZCREATOR", creator_rows)
    insert_rows("ZCHARACTER", character_rows)
    insert_rows("ZSTORY", story_rows)
    insert_rows("ZLINK", link_rows)

    # Small system/counter tables: copy wholesale, no personal data, and
    # keeps Z_PRIMARYKEY's Z_MAX counters correct for bc_add's PK allocator
    # (it reads Z_MAX per entity -- leaving it as in the source, rather than
    # compacting to the sample's smaller row counts, avoids any risk of a
    # new PK colliding with something a future `bc_add commit` creates).
    for table in ("Z_PRIMARYKEY", "Z_METADATA", "Z_MODELCACHE"):
        rows = src.execute(f"SELECT * FROM {table}").fetchall()
        insert_rows(table, rows)

    dest.commit()

    # ---- Copy only the referenced cover images ----
    dest_images.mkdir(exist_ok=True)
    copied = 0
    for row in book_rows:
        for col in ("ZFRONTCOVER", "ZBACKCOVER"):
            fname = row[col]
            if fname and (src_images / fname).is_file():
                shutil.copy2(src_images / fname, dest_images / fname)
                copied += 1
    print(f"Images copiées : {copied}")

    integrity = dest.execute("PRAGMA integrity_check").fetchone()[0]
    book_count = dest.execute(
        "SELECT COUNT(*) FROM ZCOLLECTIBLE WHERE Z_ENT = ?", (book_ent,)
    ).fetchone()[0]
    print(f"PRAGMA integrity_check: {integrity}")
    print(f"Fiches dans l'échantillon: {book_count}")
    if integrity != "ok":
        sys.exit(1)


if __name__ == "__main__":
    main()
