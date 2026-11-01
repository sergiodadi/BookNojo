//! Export a Collectorz "Book Collector" Core Data/SQLite collection to CSV and JSON.
//!
//! The database ("Book_data", inside a .bkcp document package) is opened
//! read-only: SQLITE_OPEN_READ_ONLY is passed explicitly, so even a bug in
//! this program cannot write to the source file.
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
//! - Authors are a join entity ZAUTHOR(ZBOOK, ZAUTHORPERSON, ZRANK) rather
//!   than a plain many-to-many table, which is why it needs its own query.
//! - Many-to-many tags/genres/subjects use Core Data's auto-named join
//!   tables (Z_<entityId><RELATIONSHIP>). Those entity ids come from the
//!   compiled .mom model shipped inside Book Collector.app, not from user
//!   data, so they are stable across collections created by the same app
//!   version and are hardcoded below rather than rediscovered at runtime.
//! - Cover images are NOT stored as BLOBs: ZFRONTCOVER/ZBACKCOVER hold a
//!   filename that lives in the sibling "Images/" directory next to
//!   Book_data inside the .bkcp package.

use anyhow::{Context, Result};
use base64::Engine;
use book_collector_export::{
    book_entity_id, expand_tilde, resolve_collection_pk, resolve_db_path, resolve_images_dir,
};
use clap::Parser;
use image::ImageEncoder;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Seconds between the Unix epoch (1970-01-01) and the Core Data / NSDate
/// reference date (2001-01-01), both UTC. Core Data TIMESTAMP columns store
/// seconds relative to the latter.
const CORE_DATA_EPOCH_OFFSET: i64 = 978_307_200;

#[derive(Parser, Debug)]
#[command(
    name = "bc_export",
    about = "Exporte une collection Book Collector (Core Data/SQLite) en CSV et JSON",
    version
)]
struct Args {
    /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
    #[arg(long)]
    db: PathBuf,

    /// Fichier CSV de sortie.
    #[arg(long, default_value = "book_collection.csv")]
    csv: PathBuf,

    /// Fichier JSON de sortie.
    #[arg(long, default_value = "book_collection.json")]
    json: PathBuf,

    /// Fichier HTML de sortie (page autonome façon Book Collector : liste +
    /// fiche détail). Omis par défaut — ne s'écrit que si précisé.
    #[arg(long)]
    html: Option<PathBuf>,

    /// Ne garder que la sous-collection nommée (table ZCOLLECTION, ex.
    /// "book-collector-serge"). Un fichier .bkcp peut contenir plusieurs
    /// sous-collections ; sans cette option, TOUTES sont exportées.
    #[arg(long)]
    collection: Option<String>,

    /// Liste les sous-collections disponibles (nom, nb de fiches, nb
    /// d'exemplaires) et s'arrête — n'écrit ni CSV ni JSON.
    #[arg(long)]
    list_collections: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Book {
    id: i64,
    title: Option<String>,
    subtitle: Option<String>,
    authors: Vec<String>,
    isbn: Option<String>,
    publisher: Option<String>,
    series: Option<String>,
    format: Option<String>,
    language: Option<String>,
    country: Option<String>,
    condition: Option<String>,
    location: Option<String>,
    edition: Option<String>,
    printed_by: Option<String>,
    volume: Option<String>,
    paper_type: Option<String>,
    owner: Option<String>,
    store: Option<String>,
    publication_day: Option<String>,
    publication_month: Option<String>,
    publication_year: Option<String>,
    number_of_pages: Option<i64>,
    my_rating: Option<i64>,
    read_it: Option<bool>,
    date_added: Option<String>,
    genres: Vec<String>,
    subjects: Vec<String>,
    tags: Vec<String>,
    notes: Option<String>,
    plot: Option<String>,
    current_value: Option<f64>,
    purchase_price: Option<f64>,
    collection: Option<String>,
    /// Nombre d'exemplaires (ZQUANTITY, 1 si absent). L'app additionne ce
    /// champ sur toute une sous-collection pour afficher son compteur —
    /// voir `sum_quantity`.
    quantity: i64,
    front_cover_file: Option<String>,
    front_cover_path: Option<String>,
    back_cover_file: Option<String>,
    back_cover_path: Option<String>,
}

/// Prints every sous-collection with its number of fiches and the sum of
/// their quantities (the number the app's own counter displays).
fn print_collections(conn: &Connection) -> Result<()> {
    let book_ent = book_entity_id(conn)?;
    let mut stmt = conn.prepare(
        "SELECT col.ZTITLE, COUNT(c.Z_PK), SUM(COALESCE(c.ZQUANTITY, 1))
         FROM ZCOLLECTION col
         LEFT JOIN ZCOLLECTIBLE c ON c.ZCOLLECTION = col.Z_PK AND c.Z_ENT = ?1
         GROUP BY col.Z_PK
         ORDER BY col.ZTITLE",
    )?;
    let mut rows = stmt.query([book_ent])?;
    println!("{:<30} {:>10} {:>12}", "collection", "fiches", "exemplaires");
    while let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        let fiches: i64 = row.get(1)?;
        let qty: i64 = row.get(2)?;
        println!("{:<30} {:>10} {:>12}", name, fiches, qty);
    }
    Ok(())
}

