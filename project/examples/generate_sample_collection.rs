//! Generates `sample-collection/MyCollection.bkcp/` — a small, entirely
//! fictional collection (2 sous-collections, 10 fiches, 4 of them with
//! every field populated) that BookNojo can safely ship and default to on first
//! launch.
//!
//! Why this exists: `templates/app.js`'s `DEV_COLLECTION_PATH` used to point
//! at `testdata-sample/`, a ~100-book *extract of the user's real
//! collection* — harmless for local development (never committed, never
//! packaged), but wrong as the app's shipped default: it meant a freshly
//! installed BookNojo opened real personal data by default. This script produces
//! a safe replacement, invented from scratch, with no connection to any real
//! collection at all.
//!
//! Run once (or again, to regenerate from scratch) with:
//!   cargo run --release --example generate_sample_collection
//!
//! How it works: Core Data's SQLite format can't be built from nothing with
//! the tooling this project has (no from-scratch schema generator, and
//! building one would mean re-deriving Book Collector's whole Core Data
//! model) — so this starts from a copy of `testdata/`'s schema, wipes every
//! data table completely (every real book/author/genre/etc. row gone), then
//! uses the same `commit_add_book` pipeline the app itself and the
//! integration tests already rely on to insert 10 invented books. The
//! result has zero connection to the original source's data, only its
//! (empty) structure.

use std::path::Path;

use book_collector_export::*;
use rusqlite::Connection;

const OUTPUT_DIR: &str = "sample-collection/MyCollection.bkcp";

fn main() -> anyhow::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = root.join(OUTPUT_DIR);

    // Images/ is deliberately NOT wiped on a rerun: the cover .jpg files in
    // there are a hand-crafted, out-of-band asset (generated once with a
    // throwaway Pillow script, not reproducible by this Rust script itself
    // — see sample_books()'s doc comment), meant to survive any number of
    // Book_data regenerations. Only Book_data/preferences.plist are reset.
    std::fs::create_dir_all(&out_dir)?;
    std::fs::create_dir_all(out_dir.join("Images"))?;
    std::fs::copy(root.join("testdata/MyCollection.bkcp/Book_data"), out_dir.join("Book_data"))?;
    std::fs::copy(
        root.join("testdata/MyCollection.bkcp/preferences.plist"),
        out_dir.join("preferences.plist"),
    )?;
    let db_path = out_dir.join("Book_data");
    let mut perms = std::fs::metadata(&db_path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&db_path, perms)?;

    wipe_and_seed_collections(&db_path)?;

    let backup_dir = out_dir.join("_setup_backups_tmp");
    for (book, cover_filename) in sample_books() {
        let title = book.title.clone();
        commit_add_book(&db_path, &backup_dir, book)?;
        // commit_add_book/NewBookInput have no cover field at all (file
        // management is explicitly out of scope for the add-book pipeline
        // — see the "Couvertures" note in the form itself), so the just-
        // inserted row's ZFRONTCOVER is set directly here instead.
        let conn = Connection::open(&db_path)?;
        conn.execute(
            "UPDATE ZCOLLECTIBLE SET ZFRONTCOVER = ?1 WHERE ZTITLE = ?2",
            rusqlite::params![cover_filename, title],
        )?;
    }
    std::fs::remove_dir_all(&backup_dir)?;
    // Shrinks the file back down after deleting every real row in
    // wipe_and_seed_collections() — SQLite doesn't reclaim that space on
    // its own (~8MB down to ~1MB for these 10 tiny rows), and this is
    // meant to be a small file committed to the repo.
    Connection::open(&db_path)?.execute_batch("VACUUM;")?;
    // Drop SQLite's WAL/SHM sidecars so the shipped fixture is just the one file.
    let _ = std::fs::remove_file(out_dir.join("Book_data-wal"));
    let _ = std::fs::remove_file(out_dir.join("Book_data-shm"));

    println!("Collection générée : {}", db_path.display());
    Ok(())
}

