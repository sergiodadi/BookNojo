//! Shared helpers for reading a Collectorz "Book Collector" Core Data/SQLite
//! collection (`Book_data`, inside a `.bkcp` document package).
//!
//! Used by both binaries in this crate, and by the NBC app (`app/src-tauri`)
//! via a path dependency on this crate:
//! - `bc_export` (src/main.rs): read-only export to CSV/JSON/HTML.
//! - `bc_add` (src/bin/bc_add.rs): CLI front end for the add-a-book pipeline
//!   below (resolve → plan → commit) — see its own doc comment for the
//!   safety gates (`--force`, typed "OUI") that stay CLI-only.
//! - NBC's "Ajouter un livre" form calls the same `resolve_book`/
//!   `plan_book_sql`/`prepare_add_book`/`commit_add_book` functions directly
//!   over Tauri IPC, instead of bc_add's file-based resolve→plan→commit
//!   round trip.
//!
//! Schema background:
//! - Core Data uses single-table inheritance: all book rows live in
//!   ZCOLLECTIBLE, discriminated by Z_ENT (looked up by name "Book" below,
//!   since the numeric id is a model-compiler detail, not guaranteed stable).
//! - "Lookup" entities (Publisher, Series, Format, Language, Country,
//!   Condition, Location, PaperType, ...) don't store their own name: they
//!   point via ZDELEGATE to a shared ZLOOKUPITEMDELEGATE row that holds
//!   ZDISPLAYNAME. This is a Core Data flyweight pattern for reusable values.
//! - Person-like entities (AuthorPerson, CharacterPerson, ...) follow the
//!   same pattern through ZPERSONDELEGATE instead.

use anyhow::{bail, Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

/// Expands a leading "~" or "~/..." to $HOME. The shell only does this when
/// the argument is unquoted, so a quoted "~/Documents/..." reaches us
/// literally — this makes `--db "~/Documents/..."` work either way.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    let Ok(home) = std::env::var("HOME") else {
        return path.to_path_buf();
    };
    if s == "~" {
        PathBuf::from(home)
    } else if let Some(rest) = s.strip_prefix("~/") {
        PathBuf::from(home).join(rest)
    } else {
        path.to_path_buf()
    }
}

/// Resolves `--db` (a `.bkcp` directory or a direct path to `Book_data`) to
/// an absolute, canonicalized path to `Book_data`.
pub fn resolve_db_path(input: &Path) -> Result<PathBuf> {
    let input = &expand_tilde(input);
    let resolved = if input.is_dir() {
        let candidate = input.join("Book_data");
        if !candidate.is_file() {
            bail!(
                "{} est un dossier mais ne contient pas de fichier 'Book_data'",
                input.display()
            );
        }
        candidate
    } else {
        if !input.is_file() {
            bail!("fichier introuvable: {}", input.display());
        }
        input.to_path_buf()
    };
    // Canonicalize to an absolute path: anything derived from this (e.g.
    // file:// URLs in bc_export's HTML output) needs to be absolute to
    // resolve correctly outside the directory the tool was launched from.
    std::fs::canonicalize(&resolved)
        .with_context(|| format!("résolution du chemin absolu de {}", resolved.display()))
}

