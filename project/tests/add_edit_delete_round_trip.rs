//! Integration tests for the full add/edit/delete pipelines, exercised
//! against a real SQLite fixture through the same public functions the
//! `bc_add` CLI and the BookNojo app's Tauri commands call. Before this file,
//! these flows had only ever been checked manually (CLI runs against
//! `testdata-rw/`, or throwaway examples) — the unit tests in `src/lib.rs`
//! only cover small isolated helpers (path handling, date conversion),
//! never a real write against a real database.
//!
//! Each test works on its own private copy of `testdata/`'s read-only
//! `Book_data` (never the shared file itself — the project's standing
//! rule), inside a unique temp directory discarded at the end of the test.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};

use book_collector_export::*;

fn fixture_copy(test_name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/MyCollection.bkcp/Book_data");
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("bc_export_it_{test_name}_{}_{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp fixture dir");
    let dest = dir.join("Book_data");
    std::fs::copy(&source, &dest).expect("copy read-only fixture into the temp dir");
    // testdata/ is deliberately chmod 444; the copy inherits that, so it
    // must be made writable before any of these tests can write to it.
    let mut perms = std::fs::metadata(&dest).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&dest, perms).unwrap();
    dest
}

fn cleanup(db_path: &Path) {
    let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
}

fn backup_dir_for(db_path: &Path) -> PathBuf {
    db_path.parent().unwrap().join("backups")
}

fn open_ro(db_path: &Path) -> Connection {
    Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open fixture read-only")
}

fn find_by_title(db_path: &Path, title: &str) -> i64 {
    let conn = open_ro(db_path);
    conn.query_row("SELECT Z_PK FROM ZCOLLECTIBLE WHERE ZTITLE = ?1", [title], |row| row.get(0))
        .unwrap_or_else(|e| panic!("book {title:?} not found after write: {e}"))
}

fn item_delegate_of(db_path: &Path, book_pk: i64) -> i64 {
    let conn = open_ro(db_path);
    conn.query_row("SELECT ZITEMDELEGATE FROM ZCOLLECTIBLE WHERE Z_PK = ?1", [book_pk], |row| row.get(0))
        .expect("book must have its own ZITEMDELEGATE row")
}

fn integrity_check(db_path: &Path) {
    let conn = open_ro(db_path);
    let result: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(result, "ok", "PRAGMA integrity_check failed after write");
}