/// Allocates the next primary key for `entity` and keeps `Z_PRIMARYKEY` in
/// sync (same bookkeeping `plan_book_sql`'s own `PkAllocator` does for
/// every other entity) so future writes by the real app never collide with
/// what this script inserts by hand.
fn alloc_pk(conn: &Connection, entity: &str) -> anyhow::Result<i64> {
    let next = entity_max_pk(conn, entity)? + 1;
    conn.execute("UPDATE Z_PRIMARYKEY SET Z_MAX = ?1 WHERE Z_NAME = ?2", rusqlite::params![next, entity])?;
    Ok(next)
}

/// Deletes every row from every data/entity/join table — everything except
/// the three Core Data bookkeeping tables (`Z_METADATA`, `Z_MODELCACHE`,
/// `Z_PRIMARYKEY`) that define the schema itself and must survive intact for
/// the file to still be recognized as a valid Book_data store. Then creates
/// the two fresh sous-collections ("Livres", "BD") the sample books below
/// are added into.
fn wipe_and_seed_collections(db_path: &Path) -> anyhow::Result<()> {
    const TABLES_TO_WIPE: &[&str] = &[
        "ZAUTHOR", "ZAUTHORPERSON", "ZBOXSET", "ZCHARACTER", "ZCHARACTERPERSON",
        "ZCOLLECTIBLE", "ZCOLLECTION", "ZCONDITION", "ZCOUNTRY", "ZCREATOR",
        "ZCREDITPERSON", "ZEDITION", "ZEXTRA", "ZFORMAT", "ZFREQUENCY", "ZGENRE",
        "ZITEMDELEGATE", "ZLANGUAGE", "ZLINK", "ZLOAN", "ZLOANER", "ZLOCATION",
        "ZLOOKUPITEMDELEGATE", "ZOWNER", "ZPAPERTYPE", "ZPERSONDELEGATE",
        "ZPRINTEDBY", "ZPUBLISHER", "ZSEARCHSTRING", "ZSERIES", "ZSTORE",
        "ZSTORY", "ZSUBJECT", "ZTAG", "ZUSERDEFINEDFIELD", "ZUSERDEFINEDLOOKUPITEM",
        "ZUSERLOOKUP1", "ZUSERLOOKUP2", "ZVOLUME",
        "Z_36USERDEFINEDLOOKUPITEM", "Z_6TAGS", "Z_7GENRES",
        "Z_7USERDEFINEDLOOKUPITEM", "Z_8EXTRAS", "Z_8SUBJECTS",
    ];

    let mut conn = Connection::open(db_path)?;
    let tx = conn.transaction()?;
    for table in TABLES_TO_WIPE {
        tx.execute(&format!("DELETE FROM {table}"), [])?;
    }

    let collection_ent = entity_ent_id(&tx, "Collection")?;
    let itemdelegate_ent = entity_ent_id(&tx, "ItemDelegate")?;
    for (rank, name) in [(0, "Livres"), (1, "BD")] {
        let delegate_pk = alloc_pk(&tx, "ItemDelegate")?;
        let collection_pk = alloc_pk(&tx, "Collection")?;
        // Mirrors a real sous-collection's own ZITEMDELEGATE row exactly
        // (confirmed against testdata/: Z_OPT=1, ZUNIQUEID=1, the back-ref
        // in the type-specific ZCOLLECTION column, every other type column
        // left NULL).
        tx.execute(
            "INSERT INTO ZITEMDELEGATE (Z_PK, Z_ENT, Z_OPT, ZUNIQUEID, ZCOLLECTION) VALUES (?1, ?2, 1, 1, ?3)",
            rusqlite::params![delegate_pk, itemdelegate_ent, collection_pk],
        )?;
        tx.execute(
            "INSERT INTO ZCOLLECTION (Z_PK, Z_ENT, Z_OPT, ZRANK, ZITEMDELEGATE, ZTITLE) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
            rusqlite::params![collection_pk, collection_ent, rank, delegate_pk, name],
        )?;
    }

    tx.commit()?;
    Ok(())
}