fn core_data_timestamp_to_iso8601(seconds: f64) -> String {
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

fn load_books(
    conn: &Connection,
    images_dir: Option<&Path>,
    collection_pk: Option<i64>,
) -> Result<Vec<Book>> {
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

fn write_csv(path: &Path, books: &[Book]) -> Result<()> {
    let mut w = csv::Writer::from_path(path)?;
    w.write_record([
        "id",
        "title",
        "subtitle",
        "authors",
        "isbn",
        "publisher",
        "series",
        "format",
        "language",
        "country",
        "condition",
        "location",
        "edition",
        "printed_by",
        "volume",
        "paper_type",
        "owner",
        "store",
        "publication_day",
        "publication_month",
        "publication_year",
        "number_of_pages",
        "my_rating",
        "read_it",
        "date_added",
        "genres",
        "subjects",
        "tags",
        "notes",
        "plot",
        "current_value",
        "purchase_price",
        "collection",
        "quantity",
        "front_cover_file",
        "front_cover_path",
        "back_cover_file",
        "back_cover_path",
    ])?;

    let join = |v: &[String]| v.join(" | ");
    let opt = |v: &Option<String>| v.clone().unwrap_or_default();

    for b in books {
        w.write_record([
            b.id.to_string(),
            opt(&b.title),
            opt(&b.subtitle),
            join(&b.authors),
            opt(&b.isbn),
            opt(&b.publisher),
            opt(&b.series),
            opt(&b.format),
            opt(&b.language),
            opt(&b.country),
            opt(&b.condition),
            opt(&b.location),
            opt(&b.edition),
            opt(&b.printed_by),
            opt(&b.volume),
            opt(&b.paper_type),
            opt(&b.owner),
            opt(&b.store),
            opt(&b.publication_day),
            opt(&b.publication_month),
            opt(&b.publication_year),
            b.number_of_pages.map(|v| v.to_string()).unwrap_or_default(),
            b.my_rating.map(|v| v.to_string()).unwrap_or_default(),
            b.read_it.map(|v| v.to_string()).unwrap_or_default(),
            opt(&b.date_added),
            join(&b.genres),
            join(&b.subjects),
            join(&b.tags),
            opt(&b.notes),
            opt(&b.plot),
            b.current_value.map(|v| v.to_string()).unwrap_or_default(),
            b.purchase_price.map(|v| v.to_string()).unwrap_or_default(),
            opt(&b.collection),
            b.quantity.to_string(),
            opt(&b.front_cover_file),
            opt(&b.front_cover_path),
            opt(&b.back_cover_file),
            opt(&b.back_cover_path),
        ])?;
    }
    w.flush()?;
    Ok(())
}

fn write_json(path: &Path, books: &[Book]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(file, books)?;
    Ok(())
}

/// Percent-encodes a filesystem path for use as a `file://` URL (spaces,
/// accented bytes, parentheses, ... — everything but unreserved chars and
/// the path separator).
fn file_url(path: &str) -> String {
    let mut out = String::from("file://");
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Decodes a cover image, downsizes it to fit within `max_dim`×`max_dim`
/// (aspect preserved), re-encodes as a modest-quality JPEG, and returns it
/// as a `data:` URI. Embedding small thumbnails directly in the page avoids
/// shipping full-resolution covers (often several hundred KB each) just to
/// render a 32–120px list/grid thumbnail, and sidesteps the macOS
/// Documents-folder access prompt for the common case of just browsing the
/// list. Returns `None` on any decode/encode failure — the caller falls
/// back to a placeholder box rather than failing the whole export.
fn make_thumbnail_data_uri(path: &Path, max_dim: u32) -> Option<String> {
    let img = image::open(path).ok()?;
    let thumb = img.resize(max_dim, max_dim, image::imageops::FilterType::Triangle);
    let rgb = thumb.to_rgb8();
    let mut buf = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 70);
    encoder
        .write_image(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Some(format!("data:image/jpeg;base64,{b64}"))
}

/// Minimal subset of Book serialized for the HTML page's embedded JSON —
/// cover images as ready-to-use `file://` URLs instead of raw filesystem
/// paths, since the browser needs a URL, not a path. `collection` is kept
/// per-book (rather than splitting into separate datasets) so the page's
/// own JS can switch between sous-collections without re-running the tool.
#[derive(Serialize)]
struct HtmlBook<'a> {
    id: i64,
    title: &'a str,
    subtitle: &'a str,
    authors: &'a [String],
    isbn: &'a str,
    publisher: &'a str,
    format: &'a str,
    language: &'a str,
    country: &'a str,
    condition: &'a str,
    location: &'a str,
    publication_year: &'a str,
    number_of_pages: Option<i64>,
    my_rating: i64,
    genres: &'a [String],
    subjects: &'a [String],
    tags: &'a [String],
    notes: &'a str,
    plot: &'a str,
    current_value: Option<f64>,
    purchase_price: Option<f64>,
    collection: &'a str,
    quantity: i64,
    /// Full-resolution cover, referenced by `file://` — used by the detail
    /// card, where image quality matters.
    cover_url: Option<String>,
    /// Small embedded JPEG (data URI) — used by list/grid row thumbnails.
    thumb_url: Option<String>,
}

const EMPTY: &str = "";
const SANS_COLLECTION: &str = "(sans sous-collection)";

/// `books` should be the UNFILTERED set (every sous-collection) so the
/// generated page's tab bar has something to switch between — pass the
/// result of `load_books(..., None)` regardless of any `--collection` CLI
/// filter used for the CSV/JSON outputs.
///
/// Layout mirrors the real app's 3-pane window: a left sidebar groups books
/// by a chosen field (Author by default, like the app's own index), the
/// top-right pane lists the books in the selected group, and the
/// bottom-right pane shows the dark "fiche détail" card for the selected
/// book.
fn write_html(path: &Path, books: &[Book]) -> Result<()> {
    const THUMB_MAX_DIM: u32 = 160;
    let with_cover = books.iter().filter(|b| b.front_cover_path.is_some()).count();
    println!("Génération de {with_cover} miniatures (max {THUMB_MAX_DIM}px)…");

    let mut html_books: Vec<HtmlBook> = Vec::with_capacity(books.len());
    let mut thumbnailed = 0usize;
    for b in books {
        let thumb_url = b
            .front_cover_path
            .as_deref()
            .and_then(|p| make_thumbnail_data_uri(Path::new(p), THUMB_MAX_DIM));
        if thumb_url.is_some() {
            thumbnailed += 1;
            if thumbnailed % 200 == 0 {
                println!("  … {thumbnailed}/{with_cover}");
            }
        }

        html_books.push(HtmlBook {
            id: b.id,
            title: b.title.as_deref().unwrap_or(EMPTY),
            subtitle: b.subtitle.as_deref().unwrap_or(EMPTY),
            authors: &b.authors,
            isbn: b.isbn.as_deref().unwrap_or(EMPTY),
            publisher: b.publisher.as_deref().unwrap_or(EMPTY),
            format: b.format.as_deref().unwrap_or(EMPTY),
            language: b.language.as_deref().unwrap_or(EMPTY),
            country: b.country.as_deref().unwrap_or(EMPTY),
            condition: b.condition.as_deref().unwrap_or(EMPTY),
            location: b.location.as_deref().unwrap_or(EMPTY),
            publication_year: b.publication_year.as_deref().unwrap_or(EMPTY),
            number_of_pages: b.number_of_pages,
            my_rating: b.my_rating.unwrap_or(0),
            genres: &b.genres,
            subjects: &b.subjects,
            tags: &b.tags,
            notes: b.notes.as_deref().unwrap_or(EMPTY),
            plot: b.plot.as_deref().unwrap_or(EMPTY),
            current_value: b.current_value,
            purchase_price: b.purchase_price,
            collection: b.collection.as_deref().unwrap_or(SANS_COLLECTION),
            quantity: b.quantity,
            cover_url: b.front_cover_path.as_deref().map(file_url),
            thumb_url,
        });
    }
    println!("Miniatures générées: {thumbnailed}/{with_cover}");

    let data_json = serde_json::to_string(&html_books)?;

    let page_template = include_str!("../templates/page.html");
    let css = include_str!("../templates/style.css");
    let js = include_str!("../templates/app.js");

    // Order matters: substitute the fixed, compile-time-known pieces
    // first, and the data payload last, so a book field that happened to
    // contain a placeholder-looking string (e.g. literally "__SCRIPT__")
    // could never be mistaken for a template token by a later pass.
    let html = page_template
        .replace("__STYLE__", css)
        .replace("__SCRIPT__", js)
        .replace("__VERSION__", env!("CARGO_PKG_VERSION"))
        .replace("__DATA_JSON__", &data_json);

    std::fs::write(path, html)?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let db_path = resolve_db_path(&args.db)?;
    let images_dir = resolve_images_dir(&db_path);

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;

    if args.list_collections {
        println!("Base lue (lecture seule): {}", db_path.display());
        print_collections(&conn)?;
        return Ok(());
    }

    let collection_pk = match &args.collection {
        Some(name) => Some(resolve_collection_pk(&conn, name)?),
        None => None,
    };

    let books = load_books(&conn, images_dir.as_deref(), collection_pk)?;

    let csv_path = expand_tilde(&args.csv);
    let json_path = expand_tilde(&args.json);
    write_csv(&csv_path, &books)?;
    write_json(&json_path, &books)?;

    let html_path = args.html.as_ref().map(|p| expand_tilde(p));
    if let Some(path) = &html_path {
        // The page always offers every sous-collection via its tab bar, so
        // it needs the unfiltered set — independent of --collection, which
        // only scopes the CSV/JSON outputs.
        let html_books = if collection_pk.is_some() {
            load_books(&conn, images_dir.as_deref(), None)?
        } else {
            books.clone()
        };
        write_html(path, &html_books)?;
    }

    let with_front_cover = books.iter().filter(|b| b.front_cover_file.is_some()).count();
    let total_quantity: i64 = books.iter().map(|b| b.quantity).sum();

    println!("Base lue (lecture seule): {}", db_path.display());
    if let Some(dir) = &images_dir {
        println!("Dossier des couvertures: {}", dir.display());
    } else {
        println!("Dossier des couvertures: introuvable (pas d'Images/ à côté de Book_data)");
    }
    match &args.collection {
        Some(name) => println!("Sous-collection: {name}"),
        None => println!("Sous-collection: (toutes)"),
    }
    println!("Fiches exportées: {}", books.len());
    println!("  dont avec couverture avant: {}", with_front_cover);
    println!("Somme des quantités (= compteur affiché par l'app pour cette sous-collection): {total_quantity}");
    println!("CSV:  {}", csv_path.display());
    println!("JSON: {}", json_path.display());
    if let Some(path) = &html_path {
        println!("HTML: {}", path.display());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
