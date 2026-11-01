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
}