/// Each book paired with its cover image's filename under `Images/` (see
/// `/tmp/generate_sample_covers.py` — a throwaway Pillow script, same
/// convention as the app icon's own generation, not committed — the
/// resulting .jpg files are what's committed). Covers are invented, not
/// generated by this script itself, since `commit_add_book`/`NewBookInput`
/// deliberately don't cover file management at all (same "Couvertures"
/// scope line the Add-book form itself shows) — set directly via SQL below.
fn sample_books() -> Vec<(NewBookInput, &'static str)> {
    vec![
        (livre_complet_1(), "echo-des-brumes.jpg"),
        (livre_complet_2(), "dernier-phare.jpg"),
        (livre_simple("Les Carnets de l'Horizon", "Noam Lacroix", 2015), "carnets-horizon.jpg"),
        (livre_simple("Une Saison à Clairval", "Noam Lacroix", 2018), "saison-clairval.jpg"),
        (livre_simple("Le Murmure des Pierres", "Yvaine Castellan", 2021), "murmure-pierres.jpg"),
        (bd_complete_1(), "capitaine-zephyr.jpg"),
        (bd_complete_2(), "chroniques-nocturna.jpg"),
        (bd_simple("Professeur Griffon et le Secret des Marées", "Hugo Delacroix", 2016), "professeur-griffon.jpg"),
        (bd_simple("Villebrume, Tome 3", "Maëlle Ostrand", 2019), "villebrume.jpg"),
        (bd_simple("Le Gang des Moineaux", "Hugo Delacroix", 2022), "gang-moineaux.jpg"),
    ]
}

/// A minimal entry — just enough fields to look like a real row in the
/// library/list views, deliberately not exercising every field (the 4
/// "complete" entries below do that).
fn livre_simple(title: &str, author: &str, year: i64) -> NewBookInput {
    NewBookInput {
        title: title.to_string(),
        collection: "Livres".to_string(),
        authors: vec![author.to_string()],
        publisher: Some("Éditions de l'Horizon Lointain".to_string()),
        format: Some("Broché".to_string()),
        language: Some("Français".to_string()),
        country: Some("France".to_string()),
        publication_year: Some(year.to_string()),
        genres: vec!["Fiction".to_string()],
        plot: Some("Fiche d'exemple — toutes les valeurs sont fictives.".to_string()),
        // NewBookInput's #[derive(Default)] gives quantity: 0 (plain
        // i64::default()) — the "1" default only applies to JSON
        // deserialization via #[serde(default = "default_quantity")], not
        // to this Rust-level Default impl, so it must be set explicitly
        // here (found by bc_export reporting an implausible total
        // quantity after the first generation run).
        quantity: 1,
        ..Default::default()
    }
}

fn bd_simple(title: &str, author: &str, year: i64) -> NewBookInput {
    NewBookInput {
        title: title.to_string(),
        collection: "BD".to_string(),
        authors: vec![author.to_string()],
        publisher: Some("Éditions du Cerf-Volant".to_string()),
        format: Some("Album".to_string()),
        language: Some("Français".to_string()),
        country: Some("France".to_string()),
        publication_year: Some(year.to_string()),
        genres: vec!["Bande dessinée".to_string()],
        plot: Some("Fiche d'exemple — toutes les valeurs sont fictives.".to_string()),
        quantity: 1,
        ..Default::default()
    }
}