/// "Images/" lives next to Book_data inside the .bkcp package.
pub fn resolve_images_dir(db_path: &Path) -> Option<PathBuf> {
    let dir = db_path.parent()?.join("Images");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Looks up a Core Data entity's `Z_ENT` id by name, rather than
/// hardcoding it — the numeric id is a model-compiler detail, not
/// guaranteed stable across collections.
pub fn entity_ent_id(conn: &Connection, name: &str) -> Result<i64> {
    conn.query_row("SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = ?1", [name], |row| {
        row.get(0)
    })
    .with_context(|| format!("entité Core Data '{name}' introuvable dans Z_PRIMARYKEY (schéma inattendu)"))
}

/// Looks up a Core Data entity's `Z_MAX` — the highest primary key handed
/// out so far for that entity (shared by every sub-entity in a
/// single-table-inheritance hierarchy, e.g. Book shares Collectible's
/// counter). Used by `bc_add plan` to allocate new primary keys without
/// colliding with ones the app itself would hand out next.
pub fn entity_max_pk(conn: &Connection, name: &str) -> Result<i64> {
    conn.query_row("SELECT Z_MAX FROM Z_PRIMARYKEY WHERE Z_NAME = ?1", [name], |row| {
        row.get(0)
    })
    .with_context(|| format!("entité Core Data '{name}' introuvable dans Z_PRIMARYKEY (schéma inattendu)"))
}

/// Thin wrapper kept for `bc_export`'s existing call sites.
pub fn book_entity_id(conn: &Connection) -> Result<i64> {
    entity_ent_id(conn, "Book")
}

/// Resolves a sous-collection name (ZCOLLECTION.ZTITLE, case-insensitive) to
/// its primary key, or lists the available names in the error if not found.
pub fn resolve_collection_pk(conn: &Connection, name: &str) -> Result<i64> {
    let pk: Option<i64> = conn
        .query_row(
            "SELECT Z_PK FROM ZCOLLECTION WHERE ZTITLE = ?1 COLLATE NOCASE",
            [name],
            |row| row.get(0),
        )
        .ok();
    match pk {
        Some(pk) => Ok(pk),
        None => {
            let mut stmt = conn.prepare("SELECT ZTITLE FROM ZCOLLECTION ORDER BY ZTITLE")?;
            let mut rows = stmt.query([])?;
            let mut names = Vec::new();
            while let Some(row) = rows.next()? {
                names.push(row.get::<_, String>(0)?);
            }
            bail!(
                "sous-collection '{name}' introuvable. Disponibles: {}",
                names.join(", ")
            );
        }
    }
}

/// Resolves a "lookup" value (Publisher, Genre, Format, Language, Country,
/// Location, Subject, ...) against the shared flyweight table: looks for a
/// row in `table` whose `ZDELEGATE` points to a `ZLOOKUPITEMDELEGATE` row
/// with this display name (case-insensitive). Returns the **lookup
/// entity's** primary key (e.g. the ZPUBLISHER.Z_PK, not the delegate's),
/// since that's what a ZCOLLECTIBLE foreign key column points to.
///
/// `table` must be one of the entities that follow this exact pattern
/// (`Z_PK, Z_ENT, Z_OPT, ZDELEGATE, ...`) — see the schema doc comment.
/// Not parameterized by the caller beyond that: always queries
/// `ZLOOKUPITEMDELEGATE` as the delegate table.
pub fn resolve_lookup_pk(conn: &Connection, table: &str, display_name: &str) -> Result<Option<i64>> {
    // `table` is never user input (it's one of a small set of hardcoded
    // literals passed by call sites below), so this format! is safe from
    // SQL injection — only `display_name` is bound as a parameter.
    let sql = format!(
        "SELECT t.Z_PK FROM {table} t \
         JOIN ZLOOKUPITEMDELEGATE l ON l.Z_PK = t.ZDELEGATE \
         WHERE l.ZDISPLAYNAME = ?1 COLLATE NOCASE"
    );
    conn.query_row(&sql, [display_name], |row| row.get(0))
        .optional_result()
}

/// Resolves a person display name (Author, Character, Credit, ...) against
/// `ZPERSONDELEGATE`, the person-equivalent flyweight table. Returns the
/// **person entity's** primary key (e.g. ZAUTHORPERSON.Z_PK).
pub fn resolve_person_pk(conn: &Connection, table: &str, display_name: &str) -> Result<Option<i64>> {
    let sql = format!(
        "SELECT t.Z_PK FROM {table} t \
         JOIN ZPERSONDELEGATE p ON p.Z_PK = t.ZDELEGATE \
         WHERE p.ZDISPLAYNAME = ?1 COLLATE NOCASE"
    );
    conn.query_row(&sql, [display_name], |row| row.get(0))
        .optional_result()
}

/// Small helper trait so `resolve_lookup_pk`/`resolve_person_pk` can turn
/// rusqlite's `QueryReturnedNoRows` into `Ok(None)` (value doesn't exist
/// yet — a legitimate, expected outcome here) while still propagating any
/// *other* error (a real problem) as `Err`.
trait OptionalResult<T> {
    fn optional_result(self) -> Result<Option<T>>;
}

impl<T> OptionalResult<T> for rusqlite::Result<T> {
    fn optional_result(self) -> Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

// --------------------------------------------------------------------
// Reading the collection (step 3 of the NBC app plan — shared between
// `bc_export`'s CSV/JSON/HTML export and the NBC app's live Tauri command,
// so both read the collection exactly the same way instead of duplicating
// ~150 lines of joins.
// --------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Seconds between the Unix epoch (1970-01-01) and the Core Data / NSDate
/// reference date (2001-01-01), both UTC. Core Data TIMESTAMP columns store
/// seconds relative to the latter.
const CORE_DATA_EPOCH_OFFSET: i64 = 978_307_200;

pub fn core_data_timestamp_to_iso8601(seconds: f64) -> String {
    let unix_secs = seconds.round() as i64 + CORE_DATA_EPOCH_OFFSET;
    let days = unix_secs.div_euclid(86_400);
    let secs_of_day = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// Howard Hinnant's days-from-civil algorithm, used in reverse to turn a
/// day count since the Unix epoch into a proleptic-Gregorian (y, m, d).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Percent-encodes a filesystem path for use as a `file://` URL (spaces,
/// accented bytes, parentheses, ... — everything but unreserved chars and
/// the path separator).
pub fn file_url(path: &str) -> String {
    const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    let mut out = String::from("file://");
    for byte in path.bytes() {
        if byte == b'/' || UNRESERVED.contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct Book {
    pub id: i64,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub series: Option<String>,
    pub format: Option<String>,
    pub language: Option<String>,
    pub country: Option<String>,
    pub condition: Option<String>,
    pub location: Option<String>,
    pub edition: Option<String>,
    pub printed_by: Option<String>,
    pub volume: Option<String>,
    pub paper_type: Option<String>,
    pub owner: Option<String>,
    pub store: Option<String>,
    pub publication_day: Option<String>,
    pub publication_month: Option<String>,
    pub publication_year: Option<String>,
    pub number_of_pages: Option<i64>,
    pub my_rating: Option<i64>,
    pub read_it: Option<bool>,
    pub date_added: Option<String>,
    pub genres: Vec<String>,
    pub subjects: Vec<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub plot: Option<String>,
    pub current_value: Option<f64>,
    pub purchase_price: Option<f64>,
    pub collection: Option<String>,
    /// Nombre d'exemplaires (ZQUANTITY, 1 si absent). L'app additionne ce
    /// champ sur toute une sous-collection pour afficher son compteur.
    pub quantity: i64,
    pub front_cover_file: Option<String>,
    pub front_cover_path: Option<String>,
    pub back_cover_file: Option<String>,
    pub back_cover_path: Option<String>,
}

/// Groups rows of (book_pk, value) into book_pk -> ordered Vec<value>.
fn group_by_book(conn: &Connection, sql: &str) -> Result<HashMap<i64, Vec<String>>> {
    let mut stmt = conn.prepare(sql)?;
    let mut map: HashMap<i64, Vec<String>> = HashMap::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let book_pk: i64 = row.get(0)?;
        let value: String = row.get(1)?;
        map.entry(book_pk).or_default().push(value);
    }
    Ok(map)
}

fn load_authors(conn: &Connection) -> Result<HashMap<i64, Vec<String>>> {
    group_by_book(
        conn,
        "SELECT a.ZBOOK, pd.ZDISPLAYNAME
         FROM ZAUTHOR a
         JOIN ZAUTHORPERSON ap ON ap.Z_PK = a.ZAUTHORPERSON
         JOIN ZPERSONDELEGATE pd ON pd.Z_PK = ap.ZDELEGATE
         WHERE a.ZBOOK IS NOT NULL AND pd.ZDISPLAYNAME IS NOT NULL
         ORDER BY a.ZBOOK, a.ZRANK",
    )
}

fn load_genres(conn: &Connection) -> Result<HashMap<i64, Vec<String>>> {
    group_by_book(
        conn,
        "SELECT j.Z_7BASEBOOKCOLLECTIBLES1, l.ZDISPLAYNAME
         FROM Z_7GENRES j
         JOIN ZGENRE g ON g.Z_PK = j.Z_18GENRES
         JOIN ZLOOKUPITEMDELEGATE l ON l.Z_PK = g.ZDELEGATE
         WHERE l.ZDISPLAYNAME IS NOT NULL
         ORDER BY j.Z_7BASEBOOKCOLLECTIBLES1, l.ZDISPLAYNAME",
    )
}

fn load_subjects(conn: &Connection) -> Result<HashMap<i64, Vec<String>>> {
    group_by_book(
        conn,
        "SELECT j.Z_8BOOKS1, l.ZDISPLAYNAME
         FROM Z_8SUBJECTS j
         JOIN ZSUBJECT s ON s.Z_PK = j.Z_38SUBJECTS
         JOIN ZLOOKUPITEMDELEGATE l ON l.Z_PK = s.ZDELEGATE
         WHERE l.ZDISPLAYNAME IS NOT NULL
         ORDER BY j.Z_8BOOKS1, l.ZDISPLAYNAME",
    )
}

fn load_tags(conn: &Connection) -> Result<HashMap<i64, Vec<String>>> {
    group_by_book(
        conn,
        "SELECT j.Z_6COLLECTIBLES, l.ZDISPLAYNAME
         FROM Z_6TAGS j
         JOIN ZTAG t ON t.Z_PK = j.Z_39TAGS
         JOIN ZLOOKUPITEMDELEGATE l ON l.Z_PK = t.ZDELEGATE
         WHERE l.ZDISPLAYNAME IS NOT NULL
         ORDER BY j.Z_6COLLECTIBLES, l.ZDISPLAYNAME",
    )
}

/// `collection_pk` filters to one sous-collection; pass `None` for every
/// sous-collection (needed so a generated page's tab bar / an app's
/// sidebar has something to switch between).
pub fn load_books(conn: &Connection, images_dir: Option<&Path>, collection_pk: Option<i64>) -> Result<Vec<Book>> {
    let book_ent = book_entity_id(conn)?;

    let authors = load_authors(conn)?;
    let genres = load_genres(conn)?;
    let subjects = load_subjects(conn)?;
    let tags = load_tags(conn)?;

    let sql = "
        SELECT
            c.Z_PK, c.ZTITLE, c.ZSUBTITLE, c.ZISBN,
            c.ZNUMBEROFPAGES, c.ZMYRATING, c.ZREADIT, c.ZDATEADDED,
            c.ZNOTES, c.ZPLOT, c.ZCURRENTVALUE, c.ZPURCHASEPRICE,
            c.ZPUBLICATIONDAY, c.ZPUBLICATIONMONTH, c.ZPUBLICATIONYEAR,
            c.ZFRONTCOVER, c.ZBACKCOVER, COALESCE(c.ZQUANTITY, 1),
            pub_l.ZDISPLAYNAME  AS publisher,
            ser_l.ZDISPLAYNAME  AS series,
            fmt_l.ZDISPLAYNAME  AS format,
            lang_l.ZDISPLAYNAME AS language,
            ctry_l.ZDISPLAYNAME AS country,
            cond_l.ZDISPLAYNAME AS condition,
            loc_l.ZDISPLAYNAME  AS location,
            col.ZTITLE          AS collection_name,
            edit_l.ZDISPLAYNAME  AS edition,
            prnt_l.ZDISPLAYNAME  AS printed_by,
            vol_l.ZDISPLAYNAME   AS volume,
            paper_l.ZDISPLAYNAME AS paper_type,
            own_p.ZDISPLAYNAME   AS owner,
            store_l.ZDISPLAYNAME AS store
        FROM ZCOLLECTIBLE c
        LEFT JOIN ZPUBLISHER p   ON p.Z_PK = c.ZPUBLISHER
        LEFT JOIN ZLOOKUPITEMDELEGATE pub_l  ON pub_l.Z_PK = p.ZDELEGATE
        LEFT JOIN ZSERIES s      ON s.Z_PK = c.ZSERIES
        LEFT JOIN ZLOOKUPITEMDELEGATE ser_l  ON ser_l.Z_PK = s.ZDELEGATE
        LEFT JOIN ZFORMAT fo     ON fo.Z_PK = c.ZFORMAT
        LEFT JOIN ZLOOKUPITEMDELEGATE fmt_l  ON fmt_l.Z_PK = fo.ZDELEGATE
        LEFT JOIN ZLANGUAGE la   ON la.Z_PK = c.ZLANGUAGE
        LEFT JOIN ZLOOKUPITEMDELEGATE lang_l ON lang_l.Z_PK = la.ZDELEGATE
        LEFT JOIN ZCOUNTRY ctry  ON ctry.Z_PK = c.ZCOUNTRY
        LEFT JOIN ZLOOKUPITEMDELEGATE ctry_l ON ctry_l.Z_PK = ctry.ZDELEGATE
        LEFT JOIN ZCONDITION cond ON cond.Z_PK = c.ZCONDITION
        LEFT JOIN ZLOOKUPITEMDELEGATE cond_l ON cond_l.Z_PK = cond.ZDELEGATE
        LEFT JOIN ZLOCATION loc  ON loc.Z_PK = c.ZLOCATION
        LEFT JOIN ZLOOKUPITEMDELEGATE loc_l  ON loc_l.Z_PK = loc.ZDELEGATE
        LEFT JOIN ZCOLLECTION col ON col.Z_PK = c.ZCOLLECTION
        LEFT JOIN ZEDITION edit  ON edit.Z_PK = c.ZEDITION
        LEFT JOIN ZLOOKUPITEMDELEGATE edit_l ON edit_l.Z_PK = edit.ZDELEGATE
        LEFT JOIN ZPRINTEDBY prnt ON prnt.Z_PK = c.ZPRINTEDBY
        LEFT JOIN ZLOOKUPITEMDELEGATE prnt_l ON prnt_l.Z_PK = prnt.ZDELEGATE
        LEFT JOIN ZVOLUME vol    ON vol.Z_PK = c.ZVOLUME
        LEFT JOIN ZLOOKUPITEMDELEGATE vol_l  ON vol_l.Z_PK = vol.ZDELEGATE
        LEFT JOIN ZPAPERTYPE paper ON paper.Z_PK = c.ZPAPERTYPE
        LEFT JOIN ZLOOKUPITEMDELEGATE paper_l ON paper_l.Z_PK = paper.ZDELEGATE
        LEFT JOIN ZOWNER own     ON own.Z_PK = c.ZOWNER
        LEFT JOIN ZPERSONDELEGATE own_p ON own_p.Z_PK = own.ZDELEGATE
        LEFT JOIN ZSTORE store   ON store.Z_PK = c.ZSTORE
        LEFT JOIN ZLOOKUPITEMDELEGATE store_l ON store_l.Z_PK = store.ZDELEGATE
        WHERE c.Z_ENT = ?1 AND (?2 IS NULL OR c.ZCOLLECTION = ?2)
        ORDER BY c.Z_PK";

    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query((book_ent, collection_pk))?;

    let mut books = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;

        let front_cover_file: Option<String> = row.get(15)?;
        let back_cover_file: Option<String> = row.get(16)?;
        let front_cover_path = front_cover_file
            .as_ref()
            .and_then(|f| images_dir.map(|d| d.join(f)))
            .map(|p| p.to_string_lossy().into_owned());
        let back_cover_path = back_cover_file
            .as_ref()
            .and_then(|f| images_dir.map(|d| d.join(f)))
            .map(|p| p.to_string_lossy().into_owned());

        let date_added: Option<f64> = row.get(7)?;
        let read_it: Option<i64> = row.get(6)?;

        books.push(Book {
            id,
            title: row.get(1)?,
            subtitle: row.get(2)?,
            authors: authors.get(&id).cloned().unwrap_or_default(),
            isbn: row.get(3)?,
            publisher: row.get(18)?,
            series: row.get(19)?,
            format: row.get(20)?,
            language: row.get(21)?,
            country: row.get(22)?,
            condition: row.get(23)?,
            location: row.get(24)?,
            edition: row.get(26)?,
            printed_by: row.get(27)?,
            volume: row.get(28)?,
            paper_type: row.get(29)?,
            owner: row.get(30)?,
            store: row.get(31)?,
            publication_day: row.get(12)?,
            publication_month: row.get(13)?,
            publication_year: row.get(14)?,
            number_of_pages: row.get(4)?,
            my_rating: row.get(5)?,
            read_it: read_it.map(|v| v != 0),
            date_added: date_added.map(core_data_timestamp_to_iso8601),
            genres: genres.get(&id).cloned().unwrap_or_default(),
            subjects: subjects.get(&id).cloned().unwrap_or_default(),
            tags: tags.get(&id).cloned().unwrap_or_default(),
            notes: row.get(8)?,
            plot: row.get(9)?,
            current_value: row.get(10)?,
            purchase_price: row.get(11)?,
            collection: row.get(25)?,
            quantity: row.get(17)?,
            front_cover_file,
            front_cover_path,
            back_cover_file,
            back_cover_path,
        });
    }
    Ok(books)
}

const EMPTY: &str = "";
const SANS_COLLECTION: &str = "(sans sous-collection)";

/// Minimal, owned subset of a `Book` for the interactive page — cover
/// images as ready-to-use `file://` URLs instead of raw filesystem paths,
/// since the consumer is a web page (whether a static export opened in a
/// browser, or the NBC desktop app's live view). Shared by both, so the
/// page's own JS (`templates/app.js`) only ever has to deal with one shape
/// regardless of which one produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageBook {
    pub id: i64,
    pub title: String,
    pub subtitle: String,
    pub authors: Vec<String>,
    pub isbn: String,
    pub publisher: String,
    pub format: String,
    pub language: String,
    pub country: String,
    pub condition: String,
    pub location: String,
    pub publication_year: String,
    pub number_of_pages: Option<i64>,
    pub my_rating: i64,
    pub genres: Vec<String>,
    pub subjects: Vec<String>,
    pub tags: Vec<String>,
    pub notes: String,
    pub plot: String,
    pub current_value: Option<f64>,
    pub purchase_price: Option<f64>,
    pub collection: String,
    pub quantity: i64,
    /// Full-resolution cover, referenced by `file://` — used by the detail
    /// card, where image quality matters.
    pub cover_url: Option<String>,
    /// Small embedded JPEG (data URI) — used by list/grid row thumbnails.
    /// `None` when the caller skipped thumbnail generation (e.g. the NBC
    /// app's live IPC path, where `templates/app.js` falls back to
    /// `cover_url` through Tauri's asset protocol instead — see
    /// `resolveCoverSrc()`).
    pub thumb_url: Option<String>,
}

/// `thumb_url` is left to the caller (`bc_export` pre-generates a small
/// embedded JPEG per book for its static export; the NBC app's live IPC
/// path passes `None` and lets the page load the real file on demand
/// through Tauri's asset protocol instead — cheaper, and no `image`/
/// `base64` dependency needed on that path).
pub fn book_to_page_book(b: &Book, thumb_url: Option<String>) -> PageBook {
    PageBook {
        id: b.id,
        title: b.title.clone().unwrap_or_else(|| EMPTY.to_string()),
        subtitle: b.subtitle.clone().unwrap_or_else(|| EMPTY.to_string()),
        authors: b.authors.clone(),
        isbn: b.isbn.clone().unwrap_or_else(|| EMPTY.to_string()),
        publisher: b.publisher.clone().unwrap_or_else(|| EMPTY.to_string()),
        format: b.format.clone().unwrap_or_else(|| EMPTY.to_string()),
        language: b.language.clone().unwrap_or_else(|| EMPTY.to_string()),
        country: b.country.clone().unwrap_or_else(|| EMPTY.to_string()),
        condition: b.condition.clone().unwrap_or_else(|| EMPTY.to_string()),
        location: b.location.clone().unwrap_or_else(|| EMPTY.to_string()),
        publication_year: b.publication_year.clone().unwrap_or_else(|| EMPTY.to_string()),
        number_of_pages: b.number_of_pages,
        my_rating: b.my_rating.unwrap_or(0),
        genres: b.genres.clone(),
        subjects: b.subjects.clone(),
        tags: b.tags.clone(),
        notes: b.notes.clone().unwrap_or_else(|| EMPTY.to_string()),
        plot: b.plot.clone().unwrap_or_else(|| EMPTY.to_string()),
        current_value: b.current_value,
        purchase_price: b.purchase_price,
        collection: b.collection.clone().unwrap_or_else(|| SANS_COLLECTION.to_string()),
        quantity: b.quantity,
        cover_url: b.front_cover_path.as_deref().map(file_url),
        thumb_url,
    }
}

/// Convenience used by the NBC app's `load_collection` Tauri command: opens
/// `db_path` read-only, loads every sous-collection, and converts straight
/// to `PageBook` (no thumbnails — see `book_to_page_book`). Encapsulates
/// the `Connection`/`images_dir` plumbing so the Tauri command itself stays
/// a thin wrapper with no `rusqlite` dependency of its own.
pub fn load_page_books(db_path: &Path) -> Result<Vec<PageBook>> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;
    let images_dir = resolve_images_dir(db_path);
    let books = load_books(&conn, images_dir.as_deref(), None)?;
    Ok(books.iter().map(|b| book_to_page_book(b, None)).collect())
}

// --------------------------------------------------------------------
// Write guardrails (step 3 of the add-a-book plan — see `bc_add guard`).
// Nothing in this crate calls `run_write_transaction` yet: the actual
// INSERTs come in a later step, executed against a dedicated writable
// copy, never against `testdata/` (kept deliberately read-only) or the
// user's real collection.
// --------------------------------------------------------------------

/// Checks whether Book Collector itself is currently running, via `pgrep`.
/// Writing to `Book_data` while the app holds it open risks an
/// inconsistent WAL or the app silently overwriting our changes on its
/// next save — this must return `false` before any real write proceeds.
pub fn is_book_collector_running() -> Result<bool> {
    let output = std::process::Command::new("pgrep")
        .args(["-x", "Book Collector"])
        .output()
        .context("exécution de pgrep pour vérifier si Book Collector est lancé")?;
    // pgrep exits 0 when it found at least one matching process, 1 when it
    // found none — both are "successful checks", not failures of pgrep
    // itself, so only its exit code (not a spawn error) distinguishes them.
    Ok(output.status.success())
}

/// Copies `db_path` (and its `-wal`/`-shm` siblings, if present, so the
/// backup is a coherent snapshot) into `backup_dir`, each file suffixed
/// with a timestamp. Returns the path to the backed-up main file.
pub fn backup_sqlite_store(db_path: &Path, backup_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(backup_dir)
        .with_context(|| format!("création du dossier de sauvegarde {}", backup_dir.display()))?;

    let ts_output = std::process::Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .context("obtention de l'horodatage pour la sauvegarde")?;
    let timestamp = String::from_utf8_lossy(&ts_output.stdout).trim().to_string();

    let db_name = db_path
        .file_name()
        .context("nom de fichier de base invalide")?
        .to_string_lossy();
    let dest = backup_dir.join(format!("{db_name}.{timestamp}"));
    std::fs::copy(db_path, &dest)
        .with_context(|| format!("copie de {} vers {}", db_path.display(), dest.display()))?;

    for suffix in ["-wal", "-shm"] {
        let sibling = PathBuf::from(format!("{}{suffix}", db_path.display()));
        if sibling.is_file() {
            let dest_sibling = backup_dir.join(format!("{db_name}.{timestamp}{suffix}"));
            std::fs::copy(&sibling, &dest_sibling)
                .with_context(|| format!("copie de {} vers {}", sibling.display(), dest_sibling.display()))?;
        }
    }

    Ok(dest)
}

/// Runs `f` inside a single SQLite transaction opened read-write on
/// `db_path`: commits only if `f` returns `Ok`, otherwise the transaction
/// is dropped without committing (rusqlite rolls back automatically on
/// drop) and the error propagates — an all-or-nothing write, as required
/// before any real commit to a Core Data store.
///
/// Callers must have already checked `is_book_collector_running() ==
/// false` and taken a `backup_sqlite_store` backup — this function enforces
/// neither on its own.
pub fn run_write_transaction<F>(db_path: &Path, f: F) -> Result<()>
where
    F: FnOnce(&Connection) -> Result<()>,
{
    let mut conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .with_context(|| format!("ouverture en lecture-écriture de {}", db_path.display()))?;
    let tx = conn.transaction().context("démarrage de la transaction")?;
    f(&tx)?;
    tx.commit().context("validation de la transaction")?;
    Ok(())
}

// --------------------------------------------------------------- Add a book
//
// Shared core of `bc_add`'s resolve → plan → commit pipeline (MANUAL.md §7),
// also used directly by the NBC app's "Ajouter un livre" form over Tauri
// IPC. Everything here stays read-only except `write_plan_sql`/
// `commit_add_book`, the only functions that actually execute SQL — same
// contract as `run_write_transaction` above: callers must have already
// checked `is_book_collector_running() == false`.

/// Les champs correspondent à l'onglet "Main" de la fenêtre "Add Book" de
/// Book Collector — seuls ceux dont on connaît déjà la colonne/relation en
/// base sont repris ici. Les noms de champs sont partagés tels quels avec
/// le formulaire JS (`templates/app.js`, `onAddFormSubmit`) et avec le
/// format `new_book.json` lu par `bc_add resolve`.
#[derive(Debug, Deserialize)]
pub struct NewBookInput {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub isbn: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub number_of_pages: Option<i64>,
    #[serde(default)]
    pub publication_year: Option<String>,
    #[serde(default)]
    pub publication_month: Option<String>,
    #[serde(default)]
    pub publication_day: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub location: Option<String>,
    /// `ZLOCNR` — a free-form number/code tied to `location` (e.g. a shelf
    /// or box number), not a lookup: real data in `testdata/` shows this as
    /// plain text (`"2001274321"`, `"96144722"`), not a FK into any table.
    #[serde(default)]
    pub location_number: Option<String>,
    #[serde(default = "default_quantity")]
    pub quantity: i64,
    /// Nom de la sous-collection cible (table ZCOLLECTION) — obligatoire :
    /// contrairement aux autres champs, elle DOIT déjà exister.
    pub collection: String,

    // ---- Onglet "Détails" ----
    /// `ZISSUENUMBER` — populated on ~49% of real rows in `testdata/`
    /// (periodicals/comics, e.g. the "Revues numéro 1" sous-collection) —
    /// confirmed by the user as a real field they expected ("Issue N°")
    /// that was missing from the first pass of this form.
    #[serde(default)]
    pub issue_number: Option<i64>,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub printed_by: Option<String>,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub volume: Option<String>,
    #[serde(default)]
    pub paper_type: Option<String>,
    #[serde(default)]
    pub height: Option<String>,
    #[serde(default)]
    pub width: Option<String>,
    #[serde(default)]
    pub dewey: Option<String>,
    #[serde(default)]
    pub lccn: Option<String>,
    #[serde(default)]
    pub dedication: Option<String>,
    #[serde(default)]
    pub first_edition: Option<bool>,
    #[serde(default)]
    pub printing: Option<i64>,
    #[serde(default)]
    pub abridged: Option<bool>,
    /// `ZBOXSET` — unpopulated in `testdata/` (never used in that
    /// collection), but an unambiguous boolean column name alongside
    /// `first_edition`/`abridged`.
    #[serde(default)]
    pub box_set: Option<bool>,
    #[serde(default)]
    pub audio_length_minutes: Option<i64>,
    #[serde(default)]
    pub original_title: Option<String>,
    #[serde(default)]
    pub original_subtitle: Option<String>,
    #[serde(default)]
    pub original_publisher: Option<String>,
    #[serde(default)]
    pub original_country: Option<String>,
    #[serde(default)]
    pub original_language: Option<String>,
    #[serde(default)]
    pub original_publication_year: Option<String>,
    #[serde(default)]
    pub original_publication_month: Option<String>,
    #[serde(default)]
    pub original_publication_day: Option<String>,

    // ---- Onglet "Personnel" ----
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub store: Option<String>,
    #[serde(default)]
    pub current_value: Option<f64>,
    #[serde(default)]
    pub purchase_price: Option<f64>,
    #[serde(default)]
    pub cover_price: Option<f64>,
    #[serde(default)]
    pub purchase_year: Option<String>,
    #[serde(default)]
    pub purchase_month: Option<String>,
    #[serde(default)]
    pub purchase_day: Option<String>,
    #[serde(default)]
    pub rare: Option<bool>,
    #[serde(default)]
    pub read_it: Option<bool>,
    #[serde(default)]
    pub read_times: Option<i64>,
    #[serde(default)]
    pub read_year: Option<String>,
    #[serde(default)]
    pub read_month: Option<String>,
    #[serde(default)]
    pub read_day: Option<String>,
    #[serde(default)]
    pub my_rating: Option<i64>,
    /// `ZUSERLOOKUP1`/`ZUSERLOOKUP2` — customizable lookup fields (the app
    /// lets the user rename "User 1"/"User 2"), resolved against their own
    /// dedicated lookup tables like any other list field.
    #[serde(default)]
    pub user_lookup1: Option<String>,
    #[serde(default)]
    pub user_lookup2: Option<String>,
    /// `ZUSERTEXT1`/`ZUSERTEXT2` — the same customizable fields, but as
    /// plain free text instead of a lookup.
    #[serde(default)]
    pub user_text1: Option<String>,
    #[serde(default)]
    pub user_text2: Option<String>,

    // ---- Onglet "Intrigue & Notes" ----
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,

    // ---- Onglet "Crédits & Personnages" ----
    #[serde(default)]
    pub credits: Vec<CreditInput>,
    #[serde(default)]
    pub characters: Vec<CharacterInput>,

    // ---- Onglet "Contenus" ----
    #[serde(default)]
    pub contents: Vec<ContentInput>,

    // ---- Onglet "Liens" ----
    #[serde(default)]
    pub links: Vec<LinkInput>,
}

fn default_quantity() -> i64 {
    1
}

#[derive(Debug, Deserialize)]
pub struct CharacterInput {
    pub name: String,
    #[serde(default)]
    pub alias: Option<String>,
}

/// `role` is one of the 11 role identifiers Book Collector 20.3.1 itself
/// uses internally (`Translator`, `CoverArtist`, `Editor`, `Illustrator`,
/// `Ghostwriter`, `ForewordAuthor`, `Photographer`, `Narrator`,
/// `UserRole1`, `UserRole2`, `UserRole3` — see `role_id_for_role`),
/// case-insensitive, or absent/unrecognized for no role (`ZROLEID` left
/// NULL, as before this field existed).
#[derive(Debug, Deserialize)]
pub struct CreditInput {
    pub name: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContentInput {
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub number_of_pages: Option<i64>,
    /// `ZSTORY.ZSTARTPAGE` — the page this content starts on within the
    /// book (meaningful for anthologies/periodicals). `ZSTORY` is entirely
    /// empty in `testdata/`, like the rest of this struct, so this is
    /// unverified against real rows — same confidence level as the other
    /// content fields already here.
    #[serde(default)]
    pub start_page: Option<i64>,
    #[serde(default)]
    pub authors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LinkInput {
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedLookup {
    pub value: String,
    /// `Some(pk)` = une ligne existante sera réutilisée (clé primaire de
    /// l'entité "liste", ex. ZPUBLISHER.Z_PK — pas celle du delegate).
    /// `None` = cette valeur n'existe pas encore et serait créée.
    pub existing_pk: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedBook {
    pub title: String,
    pub subtitle: Option<String>,
    pub isbn: Option<String>,
    pub authors: Vec<ResolvedLookup>,
    pub publisher: Option<ResolvedLookup>,
    pub format: Option<ResolvedLookup>,
    pub language: Option<ResolvedLookup>,
    pub country: Option<ResolvedLookup>,
    pub number_of_pages: Option<i64>,
    pub publication_year: Option<String>,
    pub publication_month: Option<String>,
    pub publication_day: Option<String>,
    pub genres: Vec<ResolvedLookup>,
    pub subjects: Vec<ResolvedLookup>,
    pub location: Option<ResolvedLookup>,
    pub location_number: Option<String>,
    pub quantity: i64,
    /// La sous-collection cible ; `existing_pk` est toujours `Some(...)`
    /// ici — on a déjà vérifié son existence avant de construire ce champ.
    pub collection: ResolvedLookup,

    // ---- Détails ----
    pub issue_number: Option<i64>,
    pub edition: Option<ResolvedLookup>,
    pub printed_by: Option<ResolvedLookup>,
    pub series: Option<ResolvedLookup>,
    pub volume: Option<ResolvedLookup>,
    pub paper_type: Option<ResolvedLookup>,
    pub height: Option<String>,
    pub width: Option<String>,
    pub dewey: Option<String>,
    pub lccn: Option<String>,
    pub dedication: Option<String>,
    pub first_edition: Option<bool>,
    pub printing: Option<i64>,
    pub abridged: Option<bool>,
    pub box_set: Option<bool>,
    pub audio_length_minutes: Option<i64>,
    pub original_title: Option<String>,
    pub original_subtitle: Option<String>,
    pub original_publisher: Option<ResolvedLookup>,
    pub original_country: Option<ResolvedLookup>,
    pub original_language: Option<ResolvedLookup>,
    pub original_publication_year: Option<String>,
    pub original_publication_month: Option<String>,
    pub original_publication_day: Option<String>,

    // ---- Personnel ----
    pub condition: Option<ResolvedLookup>,
    pub owner: Option<ResolvedLookup>,
    pub store: Option<ResolvedLookup>,
    pub current_value: Option<f64>,
    pub purchase_price: Option<f64>,
    pub cover_price: Option<f64>,
    pub purchase_year: Option<String>,
    pub purchase_month: Option<String>,
    pub purchase_day: Option<String>,
    pub rare: Option<bool>,
    pub read_it: Option<bool>,
    pub read_times: Option<i64>,
    pub read_year: Option<String>,
    pub read_month: Option<String>,
    pub read_day: Option<String>,
    pub my_rating: Option<i64>,
    pub user_lookup1: Option<ResolvedLookup>,
    pub user_lookup2: Option<ResolvedLookup>,
    pub user_text1: Option<String>,
    pub user_text2: Option<String>,

    // ---- Intrigue & Notes ----
    pub plot: Option<String>,
    pub notes: Option<String>,

    // ---- Crédits & Personnages ----
    pub credits: Vec<ResolvedCredit>,
    pub characters: Vec<ResolvedCharacter>,

    // ---- Contenus ----
    pub contents: Vec<ResolvedContent>,

    // ---- Liens ----
    pub links: Vec<LinkInput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedCredit {
    pub person: ResolvedLookup,
    pub role_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedCharacter {
    pub person: ResolvedLookup,
    pub alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResolvedContent {
    pub title: String,
    pub subtitle: Option<String>,
    pub number_of_pages: Option<i64>,
    pub start_page: Option<i64>,
    pub authors: Vec<ResolvedLookup>,
}

fn resolve_lookup(conn: &Connection, table: &str, value: &str) -> Result<ResolvedLookup> {
    let existing_pk = resolve_lookup_pk(conn, table, value)
        .with_context(|| format!("résolution de '{value}' dans {table}"))?;
    Ok(ResolvedLookup {
        value: value.to_string(),
        existing_pk,
    })
}

fn resolve_lookup_list(conn: &Connection, table: &str, values: &[String]) -> Result<Vec<ResolvedLookup>> {
    values.iter().map(|v| resolve_lookup(conn, table, v)).collect()
}

fn resolve_person(conn: &Connection, table: &str, value: &str) -> Result<ResolvedLookup> {
    let existing_pk = resolve_person_pk(conn, table, value)
        .with_context(|| format!("résolution de la personne '{value}' dans {table}"))?;
    Ok(ResolvedLookup {
        value: value.to_string(),
        existing_pk,
    })
}

fn resolve_person_list(conn: &Connection, table: &str, values: &[String]) -> Result<Vec<ResolvedLookup>> {
    values.iter().map(|v| resolve_person(conn, table, v)).collect()
}

/// Resolves every "liste"/person field of a new book against `conn` —
/// matches existing values by exact display name (`resolve_lookup_pk`/
/// `resolve_person_pk`), never writes anything. The one exception is the
/// target sous-collection (`collection`), which MUST already exist: unlike
/// every other field, nothing here knows how to create one.
pub fn resolve_book(conn: &Connection, book: NewBookInput) -> Result<ResolvedBook> {
    let collection_pk = resolve_collection_pk(conn, &book.collection).context("sous-collection cible")?;

    let authors = resolve_person_list(conn, "ZAUTHORPERSON", &book.authors)?;

    let publisher = book.publisher.as_deref().map(|v| resolve_lookup(conn, "ZPUBLISHER", v)).transpose()?;
    let format = book.format.as_deref().map(|v| resolve_lookup(conn, "ZFORMAT", v)).transpose()?;
    let language = book.language.as_deref().map(|v| resolve_lookup(conn, "ZLANGUAGE", v)).transpose()?;
    let country = book.country.as_deref().map(|v| resolve_lookup(conn, "ZCOUNTRY", v)).transpose()?;
    let location = book.location.as_deref().map(|v| resolve_lookup(conn, "ZLOCATION", v)).transpose()?;
    let genres = resolve_lookup_list(conn, "ZGENRE", &book.genres)?;
    let subjects = resolve_lookup_list(conn, "ZSUBJECT", &book.subjects)?;

    // ---- Détails ----
    let edition = book.edition.as_deref().map(|v| resolve_lookup(conn, "ZEDITION", v)).transpose()?;
    let printed_by = book.printed_by.as_deref().map(|v| resolve_lookup(conn, "ZPRINTEDBY", v)).transpose()?;
    let series = book.series.as_deref().map(|v| resolve_lookup(conn, "ZSERIES", v)).transpose()?;
    let volume = book.volume.as_deref().map(|v| resolve_lookup(conn, "ZVOLUME", v)).transpose()?;
    let paper_type = book.paper_type.as_deref().map(|v| resolve_lookup(conn, "ZPAPERTYPE", v)).transpose()?;
    let original_publisher = book
        .original_publisher
        .as_deref()
        .map(|v| resolve_lookup(conn, "ZPUBLISHER", v))
        .transpose()?;
    let original_country = book
        .original_country
        .as_deref()
        .map(|v| resolve_lookup(conn, "ZCOUNTRY", v))
        .transpose()?;
    let original_language = book
        .original_language
        .as_deref()
        .map(|v| resolve_lookup(conn, "ZLANGUAGE", v))
        .transpose()?;

    // ---- Personnel ----
    let condition = book.condition.as_deref().map(|v| resolve_lookup(conn, "ZCONDITION", v)).transpose()?;
    let owner = book.owner.as_deref().map(|v| resolve_person(conn, "ZOWNER", v)).transpose()?;
    let store = book.store.as_deref().map(|v| resolve_lookup(conn, "ZSTORE", v)).transpose()?;
    // `ZUSERLOOKUP1`/`ZUSERLOOKUP2` ("Personal" tab's customizable fields)
    // use the same ZLOOKUPITEMDELEGATE flyweight pattern as every other
    // lookup field here — matches the resolved-display-name
    // convention already used for every other lookup field (e.g.
    // `illustratorString`), and real `UserLookup1`/`UserLookup2` entities
    let user_lookup1 = book.user_lookup1.as_deref().map(|v| resolve_lookup(conn, "ZUSERLOOKUP1", v)).transpose()?;
    let user_lookup2 = book.user_lookup2.as_deref().map(|v| resolve_lookup(conn, "ZUSERLOOKUP2", v)).transpose()?;

    // ---- Crédits & Personnages ----
    let credits: Vec<ResolvedCredit> = book
        .credits
        .iter()
        .map(|c| {
            Ok::<_, anyhow::Error>(ResolvedCredit {
                person: resolve_person(conn, "ZCREDITPERSON", &c.name)?,
                role_id: c.role.as_deref().and_then(role_id_for_role),
            })
        })
        .collect::<Result<_>>()?;
    let characters: Vec<ResolvedCharacter> = book
        .characters
        .iter()
        .map(|c| {
            Ok::<_, anyhow::Error>(ResolvedCharacter {
                person: resolve_person(conn, "ZCHARACTERPERSON", &c.name)?,
                alias: c.alias.clone(),
            })
        })
        .collect::<Result<_>>()?;

    // ---- Contenus ----
    let contents: Vec<ResolvedContent> = book
        .contents
        .iter()
        .map(|c| {
            Ok::<_, anyhow::Error>(ResolvedContent {
                title: c.title.clone(),
                subtitle: c.subtitle.clone(),
                number_of_pages: c.number_of_pages,
                start_page: c.start_page,
                authors: resolve_person_list(conn, "ZAUTHORPERSON", &c.authors)?,
            })
        })
        .collect::<Result<_>>()?;

    Ok(ResolvedBook {
        title: book.title,
        subtitle: book.subtitle,
        isbn: book.isbn,
        authors,
        publisher,
        format,
        language,
        country,
        number_of_pages: book.number_of_pages,
        publication_year: book.publication_year,
        publication_month: book.publication_month,
        publication_day: book.publication_day,
        genres,
        subjects,
        location,
        location_number: book.location_number,
        quantity: book.quantity,
        collection: ResolvedLookup {
            value: book.collection,
            existing_pk: Some(collection_pk),
        },
        issue_number: book.issue_number,
        edition,
        printed_by,
        series,
        volume,
        paper_type,
        height: book.height,
        width: book.width,
        dewey: book.dewey,
        lccn: book.lccn,
        dedication: book.dedication,
        first_edition: book.first_edition,
        printing: book.printing,
        abridged: book.abridged,
        box_set: book.box_set,
        audio_length_minutes: book.audio_length_minutes,
        original_title: book.original_title,
        original_subtitle: book.original_subtitle,
        original_publisher,
        original_country,
        original_language,
        original_publication_year: book.original_publication_year,
        original_publication_month: book.original_publication_month,
        original_publication_day: book.original_publication_day,
        condition,
        owner,
        store,
        current_value: book.current_value,
        purchase_price: book.purchase_price,
        cover_price: book.cover_price,
        purchase_year: book.purchase_year,
        purchase_month: book.purchase_month,
        purchase_day: book.purchase_day,
        rare: book.rare,
        read_it: book.read_it,
        read_times: book.read_times,
        read_year: book.read_year,
        read_month: book.read_month,
        read_day: book.read_day,
        my_rating: book.my_rating,
        user_lookup1,
        user_lookup2,
        user_text1: book.user_text1,
        user_text2: book.user_text2,
        plot: book.plot,
        notes: book.notes,
        credits,
        characters,
        contents,
        links: book.links,
    })
}

/// Alloue de nouvelles clés primaires sans jamais écrire en base : lit
/// Z_PRIMARYKEY.Z_MAX une fois par entité (mis en cache), puis incrémente
/// localement. À la fin, `updates()` donne les UPDATE à afficher pour
/// chaque entité dont le compteur a effectivement bougé.
struct PkAllocator<'a> {
    conn: &'a Connection,
    current: HashMap<String, i64>,
    touched: Vec<String>,
}

impl<'a> PkAllocator<'a> {
    fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            current: HashMap::new(),
            touched: Vec::new(),
        }
    }

    fn alloc(&mut self, entity: &str) -> Result<i64> {
        if !self.current.contains_key(entity) {
            let max = entity_max_pk(self.conn, entity)?;
            self.current.insert(entity.to_string(), max);
        }
        let next = self.current.get_mut(entity).unwrap();
        *next += 1;
        if !self.touched.contains(&entity.to_string()) {
            self.touched.push(entity.to_string());
        }
        Ok(*next)
    }

    fn updates(&self) -> Vec<(String, i64)> {
        self.touched
            .iter()
            .map(|e| (e.clone(), self.current[e]))
            .collect()
    }
}

fn sql_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn sql_opt_str(s: &Option<String>) -> String {
    match s {
        Some(v) => sql_quote(v),
        None => "NULL".to_string(),
    }
}

fn sql_opt_i64(v: Option<i64>) -> String {
    match v {
        Some(v) => v.to_string(),
        None => "NULL".to_string(),
    }
}

fn sql_opt_f64(v: Option<f64>) -> String {
    match v {
        Some(v) => v.to_string(),
        None => "NULL".to_string(),
    }
}

/// Core Data stores booleans as 0/1 integers.
fn sql_opt_bool(v: Option<bool>) -> String {
    match v {
        Some(true) => "1".to_string(),
        Some(false) => "0".to_string(),
        None => "NULL".to_string(),
    }
}

/// One "liste" field: Core Data entity name (for Z_PRIMARYKEY lookups) and
/// SQL table name. Both the lookup table itself and the shared
/// ZLOOKUPITEMDELEGATE row behind it need a fresh primary key when the
/// value doesn't exist yet.
struct LookupKind {
    entity: &'static str,
    table: &'static str,
}

const PUBLISHER: LookupKind = LookupKind { entity: "Publisher", table: "ZPUBLISHER" };
const FORMAT: LookupKind = LookupKind { entity: "Format", table: "ZFORMAT" };
const LANGUAGE: LookupKind = LookupKind { entity: "Language", table: "ZLANGUAGE" };
const COUNTRY: LookupKind = LookupKind { entity: "Country", table: "ZCOUNTRY" };
const LOCATION: LookupKind = LookupKind { entity: "Location", table: "ZLOCATION" };
const GENRE: LookupKind = LookupKind { entity: "Genre", table: "ZGENRE" };
const SUBJECT: LookupKind = LookupKind { entity: "Subject", table: "ZSUBJECT" };
const EDITION: LookupKind = LookupKind { entity: "Edition", table: "ZEDITION" };
const PRINTEDBY: LookupKind = LookupKind { entity: "PrintedBy", table: "ZPRINTEDBY" };
const SERIES: LookupKind = LookupKind { entity: "Series", table: "ZSERIES" };
const VOLUME: LookupKind = LookupKind { entity: "Volume", table: "ZVOLUME" };
const PAPERTYPE: LookupKind = LookupKind { entity: "PaperType", table: "ZPAPERTYPE" };
const CONDITION: LookupKind = LookupKind { entity: "Condition", table: "ZCONDITION" };
const STORE: LookupKind = LookupKind { entity: "Store", table: "ZSTORE" };
const USERLOOKUP1: LookupKind = LookupKind { entity: "UserLookup1", table: "ZUSERLOOKUP1" };
const USERLOOKUP2: LookupKind = LookupKind { entity: "UserLookup2", table: "ZUSERLOOKUP2" };

/// Maps a credit role name to Book Collector 20.3.1's own internal
/// `ZROLEID` integer — a fixed, known mapping (both directions
/// cross-checked against each other), since
///
///
/// this mapping exists only as compiled-in constants, not as a row in any
/// table in `Book_data` itself. `CoverDesigner` is a recognized synonym for
/// `CoverArtist` (role 2) in the real app. Case-insensitive; an
/// unrecognized name returns `None` (ZROLEID left NULL), same as before
/// this field was resolved at all.
fn role_id_for_role(role: &str) -> Option<i64> {
    match role.to_ascii_lowercase().as_str() {
        "translator" => Some(1),
        "coverartist" | "coverdesigner" => Some(2),
        "editor" => Some(3),
        "illustrator" => Some(4),
        "ghostwriter" => Some(5),
        "forewordauthor" => Some(6),
        "photographer" => Some(7),
        "narrator" => Some(8),
        "userrole1" => Some(9),
        "userrole2" => Some(10),
        "userrole3" => Some(11),
        _ => None,
    }
}

/// Same idea as `LookupKind`, but for "person" flyweight fields (shared
/// `ZPERSONDELEGATE` instead of `ZLOOKUPITEMDELEGATE`) — authors, credits,
/// characters, owner.
struct PersonKind {
    entity: &'static str,
    table: &'static str,
}

const AUTHOR_PERSON: PersonKind = PersonKind { entity: "AuthorPerson", table: "ZAUTHORPERSON" };
const CREDIT_PERSON: PersonKind = PersonKind { entity: "CreditPerson", table: "ZCREDITPERSON" };
const CHARACTER_PERSON: PersonKind = PersonKind { entity: "CharacterPerson", table: "ZCHARACTERPERSON" };
const OWNER_PERSON: PersonKind = PersonKind { entity: "Owner", table: "ZOWNER" };

/// Emits the two INSERTs needed to create a brand-new person value (a
/// `ZPERSONDELEGATE` row holding the display name, plus the person entity's
/// own row pointing at it), and returns the person entity's new primary key.
fn plan_new_person(alloc: &mut PkAllocator, kind: &PersonKind, value: &str, sql: &mut String) -> Result<i64> {
    let delegate_ent = entity_ent_id(alloc.conn, "PersonDelegate")?;
    let table_ent = entity_ent_id(alloc.conn, kind.entity)?;
    let delegate_pk = alloc.alloc("PersonDelegate")?;
    let table_pk = alloc.alloc(kind.entity)?;

    sql.push_str(&format!(
        "INSERT INTO ZPERSONDELEGATE (Z_PK, Z_ENT, Z_OPT, ZDISPLAYNAME, ZSORTNAME) \
         VALUES ({delegate_pk}, {delegate_ent}, 1, {v}, {v});\n",
        v = sql_quote(value)
    ));
    sql.push_str(&format!(
        "INSERT INTO {} (Z_PK, Z_ENT, Z_OPT, ZDELEGATE) VALUES ({table_pk}, {table_ent}, 1, {delegate_pk});\n",
        kind.table
    ));
    Ok(table_pk)
}

/// Resolves a (possibly-new) single-valued person field to its primary key,
/// emitting INSERTs first if needed.
fn plan_person_fk(
    alloc: &mut PkAllocator,
    kind: &PersonKind,
    field: &Option<ResolvedLookup>,
    sql: &mut String,
) -> Result<String> {
    match field {
        None => Ok("NULL".to_string()),
        Some(r) => match r.existing_pk {
            Some(pk) => Ok(pk.to_string()),
            None => Ok(plan_new_person(alloc, kind, &r.value, sql)?.to_string()),
        },
    }
}

/// Resolves a list of (possibly-new) persons (authors, credits...) to their
/// primary keys, in order, emitting INSERTs first for any new ones.
fn plan_person_list(
    alloc: &mut PkAllocator,
    kind: &PersonKind,
    values: &[ResolvedLookup],
    sql: &mut String,
) -> Result<Vec<i64>> {
    values
        .iter()
        .map(|r| match r.existing_pk {
            Some(pk) => Ok(pk),
            None => plan_new_person(alloc, kind, &r.value, sql),
        })
        .collect()
}

/// Emits the two INSERTs needed to create a brand-new "liste" value (a
/// ZLOOKUPITEMDELEGATE row holding the display name, plus the lookup
/// entity's own row pointing at it), and returns the lookup entity's new
/// primary key — the one a ZCOLLECTIBLE/etc. foreign key column needs.
fn plan_new_lookup(alloc: &mut PkAllocator, kind: &LookupKind, value: &str, sql: &mut String) -> Result<i64> {
    let delegate_ent = entity_ent_id(alloc.conn, "LookupItemDelegate")?;
    let table_ent = entity_ent_id(alloc.conn, kind.entity)?;
    let delegate_pk = alloc.alloc("LookupItemDelegate")?;
    let table_pk = alloc.alloc(kind.entity)?;

    sql.push_str(&format!(
        "INSERT INTO ZLOOKUPITEMDELEGATE (Z_PK, Z_ENT, Z_OPT, ZDISPLAYNAME, ZNAME, ZSORTNAME) \
         VALUES ({delegate_pk}, {delegate_ent}, 1, {v}, {v}, {v});\n",
        v = sql_quote(value)
    ));
    sql.push_str(&format!(
        "INSERT INTO {} (Z_PK, Z_ENT, Z_OPT, ZDELEGATE) VALUES ({table_pk}, {table_ent}, 1, {delegate_pk});\n",
        kind.table
    ));
    Ok(table_pk)
}

/// Resolves a (possibly-new) lookup field to the SQL fragment for its
/// ZCOLLECTIBLE foreign key column, emitting INSERTs first if needed.
fn plan_lookup_fk(
    alloc: &mut PkAllocator,
    kind: &LookupKind,
    field: &Option<ResolvedLookup>,
    sql: &mut String,
) -> Result<String> {
    match field {
        None => Ok("NULL".to_string()),
        Some(r) => match r.existing_pk {
            Some(pk) => Ok(pk.to_string()),
            None => Ok(plan_new_lookup(alloc, kind, &r.value, sql)?.to_string()),
        },
    }
}

/// Builds the exact INSERT/UPDATE SQL for a resolved new book — new primary
/// keys, new "liste"/person values, join rows — but never executes it (see
/// `write_plan_sql` for the one function that does). Wrapped in its own
/// `BEGIN TRANSACTION;`/`COMMIT;` for a readable standalone preview;
/// `write_plan_sql` strips that wrapper back out before executing, since
/// the write path opens its own transaction (`run_write_transaction`).
pub fn plan_book_sql(conn: &Connection, book: &ResolvedBook) -> Result<String> {
    let mut alloc = PkAllocator::new(conn);
    let mut sql = String::new();
    sql.push_str("-- Plan d'insertion — APERÇU, RIEN N'EST EXÉCUTÉ.\n");
    sql.push_str("BEGIN TRANSACTION;\n\n");

    // 1) Nouvelles valeurs "liste" (éditeur, format, langue, pays, emplacement...).
    let publisher_fk = plan_lookup_fk(&mut alloc, &PUBLISHER, &book.publisher, &mut sql)?;
    let format_fk = plan_lookup_fk(&mut alloc, &FORMAT, &book.format, &mut sql)?;
    let language_fk = plan_lookup_fk(&mut alloc, &LANGUAGE, &book.language, &mut sql)?;
    let country_fk = plan_lookup_fk(&mut alloc, &COUNTRY, &book.country, &mut sql)?;
    let location_fk = plan_lookup_fk(&mut alloc, &LOCATION, &book.location, &mut sql)?;
    let edition_fk = plan_lookup_fk(&mut alloc, &EDITION, &book.edition, &mut sql)?;
    let printed_by_fk = plan_lookup_fk(&mut alloc, &PRINTEDBY, &book.printed_by, &mut sql)?;
    let series_fk = plan_lookup_fk(&mut alloc, &SERIES, &book.series, &mut sql)?;
    let volume_fk = plan_lookup_fk(&mut alloc, &VOLUME, &book.volume, &mut sql)?;
    let paper_type_fk = plan_lookup_fk(&mut alloc, &PAPERTYPE, &book.paper_type, &mut sql)?;
    let original_publisher_fk = plan_lookup_fk(&mut alloc, &PUBLISHER, &book.original_publisher, &mut sql)?;
    let original_country_fk = plan_lookup_fk(&mut alloc, &COUNTRY, &book.original_country, &mut sql)?;
    let original_language_fk = plan_lookup_fk(&mut alloc, &LANGUAGE, &book.original_language, &mut sql)?;
    let condition_fk = plan_lookup_fk(&mut alloc, &CONDITION, &book.condition, &mut sql)?;
    let store_fk = plan_lookup_fk(&mut alloc, &STORE, &book.store, &mut sql)?;
    let owner_fk = plan_person_fk(&mut alloc, &OWNER_PERSON, &book.owner, &mut sql)?;
    let user_lookup1_fk = plan_lookup_fk(&mut alloc, &USERLOOKUP1, &book.user_lookup1, &mut sql)?;
    let user_lookup2_fk = plan_lookup_fk(&mut alloc, &USERLOOKUP2, &book.user_lookup2, &mut sql)?;

    // 2) Nouveaux auteurs (ZPERSONDELEGATE + ZAUTHORPERSON), avant la fiche
    //    livre elle-même puisque ZAUTHOR a besoin du Z_PK du livre.
    let author_person_pks = plan_person_list(&mut alloc, &AUTHOR_PERSON, &book.authors, &mut sql)?;

    // 3) La fiche livre elle-même (ZCOLLECTIBLE, Z_ENT="Book"). Chaque livre
    //    réel possède sa propre ligne ZITEMDELEGATE compagnon (confirmé sur
    //    100% des fiches de testdata/) — motif générique déjà vu pour ZLINK,
    //    mais ici un seul wrapper par livre, sans ZONLINELINK renseigné.
    let book_ent = entity_ent_id(conn, "Book")?;
    let book_pk = alloc.alloc("Collectible")?; // Book partage le compteur de l'entité racine.
    let book_itemdelegate_ent = entity_ent_id(conn, "ItemDelegate")?;
    let book_itemdelegate_pk = alloc.alloc("ItemDelegate")?;
    sql.push('\n');
    sql.push_str(&format!(
        "INSERT INTO ZITEMDELEGATE (Z_PK, Z_ENT, Z_OPT, ZUNIQUEID) VALUES ({book_itemdelegate_pk}, {book_itemdelegate_ent}, 1, 0);\n"
    ));
    sql.push_str(&format!(
        "INSERT INTO ZCOLLECTIBLE (\n\
         \x20 Z_PK, Z_ENT, Z_OPT, ZITEMDELEGATE, ZCOLLECTION, ZCOLLECTIONSTATUS, ZQUANTITY, ZISSUENUMBER,\n\
         \x20 ZTITLE, ZTITLESORT, ZSUBTITLE, ZISBN,\n\
         \x20 ZPUBLISHER, ZFORMAT, ZLANGUAGE, ZCOUNTRY, ZLOCATION, ZLOCNR,\n\
         \x20 ZNUMBEROFPAGES, ZPUBLICATIONYEAR, ZPUBLICATIONMONTH, ZPUBLICATIONDAY,\n\
         \x20 ZEDITION, ZPRINTEDBY, ZSERIES, ZVOLUME, ZPAPERTYPE,\n\
         \x20 ZHEIGHT, ZWIDTH, ZDEWEY, ZLCCN, ZDEDICATION,\n\
         \x20 ZFIRSTEDITION, ZPRINTING, ZABRIDGED, ZBOXSET, ZAUDIOLENGTHMINUTES, ZAUDIOLENGTH,\n\
         \x20 ZORIGINALTITLE, ZORIGINALSUBTITLE, ZORIGINALPUBLISHER, ZORIGINALCOUNTRY, ZORIGINALLANGUAGE,\n\
         \x20 ZORIGINALPUBLICATIONYEAR, ZORIGINALPUBLICATIONMONTH, ZORIGINALPUBLICATIONDAY,\n\
         \x20 ZCONDITION, ZOWNER, ZSTORE, ZCURRENTVALUE, ZPURCHASEPRICE, ZCOVERPRICE,\n\
         \x20 ZPURCHASEYEAR, ZPURCHASEMONTH, ZPURCHASEDAY,\n\
         \x20 ZRARE, ZREADIT, ZREADTIMES, ZREADYEAR, ZREADMONTH, ZREADDAY, ZMYRATING,\n\
         \x20 ZUSERLOOKUP1, ZUSERLOOKUP2, ZUSERTEXT1, ZUSERTEXT2,\n\
         \x20 ZPLOT, ZNOTES\n\
         ) VALUES (\n\
         \x20 {book_pk}, {book_ent}, 1, {book_itemdelegate_pk}, {collection_pk}, 2, {quantity}, {issue_number},\n\
         \x20 {title}, {title_sort}, {subtitle}, {isbn},\n\
         \x20 {publisher_fk}, {format_fk}, {language_fk}, {country_fk}, {location_fk}, {location_number},\n\
         \x20 {pages}, {year}, {month}, {day},\n\
         \x20 {edition_fk}, {printed_by_fk}, {series_fk}, {volume_fk}, {paper_type_fk},\n\
         \x20 {height}, {width}, {dewey}, {lccn}, {dedication},\n\
         \x20 {first_edition}, {printing}, {abridged}, {box_set}, {audio_length_minutes}, {audio_length},\n\
         \x20 {original_title}, {original_subtitle}, {original_publisher_fk}, {original_country_fk}, {original_language_fk},\n\
         \x20 {original_year}, {original_month}, {original_day},\n\
         \x20 {condition_fk}, {owner_fk}, {store_fk}, {current_value}, {purchase_price}, {cover_price},\n\
         \x20 {purchase_year}, {purchase_month}, {purchase_day},\n\
         \x20 {rare}, {read_it}, {read_times}, {read_year}, {read_month}, {read_day}, {my_rating},\n\
         \x20 {user_lookup1_fk}, {user_lookup2_fk}, {user_text1}, {user_text2},\n\
         \x20 {plot}, {notes}\n\
         );\n",
        collection_pk = book.collection.existing_pk.context("sous-collection non résolue")?,
        quantity = book.quantity,
        issue_number = sql_opt_i64(book.issue_number),
        title = sql_quote(&book.title),
        title_sort = sql_quote(&book.title), // simplification — voir note en bas de sortie
        subtitle = sql_opt_str(&book.subtitle),
        isbn = sql_opt_str(&book.isbn),
        location_number = sql_opt_str(&book.location_number),
        pages = sql_opt_i64(book.number_of_pages),
        year = sql_opt_str(&book.publication_year),
        month = sql_opt_str(&book.publication_month),
        day = sql_opt_str(&book.publication_day),
        height = sql_opt_str(&book.height),
        width = sql_opt_str(&book.width),
        dewey = sql_opt_str(&book.dewey),
        lccn = sql_opt_str(&book.lccn),
        dedication = sql_opt_str(&book.dedication),
        first_edition = sql_opt_bool(book.first_edition),
        printing = sql_opt_i64(book.printing),
        abridged = sql_opt_bool(book.abridged),
        box_set = sql_opt_bool(book.box_set),
        audio_length_minutes = sql_opt_i64(book.audio_length_minutes),
        // ZAUDIOLENGTH has no independent form field: every populated row in
        // testdata/ mirrors ZAUDIOLENGTHMINUTES exactly (both "0" wherever
        // sampled) — written here as the same value in string form rather
        // than left NULL, to match the app's own observed convention.
        audio_length = sql_opt_str(&book.audio_length_minutes.map(|m| m.to_string())),
        original_title = sql_opt_str(&book.original_title),
        original_subtitle = sql_opt_str(&book.original_subtitle),
        original_year = sql_opt_str(&book.original_publication_year),
        original_month = sql_opt_str(&book.original_publication_month),
        original_day = sql_opt_str(&book.original_publication_day),
        current_value = sql_opt_f64(book.current_value),
        purchase_price = sql_opt_f64(book.purchase_price),
        cover_price = sql_opt_f64(book.cover_price),
        purchase_year = sql_opt_str(&book.purchase_year),
        purchase_month = sql_opt_str(&book.purchase_month),
        purchase_day = sql_opt_str(&book.purchase_day),
        rare = sql_opt_bool(book.rare),
        read_it = sql_opt_bool(book.read_it),
        read_times = sql_opt_i64(book.read_times),
        read_year = sql_opt_str(&book.read_year),
        read_month = sql_opt_str(&book.read_month),
        read_day = sql_opt_str(&book.read_day),
        my_rating = sql_opt_i64(book.my_rating),
        user_text1 = sql_opt_str(&book.user_text1),
        user_text2 = sql_opt_str(&book.user_text2),
        plot = sql_opt_str(&book.plot),
        notes = sql_opt_str(&book.notes),
    ));

    // 4) Auteurs : lignes de jointure ZAUTHOR(ZBOOK, ZAUTHORPERSON, ZRANK).
    let author_ent = entity_ent_id(conn, "Author")?;
    for (i, person_pk) in author_person_pks.iter().enumerate() {
        let author_pk = alloc.alloc("Author")?;
        sql.push_str(&format!(
            "INSERT INTO ZAUTHOR (Z_PK, Z_ENT, Z_OPT, ZRANK, ZAUTHORPERSON, ZBOOK) \
             VALUES ({author_pk}, {author_ent}, 1, {rank}, {person_pk}, {book_pk});\n",
            rank = i + 1
        ));
    }

    // 5) Genres / sujets : nouvelles valeurs puis lignes de jointure.
    for g in &book.genres {
        let genre_pk = match g.existing_pk {
            Some(pk) => pk,
            None => plan_new_lookup(&mut alloc, &GENRE, &g.value, &mut sql)?,
        };
        sql.push_str(&format!(
            "INSERT INTO Z_7GENRES (Z_7BASEBOOKCOLLECTIBLES1, Z_18GENRES) VALUES ({book_pk}, {genre_pk});\n"
        ));
    }
    for s in &book.subjects {
        let subject_pk = match s.existing_pk {
            Some(pk) => pk,
            None => plan_new_lookup(&mut alloc, &SUBJECT, &s.value, &mut sql)?,
        };
        sql.push_str(&format!(
            "INSERT INTO Z_8SUBJECTS (Z_8BOOKS1, Z_38SUBJECTS) VALUES ({book_pk}, {subject_pk});\n"
        ));
    }

    // 6) Crédits : ZCREATOR, rattachés directement au livre (ZBASEBOOKCOLLECTIBLE
    //    + sa colonne jumelle Z7_BASEBOOKCOLLECTIBLE, qui porte le Z_ENT de la
    //    cible — motif confirmé empiriquement sur ZLINK, § non couvert plus bas
    //    pour ce qui reste incertain ici). ZROLEID est résolu via
    //    `role_id_for_role` (mapping connu) plutôt que laissé NULL.
    let creator_ent = entity_ent_id(conn, "Creator")?;
    let creator_ent = entity_ent_id(conn, "Creator")?;
    for (i, c) in book.credits.iter().enumerate() {
        let person_pk = match c.person.existing_pk {
            Some(pk) => pk,
            None => plan_new_person(&mut alloc, &CREDIT_PERSON, &c.person.value, &mut sql)?,
        };
        let creator_pk = alloc.alloc("Creator")?;
        sql.push_str(&format!(
            "INSERT INTO ZCREATOR (Z_PK, Z_ENT, Z_OPT, ZRANK, ZROLEID, ZCREDITPERSON, ZBASEBOOKCOLLECTIBLE, Z7_BASEBOOKCOLLECTIBLE) \
             VALUES ({creator_pk}, {creator_ent}, 1, {rank}, {role_id}, {person_pk}, {book_pk}, {book_ent});\n",
            rank = i + 1,
            role_id = sql_opt_i64(c.role_id),
        ));
    }

    // 7) Personnages : ZCHARACTER, même motif que les crédits. ZALIAS est
    //    porté par ZCHARACTERPERSON lui-même (valeur partagée) : il n'est
    //    donc écrit que lors de la création d'un nouveau personnage, jamais
    //    en réutilisant un personnage existant.
    let character_ent = entity_ent_id(conn, "Character")?;
    let character_person_ent = entity_ent_id(conn, CHARACTER_PERSON.entity)?;
    for (i, c) in book.characters.iter().enumerate() {
        let person_pk = match c.person.existing_pk {
            Some(pk) => pk,
            None => {
                let delegate_ent = entity_ent_id(conn, "PersonDelegate")?;
                let delegate_pk = alloc.alloc("PersonDelegate")?;
                let person_pk = alloc.alloc(CHARACTER_PERSON.entity)?;
                sql.push_str(&format!(
                    "INSERT INTO ZPERSONDELEGATE (Z_PK, Z_ENT, Z_OPT, ZDISPLAYNAME, ZSORTNAME) \
                     VALUES ({delegate_pk}, {delegate_ent}, 1, {v}, {v});\n",
                    v = sql_quote(&c.person.value)
                ));
                sql.push_str(&format!(
                    "INSERT INTO {} (Z_PK, Z_ENT, Z_OPT, ZDELEGATE, ZALIAS) \
                     VALUES ({person_pk}, {character_person_ent}, 1, {delegate_pk}, {alias});\n",
                    CHARACTER_PERSON.table,
                    alias = sql_opt_str(&c.alias)
                ));
                person_pk
            }
        };
        let character_pk = alloc.alloc("Character")?;
        sql.push_str(&format!(
            "INSERT INTO ZCHARACTER (Z_PK, Z_ENT, Z_OPT, ZRANK, ZCHARACTERPERSON, ZBASEBOOKCOLLECTIBLE, Z7_BASEBOOKCOLLECTIBLE) \
             VALUES ({character_pk}, {character_ent}, 1, {rank}, {person_pk}, {book_pk}, {book_ent});\n",
            rank = i + 1
        ));
    }

    // 8) Contenus : ZSTORY (une "histoire" dans le livre), avec ses propres
    //    auteurs rattachés via ZAUTHOR.ZSTORY au lieu de ZAUTHOR.ZBOOK —
    //    même table de jointure, déjà vérifiée en conditions réelles pour
    //    les auteurs du livre lui-même (étape 4).
    let story_ent = entity_ent_id(conn, "Story")?;
    for (ci, c) in book.contents.iter().enumerate() {
        let story_pk = alloc.alloc("Story")?;
        sql.push_str(&format!(
            "INSERT INTO ZSTORY (Z_PK, Z_ENT, Z_OPT, ZRANK, ZBOOK, ZNUMBEROFPAGES, ZSTARTPAGE, ZTITLE, ZTITLESORT, ZSUBTITLE) \
             VALUES ({story_pk}, {story_ent}, 1, {rank}, {book_pk}, {pages}, {start_page}, {title}, {title_sort}, {subtitle});\n",
            rank = ci + 1,
            pages = sql_opt_i64(c.number_of_pages),
            start_page = sql_opt_i64(c.start_page),
            title = sql_quote(&c.title),
            title_sort = sql_quote(&c.title),
            subtitle = sql_opt_str(&c.subtitle),
        ));

        let content_author_pks = plan_person_list(&mut alloc, &AUTHOR_PERSON, &c.authors, &mut sql)?;
        for (ai, person_pk) in content_author_pks.iter().enumerate() {
            let content_author_pk = alloc.alloc("Author")?;
            sql.push_str(&format!(
                "INSERT INTO ZAUTHOR (Z_PK, Z_ENT, Z_OPT, ZRANK, ZAUTHORPERSON, ZSTORY) \
                 VALUES ({content_author_pk}, {author_ent}, 1, {rank}, {person_pk}, {story_pk});\n",
                rank = ai + 1
            ));
        }
    }

    // 9) Liens : ZLINK + une ligne ZITEMDELEGATE compagnon (motif confirmé
    //    sur 832 liens réels existants dans cette collection — ZTYPEID=4
    //    pour 100% d'entre eux, repris tel quel ici).
    let link_ent = entity_ent_id(conn, "Link")?;
    let itemdelegate_ent = entity_ent_id(conn, "ItemDelegate")?;
    for (i, l) in book.links.iter().enumerate() {
        let link_pk = alloc.alloc("Link")?;
        let itemdelegate_pk = alloc.alloc("ItemDelegate")?;
        sql.push_str(&format!(
            "INSERT INTO ZITEMDELEGATE (Z_PK, Z_ENT, Z_OPT, ZUNIQUEID, ZONLINELINK) \
             VALUES ({itemdelegate_pk}, {itemdelegate_ent}, 1, 0, {link_pk});\n"
        ));
        sql.push_str(&format!(
            "INSERT INTO ZLINK (Z_PK, Z_ENT, Z_OPT, ZRANK, ZTYPEID, ZBASEBOOKCOLLECTIBLE1, Z7_BASEBOOKCOLLECTIBLE1, ZITEMDELEGATE1, ZDESCRIPTION, ZURL) \
             VALUES ({link_pk}, {link_ent}, 1, {rank}, 4, {book_pk}, {book_ent}, {itemdelegate_pk}, {desc}, {url});\n",
            rank = i + 1,
            desc = sql_opt_str(&l.description),
            url = sql_quote(&l.url),
        ));
    }

    // 10) Compteurs Z_PRIMARYKEY à mettre à jour pour chaque entité touchée.
    sql.push('\n');
    for (entity, new_max) in alloc.updates() {
        sql.push_str(&format!(
            "UPDATE Z_PRIMARYKEY SET Z_MAX = {new_max} WHERE Z_NAME = {};\n",
            sql_quote(&entity)
        ));
    }

    sql.push_str("\nCOMMIT;\n");

    Ok(sql)
}

/// Refuses paths under `~/Documents/Collectorz/...` — the known location
/// of the user's real collection — unless the caller decides to override
/// (the CLI's `--force` + typed "OUI"; the NBC app never overrides this at
/// all — see `app/src-tauri/src/lib.rs`). Cheap defense in depth on top of
/// the standing rule to only ever write to a dedicated test/independent
/// copy.
///
/// `db_path` is always canonicalized by `resolve_db_path` before reaching
/// here, so `real_prefix` must be canonicalized the same way before the
/// comparison — otherwise a symlink anywhere in `$HOME` (e.g. macOS's
/// `/var` -> `/private/var`) silently defeats the whole check: the two
/// paths would represent the same real location on disk but never compare
/// equal as plain strings. Falls back to the raw (non-canonicalized) prefix
/// if canonicalization fails (e.g. the directory doesn't exist yet) rather
/// than disabling the check entirely.
pub fn looks_like_real_collection(db_path: &Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let real_prefix = PathBuf::from(home).join("Documents").join("Collectorz");
    let canonical_prefix = real_prefix.canonicalize().unwrap_or_else(|_| real_prefix.clone());
    db_path.starts_with(&canonical_prefix).then_some(real_prefix)
}

/// Strips a plan's own transaction wrapper (`BEGIN TRANSACTION;` ...
/// `COMMIT;`) and comments from its SQL, since `run_write_transaction` opens
/// its own transaction — keeps just the INSERT/UPDATE statements to execute.
pub fn strip_plan_wrapper(raw_sql: &str) -> String {
    raw_sql
        .lines()
        .filter(|line| {
            let t = line.trim();
            !(t.is_empty() || t.starts_with("--") || t == "BEGIN TRANSACTION;" || t == "COMMIT;")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Executes `raw_sql` (as produced by `plan_book_sql`, or `bc_add plan`'s
/// own CLI output) against `db_path` inside one transaction, after backing
/// up to `backup_dir` first. Callers must have already checked
/// `is_book_collector_running() == false` — same contract as
/// `run_write_transaction`, which this wraps. Returns the backup path.
pub fn write_plan_sql(db_path: &Path, backup_dir: &Path, raw_sql: &str) -> Result<PathBuf> {
    // Checked up front rather than left to surface as a raw SQLite error
    // ("attempt to write a readonly database") once the write is already
    // attempted — `testdata/` is deliberately `chmod 444` (see
    // `refresh-testdata.sh`), so this is the expected, common case of
    // someone testing against the wrong copy, not a corrupt-state bug.
    let is_readonly = std::fs::metadata(db_path).map(|m| m.permissions().readonly()).unwrap_or(false);
    if is_readonly {
        bail!(
            "{} est en lecture seule — impossible d'écrire. S'il s'agit d'une copie de test, \
             utilisez une copie inscriptible (ex. testdata-rw/, via ./refresh-testdata-rw.sh) ; \
             sinon, vérifiez les permissions du fichier.",
            db_path.display()
        );
    }

    let backup_path =
        backup_sqlite_store(db_path, backup_dir).context("création de la sauvegarde avant écriture")?;
    let statements = strip_plan_wrapper(raw_sql);
    run_write_transaction(db_path, |conn| {
        conn.execute_batch(&statements).context("exécution du plan SQL")
    })?;
    Ok(backup_path)
}

/// One "could this create a brand-new value?" field from a resolved book,
/// shaped for a human review step (the NBC app's add-book confirmation)
/// rather than for `plan_book_sql`'s internals.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedFieldSummary {
    pub label: String,
    pub value: String,
    pub is_new: bool,
}

fn summarize_one(label: &str, r: &ResolvedLookup, out: &mut Vec<ResolvedFieldSummary>) {
    out.push(ResolvedFieldSummary {
        label: label.to_string(),
        value: r.value.clone(),
        is_new: r.existing_pk.is_none(),
    });
}

/// Flattens every "liste"/person field of a resolved book — the ones that
/// can silently create a brand-new Publisher/Genre/etc. entry on a typo —
/// into a single reviewable list. Plain text/number fields are skipped:
/// they can't create unwanted taxonomy entries, so there's nothing to flag.
pub fn resolved_book_summary(r: &ResolvedBook) -> Vec<ResolvedFieldSummary> {
    let mut out = Vec::new();
    for a in &r.authors {
        summarize_one("Auteur", a, &mut out);
    }
    if let Some(v) = &r.publisher {
        summarize_one("Éditeur", v, &mut out);
    }
    if let Some(v) = &r.format {
        summarize_one("Format", v, &mut out);
    }
    if let Some(v) = &r.language {
        summarize_one("Langue", v, &mut out);
    }
    if let Some(v) = &r.country {
        summarize_one("Pays", v, &mut out);
    }
    if let Some(v) = &r.location {
        summarize_one("Emplacement", v, &mut out);
    }
    for g in &r.genres {
        summarize_one("Genre", g, &mut out);
    }
    for s in &r.subjects {
        summarize_one("Sujet", s, &mut out);
    }
    if let Some(v) = &r.edition {
        summarize_one("Édition", v, &mut out);
    }
    if let Some(v) = &r.printed_by {
        summarize_one("Imprimé par", v, &mut out);
    }
    if let Some(v) = &r.series {
        summarize_one("Série", v, &mut out);
    }
    if let Some(v) = &r.volume {
        summarize_one("Volume", v, &mut out);
    }
    if let Some(v) = &r.paper_type {
        summarize_one("Type de papier", v, &mut out);
    }
    if let Some(v) = &r.original_publisher {
        summarize_one("Éditeur original", v, &mut out);
    }
    if let Some(v) = &r.original_country {
        summarize_one("Pays original", v, &mut out);
    }
    if let Some(v) = &r.original_language {
        summarize_one("Langue originale", v, &mut out);
    }
    if let Some(v) = &r.condition {
        summarize_one("État", v, &mut out);
    }
    if let Some(v) = &r.owner {
        summarize_one("Propriétaire", v, &mut out);
    }
    if let Some(v) = &r.store {
        summarize_one("Magasin", v, &mut out);
    }
    if let Some(v) = &r.user_lookup1 {
        summarize_one("Champ personnalisé 1", v, &mut out);
    }
    if let Some(v) = &r.user_lookup2 {
        summarize_one("Champ personnalisé 2", v, &mut out);
    }
    for c in &r.credits {
        summarize_one("Crédit", &c.person, &mut out);
    }
    for c in &r.characters {
        summarize_one("Personnage", &c.person, &mut out);
    }
    out
}

/// Preview returned to the NBC app's confirmation step before any write —
/// what the book's title resolved to, and which fields (if any) would
/// create brand-new taxonomy entries.
#[derive(Debug, Serialize)]
pub struct AddBookPreview {
    pub title: String,
    pub fields: Vec<ResolvedFieldSummary>,
    pub new_count: usize,
}

/// Read-only half of the NBC app's "Ajouter un livre" flow: opens `db_path`
/// read-only, resolves `input` against it, and returns a human-reviewable
/// preview. Never writes anything.
pub fn prepare_add_book(db_path: &Path, input: NewBookInput) -> Result<AddBookPreview> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;
    let resolved = resolve_book(&conn, input)?;
    let fields = resolved_book_summary(&resolved);
    let new_count = fields.iter().filter(|f| f.is_new).count();
    Ok(AddBookPreview {
        title: resolved.title.clone(),
        fields,
        new_count,
    })
}

/// Write half of the NBC app's "Ajouter un livre" flow: re-resolves `input`
/// against `db_path` (read-only), builds the insert plan, then backs up and
/// writes it in one transaction. Callers must have already checked
/// `is_book_collector_running() == false` and refused `db_path` if
/// `looks_like_real_collection` returns `Some` — this function enforces
/// neither on its own, same contract as `write_plan_sql`. Returns the
/// backup path.
pub fn commit_add_book(db_path: &Path, backup_dir: &Path, input: NewBookInput) -> Result<PathBuf> {
    let sql = {
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;
        let resolved = resolve_book(&conn, input)?;
        plan_book_sql(&conn, &resolved)?
    };
    write_plan_sql(db_path, backup_dir, &sql)
}

/// Builds the full set of DELETE statements for one book: its own join rows
/// (authors, genres, subjects, tags), credits, characters, contents and
/// their own authors, links and their `ZITEMDELEGATE` companions, the
/// book's own `ZITEMDELEGATE` wrapper row (mirrors what `plan_book_sql`
/// creates for a new book), and finally the `ZCOLLECTIBLE` row itself.
///
/// Deliberately does NOT delete shared "liste"/person values (Publisher,
/// Genre, Author, ...) even if this book was their only user — left as
/// unused-but-present entries, the same conservative choice a reference-
/// counted system makes rather than risk deleting something another row
/// still points at. Wrapped in its own `BEGIN TRANSACTION;`/`COMMIT;` like
/// `plan_book_sql`, so `write_plan_sql` can strip and execute it the same way.
pub fn plan_delete_book_sql(conn: &Connection, book_pk: i64) -> Result<String> {
    let book_ent = entity_ent_id(conn, "Book")?;
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ZCOLLECTIBLE WHERE Z_PK = ?1 AND Z_ENT = ?2",
        rusqlite::params![book_pk, book_ent],
        |row| row.get(0),
    )?;
    if exists == 0 {
        bail!("aucun livre avec Z_PK={book_pk} dans cette collection");
    }

    let item_delegate_pk: Option<i64> =
        conn.query_row("SELECT ZITEMDELEGATE FROM ZCOLLECTIBLE WHERE Z_PK = ?1", [book_pk], |row| row.get(0))?;

    let story_pks: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT Z_PK FROM ZSTORY WHERE ZBOOK = ?1")?;
        let rows = stmt.query_map([book_pk], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
        rows
    };

    let link_item_delegate_pks: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT ZITEMDELEGATE1 FROM ZLINK WHERE ZBASEBOOKCOLLECTIBLE1 = ?1 AND ZITEMDELEGATE1 IS NOT NULL",
        )?;
        let rows = stmt.query_map([book_pk], |row| row.get(0))?.collect::<rusqlite::Result<_>>()?;
        rows
    };

    let mut sql = String::new();
    sql.push_str("-- Plan de suppression — APERÇU, RIEN N'EST EXÉCUTÉ.\n");
    sql.push_str("BEGIN TRANSACTION;\n\n");

    for story_pk in &story_pks {
        sql.push_str(&format!("DELETE FROM ZAUTHOR WHERE ZSTORY = {story_pk};\n"));
    }
    sql.push_str(&format!("DELETE FROM ZSTORY WHERE ZBOOK = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM ZAUTHOR WHERE ZBOOK = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM Z_7GENRES WHERE Z_7BASEBOOKCOLLECTIBLES1 = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM Z_8SUBJECTS WHERE Z_8BOOKS1 = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM Z_6TAGS WHERE Z_6COLLECTIBLES = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM ZCREATOR WHERE ZBASEBOOKCOLLECTIBLE = {book_pk};\n"));
    sql.push_str(&format!("DELETE FROM ZCHARACTER WHERE ZBASEBOOKCOLLECTIBLE = {book_pk};\n"));
    for link_item_delegate_pk in &link_item_delegate_pks {
        sql.push_str(&format!("DELETE FROM ZITEMDELEGATE WHERE Z_PK = {link_item_delegate_pk};\n"));
    }
    sql.push_str(&format!("DELETE FROM ZLINK WHERE ZBASEBOOKCOLLECTIBLE1 = {book_pk};\n"));
    if let Some(pk) = item_delegate_pk {
        sql.push_str(&format!("DELETE FROM ZITEMDELEGATE WHERE Z_PK = {pk};\n"));
    }
    sql.push_str(&format!("DELETE FROM ZCOLLECTIBLE WHERE Z_PK = {book_pk};\n"));

    sql.push_str("\nCOMMIT;\n");
    Ok(sql)
}

/// Read-build-write in one call: opens `db_path` read-only to build the
/// delete plan, then backs up and executes it. Callers must have already
/// checked `is_book_collector_running() == false` and refused `db_path` if
/// `looks_like_real_collection` returns `Some` — same contract as
/// `commit_add_book`/`write_plan_sql`. Returns the backup path.
pub fn delete_book(db_path: &Path, backup_dir: &Path, book_pk: i64) -> Result<PathBuf> {
    let sql = {
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;
        plan_delete_book_sql(&conn, book_pk)?
    };
    write_plan_sql(db_path, backup_dir, &sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed, known mapping (both
    ///
    /// against each other) — see `role_id_for_role`'s doc comment.
    #[test]
    fn role_id_for_role_matches_the_known_book_collector_mapping() {
        assert_eq!(role_id_for_role("Translator"), Some(1));
        assert_eq!(role_id_for_role("CoverArtist"), Some(2));
        assert_eq!(role_id_for_role("CoverDesigner"), Some(2)); // synonym
        assert_eq!(role_id_for_role("Editor"), Some(3));
        assert_eq!(role_id_for_role("Illustrator"), Some(4));
        assert_eq!(role_id_for_role("Ghostwriter"), Some(5));
        assert_eq!(role_id_for_role("ForewordAuthor"), Some(6));
        assert_eq!(role_id_for_role("Photographer"), Some(7));
        assert_eq!(role_id_for_role("Narrator"), Some(8));
        assert_eq!(role_id_for_role("UserRole1"), Some(9));
        assert_eq!(role_id_for_role("UserRole2"), Some(10));
        assert_eq!(role_id_for_role("UserRole3"), Some(11));
        // Case-insensitive...
        assert_eq!(role_id_for_role("illustrator"), Some(4));
        assert_eq!(role_id_for_role("ILLUSTRATOR"), Some(4));
        // ...but an unrecognized name leaves ZROLEID NULL rather than guessing.
        assert_eq!(role_id_for_role("Author"), None);
        assert_eq!(role_id_for_role("Nonsense"), None);
        assert_eq!(role_id_for_role(""), None);
    }

    #[test]
    fn expand_tilde_rewrites_home_relative_paths_only() {
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", "/tmp/bc_export_test_home");

        assert_eq!(expand_tilde(Path::new("~")), PathBuf::from("/tmp/bc_export_test_home"));
        assert_eq!(
            expand_tilde(Path::new("~/Documents/MyCollection.bkcp")),
            PathBuf::from("/tmp/bc_export_test_home/Documents/MyCollection.bkcp")
        );
        // Absolute paths, and paths that merely contain "~" past the first
        // character, must be left alone.
        assert_eq!(
            expand_tilde(Path::new("/Volumes/Data/Book_data")),
            PathBuf::from("/Volumes/Data/Book_data")
        );
        assert_eq!(expand_tilde(Path::new("a~/b")), PathBuf::from("a~/b"));

        match original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn core_data_epoch_matches_nsdate_reference_date() {
        // 0 seconds since the Core Data/NSDate reference date is, by
        // definition, 2001-01-01T00:00:00Z — the constant this whole
        // conversion rests on.
        assert_eq!(core_data_timestamp_to_iso8601(0.0), "2001-01-01T00:00:00Z");
    }

    #[test]
    fn core_data_timestamp_before_reference_date_crosses_into_1970() {
        assert_eq!(
            core_data_timestamp_to_iso8601(-978_307_200.0),
            "1970-01-01T00:00:00Z"
        );
    }

    #[test]
    fn core_data_timestamp_matches_a_real_zdateadded_value() {
        // ZDATEADDED = 603278260 observed on a real row of the collection
        // analysed in this project; cross-checked independently in Python.
        assert_eq!(
            core_data_timestamp_to_iso8601(603_278_260.0),
            "2020-02-13T09:17:40Z"
        );
    }

    #[test]
    fn core_data_timestamp_rounds_fractional_seconds_and_rolls_the_day_over() {
        // 86_399.6s past midnight rounds to 86_400s, i.e. the next day.
        assert_eq!(
            core_data_timestamp_to_iso8601(86_399.6),
            "2001-01-02T00:00:00Z"
        );
    }

    #[test]
    fn file_url_keeps_ascii_path_segments_and_slashes_untouched() {
        assert_eq!(
            file_url("/Users/serge/Images/cover.jpg"),
            "file:///Users/serge/Images/cover.jpg"
        );
    }

    #[test]
    fn file_url_percent_encodes_spaces_parens_and_utf8_bytes() {
        // 'É' is U+00C9, encoded in UTF-8 as the two bytes 0xC3 0x89.
        assert_eq!(
            file_url("/Users/serge/My Folder/Évelyne (2).jpg"),
            "file:///Users/serge/My%20Folder/%C3%89velyne%20%282%29.jpg"
        );
    }
}