#[test]
fn add_book_round_trip_through_every_relation() {
    let db_path = fixture_copy("add_round_trip");
    let title = format!("ZZ_IntegrationTest_AddRoundTrip_{}", std::process::id());

    let input = NewBookInput {
        title: title.clone(),
        collection: "book-collector-serge".to_string(),
        authors: vec!["ZZ Test Author".to_string()],
        genres: vec!["ZZ Test Genre".to_string()],
        subjects: vec!["ZZ Test Subject".to_string()],
        credits: vec![CreditInput { name: "ZZ Test Translator".to_string() }],
        characters: vec![CharacterInput { name: "ZZ Test Character".to_string(), alias: Some("ZZ Alias".to_string()) }],
        contents: vec![ContentInput {
            title: "ZZ Test Story".to_string(),
            subtitle: None,
            number_of_pages: Some(12),
            start_page: Some(1),
            authors: vec!["ZZ Test Author".to_string()],
        }],
        links: vec![LinkInput { url: "https://example.invalid/zz".to_string(), description: Some("ZZ link".to_string()) }],
        ..Default::default()
    };

    let backup_path = commit_add_book(&db_path, &backup_dir_for(&db_path), input).expect("commit_add_book");
    assert!(backup_path.is_file(), "commit_add_book must leave a backup file behind");

    integrity_check(&db_path);
    let book_pk = find_by_title(&db_path, &title);

    let reloaded = load_book_for_edit_at(&db_path, book_pk).expect("load_book_for_edit_at");
    assert_eq!(reloaded.title, title);
    assert_eq!(reloaded.authors, vec!["ZZ Test Author".to_string()]);
    assert_eq!(reloaded.genres, vec!["ZZ Test Genre".to_string()]);
    assert_eq!(reloaded.subjects, vec!["ZZ Test Subject".to_string()]);
    assert_eq!(reloaded.credits.len(), 1, "credits: {:?}", reloaded.credits);
    assert_eq!(reloaded.credits[0].name, "ZZ Test Translator");
    assert_eq!(reloaded.characters.len(), 1, "characters: {:?}", reloaded.characters);
    assert_eq!(reloaded.characters[0].name, "ZZ Test Character");
    assert_eq!(reloaded.characters[0].alias.as_deref(), Some("ZZ Alias"));
    assert_eq!(reloaded.contents.len(), 1, "contents: {:?}", reloaded.contents);
    assert_eq!(reloaded.contents[0].title, "ZZ Test Story");
    assert_eq!(reloaded.contents[0].authors, vec!["ZZ Test Author".to_string()]);
    assert_eq!(reloaded.links.len(), 1, "links: {:?}", reloaded.links);
    assert_eq!(reloaded.links[0].url, "https://example.invalid/zz");

    // load_page_books is the other read path (used by bc_export's HTML
    // export and the BookNojo app's live view) — check it agrees with
    // load_book_for_edit_at rather than assuming the two stay in sync.
    let page_books = load_page_books(&db_path).expect("load_page_books");
    let page_book = page_books.iter().find(|b| b.id == book_pk).expect("book present in load_page_books");
    assert_eq!(page_book.title, title);
    assert_eq!(page_book.credits.len(), 1);
    assert_eq!(page_book.characters.len(), 1);
    assert_eq!(page_book.contents.len(), 1);
    assert_eq!(page_book.links.len(), 1);

    cleanup(&db_path);
}

#[test]
fn edit_book_updates_fields_and_swaps_relations_without_recreating_item_delegate() {
    let db_path = fixture_copy("edit_round_trip");
    let title = format!("ZZ_IntegrationTest_Edit_{}", std::process::id());

    let input = NewBookInput {
        title: title.clone(),
        collection: "book-collector-serge".to_string(),
        genres: vec!["ZZ Edit Genre Old".to_string()],
        credits: vec![CreditInput { name: "ZZ Edit Credit".to_string() }],
        ..Default::default()
    };
    commit_add_book(&db_path, &backup_dir_for(&db_path), input).expect("initial add");
    let book_pk = find_by_title(&db_path, &title);
    let item_delegate_before = item_delegate_of(&db_path, book_pk);

    let new_title = format!("{title}_Modified");
    let mut edit_input = load_book_for_edit_at(&db_path, book_pk).expect("load for edit");
    edit_input.title = new_title.clone();
    edit_input.genres = vec!["ZZ Edit Genre New".to_string()];
    edit_input.credits.push(CreditInput { name: "ZZ Edit Credit 2".to_string() });

    let backup_path =
        commit_update_book(&db_path, &backup_dir_for(&db_path), book_pk, edit_input).expect("commit_update_book");
    assert!(backup_path.is_file(), "commit_update_book must leave a backup file behind");

    integrity_check(&db_path);

    let reloaded = load_book_for_edit_at(&db_path, book_pk).expect("reload after edit");
    assert_eq!(reloaded.title, new_title);
    assert_eq!(reloaded.genres, vec!["ZZ Edit Genre New".to_string()]);
    assert_eq!(reloaded.credits.len(), 2, "credits: {:?}", reloaded.credits);

    let item_delegate_after = item_delegate_of(&db_path, book_pk);
    assert_eq!(
        item_delegate_before, item_delegate_after,
        "editing a book must keep its own ZITEMDELEGATE row, not recreate it"
    );

    // Reference-counted convention (see plan_delete_book_relations_sql's
    // doc comment): a lookup value that's no longer linked to anything
    // stays present rather than being deleted outright.
    let conn = open_ro(&db_path);
    let old_genre_still_exists = resolve_lookup_pk(&conn, "ZGENRE", "ZZ Edit Genre Old").unwrap();
    assert!(old_genre_still_exists.is_some(), "an unlinked lookup value must stay present, not be deleted");
    drop(conn);

    cleanup(&db_path);
}