/// Every field in `NewBookInput` populated with an invented value — this
/// and the 3 other "complete" entries exist specifically so BookNojo's detail
/// card, stats panel, and add/edit form all have at least a few rows that
/// exercise literally every field, right out of the box.
fn livre_complet_1() -> NewBookInput {
    NewBookInput {
        title: "L'Écho des Brumes Lointaines".to_string(),
        subtitle: Some("Un roman du cycle des Terres Grises".to_string()),
        isbn: Some("978-2-99999-001-1".to_string()),
        authors: vec!["Elara Vancoeur".to_string()],
        publisher: Some("Éditions de l'Horizon Lointain".to_string()),
        format: Some("Broché".to_string()),
        language: Some("Français".to_string()),
        country: Some("France".to_string()),
        number_of_pages: Some(384),
        publication_year: Some("2019".to_string()),
        publication_month: Some("05".to_string()),
        publication_day: Some("14".to_string()),
        genres: vec!["Fantasy".to_string(), "Aventure".to_string()],
        subjects: vec!["Quête".to_string(), "Terres imaginaires".to_string()],
        location: Some("Bibliothèque — Salon".to_string()),
        location_number: Some("EX-001".to_string()),
        quantity: 1,
        collection: "Livres".to_string(),

        issue_number: Some(1),
        edition: Some("Édition collector".to_string()),
        printed_by: Some("Imprimerie du Nord".to_string()),
        series: Some("Les Terres Grises".to_string()),
        volume: Some("1".to_string()),
        paper_type: Some("Papier bouffant".to_string()),
        height: Some("21 cm".to_string()),
        width: Some("14 cm".to_string()),
        dewey: Some("823.92".to_string()),
        lccn: Some("2019900001".to_string()),
        dedication: Some("À tous les rêveurs de brume.".to_string()),
        first_edition: Some(true),
        printing: Some(2),
        abridged: Some(false),
        box_set: Some(false),
        audio_length_minutes: Some(540),
        original_title: Some("The Echo of Distant Mists".to_string()),
        original_subtitle: Some("A Grey Lands Novel".to_string()),
        original_publisher: Some("Farhaven Press".to_string()),
        original_country: Some("Royaume-Uni".to_string()),
        original_language: Some("Anglais".to_string()),
        original_publication_year: Some("2017".to_string()),
        original_publication_month: Some("09".to_string()),
        original_publication_day: Some("02".to_string()),

        condition: Some("Comme neuf".to_string()),
        owner: Some("Collection Exemple".to_string()),
        store: Some("Librairie de l'Alcôve".to_string()),
        current_value: Some(24.90),
        purchase_price: Some(19.50),
        cover_price: Some(22.00),
        purchase_year: Some("2020".to_string()),
        purchase_month: Some("03".to_string()),
        purchase_day: Some("10".to_string()),
        rare: Some(false),
        read_it: Some(true),
        read_times: Some(2),
        read_year: Some("2021".to_string()),
        read_month: Some("07".to_string()),
        read_day: Some("20".to_string()),
        my_rating: Some(8),
        user_lookup1: Some("Coup de cœur".to_string()),
        user_lookup2: Some("Club de lecture".to_string()),
        user_text1: Some("Prêté à Camille en 2022".to_string()),
        user_text2: Some("À relire".to_string()),

        plot: Some(
            "Dans les Terres Grises, une jeune cartographe découvre une brume qui efface \
             la mémoire de quiconque la traverse. Fiche d'exemple — toutes les valeurs sont fictives."
                .to_string(),
        ),
        notes: Some("Fiche d'exemple générée automatiquement.".to_string()),

        credits: vec![
            CreditInput { name: "Tristan Ferreira".to_string() },
            CreditInput { name: "Odile Marchand".to_string() },
        ],
        characters: vec![
            CharacterInput { name: "Liora Vancoeur".to_string(), alias: Some("La Cartographe".to_string()) },
            CharacterInput { name: "Maître Corbin".to_string(), alias: None },
        ],
        contents: vec![ContentInput {
            title: "Prologue — Le Sel et la Brume".to_string(),
            subtitle: None,
            number_of_pages: Some(12),
            start_page: Some(1),
            authors: vec!["Elara Vancoeur".to_string()],
        }],
        links: vec![LinkInput {
            url: "https://example.invalid/echo-des-brumes".to_string(),
            description: Some("Page de l'éditeur (exemple)".to_string()),
        }],
    }
}

