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
//!   tables (Z_<entityId><RELATIONSHIP>). These entity ids are stable
//!   across collections created by the same Book Collector version, so
//!   they are hardcoded below rather than resolved at runtime.
//! - Cover images are NOT stored as BLOBs: ZFRONTCOVER/ZBACKCOVER hold a
//!   filename that lives in the sibling "Images/" directory next to
//!   Book_data inside the .bkcp package.

use anyhow::{Context, Result};
use base64::Engine;
use book_collector_export::{
    book_entity_id, book_to_page_book, expand_tilde, load_books, resolve_collection_pk,
    resolve_db_path, resolve_images_dir, Book, PageBook,
};
use clap::Parser;
use image::ImageEncoder;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

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

    let mut html_books: Vec<PageBook> = Vec::with_capacity(books.len());
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
        html_books.push(book_to_page_book(b, thumb_url));
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
