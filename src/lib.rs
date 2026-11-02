//! Shared helpers for reading a Collectorz "Book Collector" Core Data/SQLite
//! collection (`Book_data`, inside a `.bkcp` document package).
//!
//! Used by both binaries in this crate:
//! - `bc_export` (src/main.rs): read-only export to CSV/JSON/HTML.
//! - `bc_add` (src/bin/bc_add.rs): read-only preparation step for adding a
//!   new book (never writes to the database — see its own doc comment).
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

#[cfg(test)]
mod tests {
    use super::*;

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