fn livre_complet_2() -> NewBookInput {
    NewBookInput {
        title: "Le Dernier Phare de Valendor".to_string(),
        subtitle: Some("Les Terres Grises, Livre II".to_string()),
        isbn: Some("978-2-99999-002-8".to_string()),
        authors: vec!["Elara Vancoeur".to_string(), "Soren Haldane".to_string()],
        publisher: Some("Éditions de l'Horizon Lointain".to_string()),
        format: Some("Relié".to_string()),
        language: Some("Français".to_string()),
        country: Some("France".to_string()),
        number_of_pages: Some(412),
        publication_year: Some("2021".to_string()),
        publication_month: Some("11".to_string()),
        publication_day: Some("03".to_string()),
        genres: vec!["Fantasy".to_string(), "Mystère".to_string()],
        subjects: vec!["Phares".to_string(), "Secrets de famille".to_string()],
        location: Some("Bureau — Étagère haute".to_string()),
        location_number: Some("EX-002".to_string()),
        quantity: 2,
        collection: "Livres".to_string(),

        issue_number: Some(2),
        edition: Some("Première édition".to_string()),
        printed_by: Some("Imprimerie du Nord".to_string()),
        series: Some("Les Terres Grises".to_string()),
        volume: Some("2".to_string()),
        paper_type: Some("Papier vélin".to_string()),
        height: Some("22 cm".to_string()),
        width: Some("15 cm".to_string()),
        dewey: Some("823.92".to_string()),
        lccn: Some("2021900002".to_string()),
        dedication: Some("Pour ceux qui gardent la lumière allumée.".to_string()),
        first_edition: Some(true),
        printing: Some(1),
        abridged: Some(false),
        box_set: Some(true),
        audio_length_minutes: Some(615),
        original_title: Some("The Last Lighthouse of Valendor".to_string()),
        original_subtitle: Some("Grey Lands Book II".to_string()),
        original_publisher: Some("Farhaven Press".to_string()),
        original_country: Some("Royaume-Uni".to_string()),
        original_language: Some("Anglais".to_string()),
        original_publication_year: Some("2020".to_string()),
        original_publication_month: Some("06".to_string()),
        original_publication_day: Some("18".to_string()),

        condition: Some("Très bon état".to_string()),
        owner: Some("Collection Exemple".to_string()),
        store: Some("Librairie de l'Alcôve".to_string()),
        current_value: Some(29.90),
        purchase_price: Some(26.00),
        cover_price: Some(26.00),
        purchase_year: Some("2021".to_string()),
        purchase_month: Some("12".to_string()),
        purchase_day: Some("24".to_string()),
        rare: Some(true),
        read_it: Some(true),
        read_times: Some(1),
        read_year: Some("2022".to_string()),
        read_month: Some("01".to_string()),
        read_day: Some("05".to_string()),
        my_rating: Some(9),
        user_lookup1: Some("Coup de cœur".to_string()),
        user_lookup2: Some("Édition limitée".to_string()),
        user_text1: Some("Dédicacé par l'auteur (exemple)".to_string()),
        user_text2: Some("Ne pas prêter".to_string()),

        plot: Some(
            "Le dernier gardien du phare de Valendor disparaît la nuit où la brume revient. \
             Fiche d'exemple — toutes les valeurs sont fictives."
                .to_string(),
        ),
        notes: Some("Fiche d'exemple générée automatiquement.".to_string()),

        credits: vec![
            CreditInput { name: "Tristan Ferreira".to_string() },
            CreditInput { name: "Camille Souriau".to_string() },
            CreditInput { name: "Odile Marchand".to_string() },
        ],
        characters: vec![
            CharacterInput { name: "Liora Vancoeur".to_string(), alias: Some("La Cartographe".to_string()) },
            CharacterInput { name: "Gardien Alric".to_string(), alias: Some("Le Dernier Phare".to_string()) },
        ],
        contents: vec![
            ContentInput {
                title: "Prologue — La Lumière qui Vacille".to_string(),
                subtitle: None,
                number_of_pages: Some(9),
                start_page: Some(1),
                authors: vec!["Elara Vancoeur".to_string()],
            },
            ContentInput {
                title: "Annexe — Carte des Terres Grises".to_string(),
                subtitle: Some("Édition annotée".to_string()),
                number_of_pages: Some(4),
                start_page: Some(405),
                authors: vec!["Soren Haldane".to_string()],
            },
        ],
        links: vec![
            LinkInput {
                url: "https://example.invalid/dernier-phare".to_string(),
                description: Some("Page de l'éditeur (exemple)".to_string()),
            },
            LinkInput { url: "https://example.invalid/dernier-phare/carte".to_string(), description: None },
        ],
    }
}