#[test]
fn delete_book_removes_only_its_own_rows() {
    let db_path = fixture_copy("delete_round_trip");
    let shared_genre = format!("ZZ_Delete_SharedGenre_{}", std::process::id());
    let title_a = format!("ZZ_Delete_BookA_{}", std::process::id());
    let title_b = format!("ZZ_Delete_BookB_{}", std::process::id());

    for title in [&title_a, &title_b] {
        let input = NewBookInput {
            title: title.clone(),
            collection: "book-collector-serge".to_string(),
            genres: vec![shared_genre.clone()],
            ..Default::default()
        };
        commit_add_book(&db_path, &backup_dir_for(&db_path), input).expect("add");
    }

    let book_a_pk = find_by_title(&db_path, &title_a);
    let book_a_item_delegate = item_delegate_of(&db_path, book_a_pk);

    let backup_path = delete_book(&db_path, &backup_dir_for(&db_path), book_a_pk).expect("delete_book");
    assert!(backup_path.is_file(), "delete_book must leave a backup file behind");

    integrity_check(&db_path);

    let conn = open_ro(&db_path);
    let book_a_still_present: i64 = conn
        .query_row("SELECT COUNT(*) FROM ZCOLLECTIBLE WHERE Z_PK = ?1", [book_a_pk], |row| row.get(0))
        .unwrap();
    assert_eq!(book_a_still_present, 0, "the deleted book's own row must be gone");

    let item_delegate_still_present: i64 = conn
        .query_row("SELECT COUNT(*) FROM ZITEMDELEGATE WHERE Z_PK = ?1", [book_a_item_delegate], |row| row.get(0))
        .unwrap();
    assert_eq!(item_delegate_still_present, 0, "the deleted book's own ZITEMDELEGATE row must be gone too");

    // Book B still references the shared genre, so deleting book A must
    // not have taken it down with it.
    let shared_genre_still_exists = resolve_lookup_pk(&conn, "ZGENRE", &shared_genre).unwrap();
    assert!(shared_genre_still_exists.is_some(), "a genre still referenced by another book must not be deleted");
    drop(conn);

    let book_b_pk = find_by_title(&db_path, &title_b);
    let reloaded_b = load_book_for_edit_at(&db_path, book_b_pk).expect("book B must be untouched");
    assert_eq!(reloaded_b.genres, vec![shared_genre]);

    cleanup(&db_path);
}

#[test]
fn commit_add_book_refuses_a_readonly_database_with_a_clear_message() {
    let db_path = fixture_copy("readonly_refusal");
    // fixture_copy() makes its copy writable by default — re-lock it here
    // to reproduce testdata/'s own chmod 444, which is exactly the
    // up-front permission check in write_plan_sql this test is for.
    let mut perms = std::fs::metadata(&db_path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&db_path, perms).unwrap();

    let input = NewBookInput {
        title: "ZZ_ShouldNeverBeWritten".to_string(),
        collection: "book-collector-serge".to_string(),
        ..Default::default()
    };
    let err = commit_add_book(&db_path, &backup_dir_for(&db_path), input)
        .expect_err("writing to a readonly database must fail");
    let message = format!("{err:#}");
    assert!(message.contains("lecture seule"), "error message should explain the file is read-only: {message}");
    assert!(message.contains("testdata-rw"), "error message should point at the writable-copy convention: {message}");

    // Undo the chmod before cleanup() so remove_dir_all doesn't trip over
    // a readonly file on platforms where that matters.
    let mut perms = std::fs::metadata(&db_path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&db_path, perms).unwrap();
    cleanup(&db_path);
}
