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
use rusqlite::Connection;
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

/// Looks up the Core Data entity id for "Book" by name, rather than
/// hardcoding it — the numeric id is a model-compiler detail, not
/// guaranteed stable across collections.
pub fn book_entity_id(conn: &Connection) -> Result<i64> {
    conn.query_row(
        "SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'Book'",
        [],
        |row| row.get(0),
    )
    .context("entité Core Data 'Book' introuvable dans Z_PRIMARYKEY (schéma inattendu)")
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