fn bd_complete_1() -> NewBookInput {
    NewBookInput {
        title: "Capitaine Zéphyr, Tome 1 : L'Éveil".to_string(),
        subtitle: Some("Série Capitaine Zéphyr".to_string()),
        isbn: Some("978-2-99999-101-5".to_string()),
        authors: vec!["Maëlle Ostrand".to_string()],
        publisher: Some("Éditions du Cerf-Volant".to_string()),
        format: Some("Album".to_string()),
        language: Some("Français".to_string()),
        country: Some("Belgique".to_string()),
        number_of_pages: Some(48),
        publication_year: Some("2018".to_string()),
        publication_month: Some("02".to_string()),
        publication_day: Some("07".to_string()),
        genres: vec!["Bande dessinée".to_string(), "Aventure".to_string()],
        subjects: vec!["Pirates".to_string(), "Ciel".to_string()],
        location: Some("Salon — Bac à BD".to_string()),
        location_number: Some("EX-101".to_string()),
        quantity: 1,
        collection: "BD".to_string(),

        issue_number: Some(1),
        edition: Some("Édition originale".to_string()),
        printed_by: Some("Imprimerie Vauclair".to_string()),
        series: Some("Capitaine Zéphyr".to_string()),
        volume: Some("1".to_string()),
        paper_type: Some("Papier couché".to_string()),
        height: Some("30 cm".to_string()),
        width: Some("23 cm".to_string()),
        dewey: Some("741.5".to_string()),
        lccn: Some("2018900101".to_string()),
        dedication: Some("Pour tous les capitaines de fortune.".to_string()),
        first_edition: Some(true),
        printing: Some(3),
        abridged: Some(false),
        box_set: Some(false),
        audio_length_minutes: Some(0),
        original_title: Some("Captain Zephyr: The Awakening".to_string()),
        original_subtitle: None,
        original_publisher: Some("Kite Comics".to_string()),
        original_country: Some("Canada".to_string()),
        original_language: Some("Anglais".to_string()),
        original_publication_year: Some("2017".to_string()),
        original_publication_month: Some("10".to_string()),
        original_publication_day: Some("12".to_string()),

        condition: Some("Bon état".to_string()),
        owner: Some("Collection Exemple".to_string()),
        store: Some("Librairie BD Expressions (exemple)".to_string()),
        current_value: Some(13.50),
        purchase_price: Some(12.90),
        cover_price: Some(12.90),
        purchase_year: Some("2018".to_string()),
        purchase_month: Some("02".to_string()),
        purchase_day: Some("09".to_string()),
        rare: Some(false),
        read_it: Some(true),
        read_times: Some(3),
        read_year: Some("2023".to_string()),
        read_month: Some("04".to_string()),
        read_day: Some("11".to_string()),
        my_rating: Some(7),
        user_lookup1: Some("Série en cours".to_string()),
        user_lookup2: Some("Humour".to_string()),
        user_text1: Some("Premier tome de la série".to_string()),
        user_text2: None,

        plot: Some(
            "Zéphyr, mousse maladroit, se retrouve propulsé capitaine d'un navire volant \
             par accident. Fiche d'exemple — toutes les valeurs sont fictives."
                .to_string(),
        ),
        notes: Some("Fiche d'exemple générée automatiquement.".to_string()),

        credits: vec![
            CreditInput { name: "Maëlle Ostrand".to_string() },
            CreditInput { name: "Denis Okonkwo".to_string() },
        ],
        characters: vec![
            CharacterInput { name: "Zéphyr Coltrane".to_string(), alias: Some("Le Mousse-Capitaine".to_string()) },
            CharacterInput { name: "Second Brask".to_string(), alias: None },
        ],
        contents: vec![],
        links: vec![LinkInput {
            url: "https://example.invalid/capitaine-zephyr".to_string(),
            description: Some("Site de la série (exemple)".to_string()),
        }],
    }
}

fn bd_complete_2() -> NewBookInput {
    NewBookInput {
        title: "Les Chroniques de Nocturna — Tome 2".to_string(),
        subtitle: Some("La Cité sous la Lune".to_string()),
        isbn: Some("978-2-99999-102-2".to_string()),
        authors: vec!["Hugo Delacroix".to_string()],
        publisher: Some("Éditions du Cerf-Volant".to_string()),
        format: Some("Album".to_string()),
        language: Some("Français".to_string()),
        country: Some("France".to_string()),
        number_of_pages: Some(56),
        publication_year: Some("2022".to_string()),
        publication_month: Some("09".to_string()),
        publication_day: Some("21".to_string()),
        genres: vec!["Bande dessinée".to_string(), "Fantastique".to_string()],
        subjects: vec!["Villes imaginaires".to_string(), "Créatures nocturnes".to_string()],
        location: Some("Salon — Bac à BD".to_string()),
        location_number: Some("EX-102".to_string()),
        quantity: 1,
        collection: "BD".to_string(),

        issue_number: Some(2),
        edition: Some("Édition originale".to_string()),
        printed_by: Some("Imprimerie Vauclair".to_string()),
        series: Some("Les Chroniques de Nocturna".to_string()),
        volume: Some("2".to_string()),
        paper_type: Some("Papier couché mat".to_string()),
        height: Some("29 cm".to_string()),
        width: Some("22 cm".to_string()),
        dewey: Some("741.5".to_string()),
        lccn: Some("2022900102".to_string()),
        dedication: None,
        first_edition: Some(false),
        printing: Some(1),
        abridged: Some(false),
        box_set: Some(false),
        audio_length_minutes: Some(0),
        original_title: None,
        original_subtitle: None,
        original_publisher: None,
        original_country: None,
        original_language: None,
        original_publication_year: None,
        original_publication_month: None,
        original_publication_day: None,

        condition: Some("Neuf".to_string()),
        owner: Some("Collection Exemple".to_string()),
        store: Some("Librairie BD Expressions (exemple)".to_string()),
        current_value: Some(15.90),
        purchase_price: Some(15.90),
        cover_price: Some(15.90),
        purchase_year: Some("2022".to_string()),
        purchase_month: Some("09".to_string()),
        purchase_day: Some("23".to_string()),
        rare: Some(false),
        read_it: Some(false),
        read_times: Some(0),
        read_year: None,
        read_month: None,
        read_day: None,
        my_rating: None,
        user_lookup1: Some("Série en cours".to_string()),
        user_lookup2: None,
        user_text1: None,
        user_text2: Some("À lire bientôt".to_string()),

        plot: Some(
            "Sous la lune de Nocturna, une archiviste découvre que la cité change de forme \
             chaque nuit. Fiche d'exemple — toutes les valeurs sont fictives."
                .to_string(),
        ),
        notes: Some("Fiche d'exemple générée automatiquement.".to_string()),

        credits: vec![CreditInput { name: "Hugo Delacroix".to_string() }],
        characters: vec![CharacterInput {
            name: "Archiviste Thessaly".to_string(),
            alias: Some("La Veilleuse".to_string()),
        }],
        contents: vec![ContentInput {
            title: "Interlude — Carnet de l'Archiviste".to_string(),
            subtitle: None,
            number_of_pages: Some(6),
            start_page: Some(28),
            authors: vec!["Hugo Delacroix".to_string()],
        }],
        links: vec![],
    }
}
