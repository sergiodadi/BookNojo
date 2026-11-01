//! bc_add — préparation de l'ajout d'un nouveau livre à Book Collector,
//! par étapes (voir MANUAL.md § 7). Ce binaire n'écrit JAMAIS dans la
//! base : chaque sous-commande ouvre `Book_data` en lecture seule
//! (`SQLITE_OPEN_READ_ONLY`), comme `bc_export`.
//!
//! - `resolve` (étape 1) : vérifie quels champs "liste" d'un livre
//!   existent déjà dans la collection (réutilisables) et lesquels seraient
//!   de nouvelles valeurs.
//! - `plan` (étape 2) : à partir du résultat de `resolve`, calcule et
//!   affiche le SQL d'insertion exact — nouvelles clés primaires,
//!   lignes à créer — SANS l'exécuter.
//!
//! Étapes suivantes (pas encore implémentées) : garde-fous d'écriture
//! (sauvegarde automatique, app fermée, transaction unique), puis le
//! premier commit réel sur une copie inscriptible dédiée.

use anyhow::{Context, Result};
use book_collector_export::{
    entity_ent_id, entity_max_pk, resolve_collection_pk, resolve_db_path, resolve_lookup_pk,
    resolve_person_pk,
};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bc_add",
    about = "Prépare (sans jamais écrire) l'ajout d'un livre à une collection Book Collector",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Étape 1 : résout les champs "liste" d'un nouveau livre contre la base existante.
    Resolve {
        /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
        #[arg(long)]
        db: PathBuf,
        /// Fichier JSON décrivant le nouveau livre (voir MANUAL.md § 7).
        #[arg(long)]
        input: PathBuf,
        /// Fichier JSON de sortie, avec chaque champ "liste" résolu.
        #[arg(long, default_value = "resolved-book.json")]
        output: PathBuf,
    },
    /// Étape 2 : calcule et affiche le SQL d'insertion — ne l'exécute jamais.
    Plan {
        /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
        #[arg(long)]
        db: PathBuf,
        /// Fichier produit par `bc_add resolve`.
        #[arg(long)]
        resolved: PathBuf,
        /// Fichier .sql de sortie (optionnel — toujours affiché sur stdout aussi).
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------- Resolve

/// Les champs correspondent à l'onglet "Main" de la fenêtre "Add Book" de
/// Book Collector — seuls ceux dont on connaît déjà la colonne/relation en
/// base sont repris ici.
#[derive(Debug, Deserialize)]
struct NewBookInput {
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    isbn: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    number_of_pages: Option<i64>,
    #[serde(default)]
    publication_year: Option<String>,
    #[serde(default)]
    publication_month: Option<String>,
    #[serde(default)]
    publication_day: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    subjects: Vec<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default = "default_quantity")]
    quantity: i64,
    /// Nom de la sous-collection cible (table ZCOLLECTION) — obligatoire :
    /// contrairement aux autres champs, elle DOIT déjà exister.
    collection: String,
}

fn default_quantity() -> i64 {
    1
}

#[derive(Debug, Serialize, Deserialize)]
struct ResolvedLookup {
    value: String,
    /// `Some(pk)` = une ligne existante sera réutilisée (clé primaire de
    /// l'entité "liste", ex. ZPUBLISHER.Z_PK — pas celle du delegate).
    /// `None` = cette valeur n'existe pas encore et serait créée.
    existing_pk: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResolvedBook {
    title: String,
    subtitle: Option<String>,
    isbn: Option<String>,
    authors: Vec<ResolvedLookup>,
    publisher: Option<ResolvedLookup>,
    format: Option<ResolvedLookup>,
    language: Option<ResolvedLookup>,
    country: Option<ResolvedLookup>,
    number_of_pages: Option<i64>,
    publication_year: Option<String>,
    publication_month: Option<String>,
    publication_day: Option<String>,
    genres: Vec<ResolvedLookup>,
    subjects: Vec<ResolvedLookup>,
    location: Option<ResolvedLookup>,
    quantity: i64,
    /// La sous-collection cible ; `existing_pk` est toujours `Some(...)`
    /// ici — on a déjà vérifié son existence avant de construire ce champ.
    collection: ResolvedLookup,
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

fn print_lookup(label: &str, r: &ResolvedLookup) {
    match r.existing_pk {
        Some(pk) => println!("  {label}: \"{}\" -> réutilise Z_PK={pk}", r.value),
        None => println!("  {label}: \"{}\" -> n'existe pas, sera créé", r.value),
    }
}

fn cmd_resolve(db: PathBuf, input: PathBuf, output: PathBuf) -> Result<()> {
    let db_path = resolve_db_path(&db)?;
    println!("Base lue (lecture seule): {}", db_path.display());

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;

    let raw = std::fs::read_to_string(&input).with_context(|| format!("lecture de {}", input.display()))?;
    let book: NewBookInput =
        serde_json::from_str(&raw).with_context(|| format!("parsing JSON de {}", input.display()))?;

    // La sous-collection DOIT déjà exister — contrairement aux autres
    // champs, on ne sait pas en créer une nouvelle ici.
    let collection_pk = resolve_collection_pk(&conn, &book.collection).context("sous-collection cible")?;

    let authors: Vec<ResolvedLookup> = book
        .authors
        .iter()
        .map(|name| {
            let existing_pk = resolve_person_pk(&conn, "ZAUTHORPERSON", name)
                .with_context(|| format!("résolution de l'auteur '{name}'"))?;
            Ok::<_, anyhow::Error>(ResolvedLookup {
                value: name.clone(),
                existing_pk,
            })
        })
        .collect::<Result<_>>()?;

    let publisher = book.publisher.as_deref().map(|v| resolve_lookup(&conn, "ZPUBLISHER", v)).transpose()?;
    let format = book.format.as_deref().map(|v| resolve_lookup(&conn, "ZFORMAT", v)).transpose()?;
    let language = book.language.as_deref().map(|v| resolve_lookup(&conn, "ZLANGUAGE", v)).transpose()?;
    let country = book.country.as_deref().map(|v| resolve_lookup(&conn, "ZCOUNTRY", v)).transpose()?;
    let location = book.location.as_deref().map(|v| resolve_lookup(&conn, "ZLOCATION", v)).transpose()?;
    let genres = resolve_lookup_list(&conn, "ZGENRE", &book.genres)?;
    let subjects = resolve_lookup_list(&conn, "ZSUBJECT", &book.subjects)?;

    println!();
    println!("Titre: \"{}\"", book.title);
    if let Some(s) = &book.subtitle {
        println!("  Sous-titre: \"{s}\"");
    }
    if let Some(i) = &book.isbn {
        println!("  ISBN: {i}");
    }
    println!("Auteurs:");
    if authors.is_empty() {
        println!("  (aucun)");
    }
    for a in &authors {
        print_lookup("Auteur", a);
    }
    if let Some(r) = &publisher {
        print_lookup("Éditeur", r);
    }
    if let Some(r) = &format {
        print_lookup("Format", r);
    }
    if let Some(r) = &language {
        print_lookup("Langue", r);
    }
    if let Some(r) = &country {
        print_lookup("Pays", r);
    }
    if let Some(r) = &location {
        print_lookup("Emplacement", r);
    }
    for g in &genres {
        print_lookup("Genre", g);
    }
    for s in &subjects {
        print_lookup("Sujet", s);
    }
    println!("Sous-collection: \"{}\" -> Z_PK={collection_pk}", book.collection);

    let resolved = ResolvedBook {
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
        quantity: book.quantity,
        collection: ResolvedLookup {
            value: book.collection,
            existing_pk: Some(collection_pk),
        },
    };

    let out = serde_json::to_string_pretty(&resolved)?;
    std::fs::write(&output, out).with_context(|| format!("écriture de {}", output.display()))?;

    println!();
    println!("Résultat écrit dans: {}", output.display());
    println!(
        "(Aucune écriture dans {} — cette commande ne modifie jamais la base.)",
        db_path.display()
    );

    Ok(())
}

// ------------------------------------------------------------------- Plan

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

fn cmd_plan(db: PathBuf, resolved_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(&db)?;
    println!("Base lue (lecture seule): {}", db_path.display());

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;

    let raw = std::fs::read_to_string(&resolved_path)
        .with_context(|| format!("lecture de {}", resolved_path.display()))?;
    let book: ResolvedBook =
        serde_json::from_str(&raw).with_context(|| format!("parsing JSON de {}", resolved_path.display()))?;

    let mut alloc = PkAllocator::new(&conn);
    let mut sql = String::new();
    sql.push_str("-- Plan d'insertion généré par `bc_add plan` — APERÇU, RIEN N'EST EXÉCUTÉ.\n");
    sql.push_str("BEGIN TRANSACTION;\n\n");

    // 1) Nouvelles valeurs "liste" (éditeur, format, langue, pays, emplacement).
    let publisher_fk = plan_lookup_fk(&mut alloc, &PUBLISHER, &book.publisher, &mut sql)?;
    let format_fk = plan_lookup_fk(&mut alloc, &FORMAT, &book.format, &mut sql)?;
    let language_fk = plan_lookup_fk(&mut alloc, &LANGUAGE, &book.language, &mut sql)?;
    let country_fk = plan_lookup_fk(&mut alloc, &COUNTRY, &book.country, &mut sql)?;
    let location_fk = plan_lookup_fk(&mut alloc, &LOCATION, &book.location, &mut sql)?;

    // 2) Nouveaux auteurs (ZPERSONDELEGATE + ZAUTHORPERSON), avant la fiche
    //    livre elle-même puisque ZAUTHOR a besoin du Z_PK du livre.
    let mut author_person_pks = Vec::new();
    for a in &book.authors {
        let pk = match a.existing_pk {
            Some(pk) => pk,
            None => {
                let delegate_ent = entity_ent_id(&conn, "PersonDelegate")?;
                let person_ent = entity_ent_id(&conn, "AuthorPerson")?;
                let delegate_pk = alloc.alloc("PersonDelegate")?;
                let person_pk = alloc.alloc("AuthorPerson")?;
                sql.push_str(&format!(
                    "INSERT INTO ZPERSONDELEGATE (Z_PK, Z_ENT, Z_OPT, ZDISPLAYNAME, ZSORTNAME) \
                     VALUES ({delegate_pk}, {delegate_ent}, 1, {v}, {v});\n",
                    v = sql_quote(&a.value)
                ));
                sql.push_str(&format!(
                    "INSERT INTO ZAUTHORPERSON (Z_PK, Z_ENT, Z_OPT, ZDELEGATE) VALUES ({person_pk}, {person_ent}, 1, {delegate_pk});\n"
                ));
                person_pk
            }
        };
        author_person_pks.push(pk);
    }

    // 3) La fiche livre elle-même (ZCOLLECTIBLE, Z_ENT="Book").
    let book_ent = entity_ent_id(&conn, "Book")?;
    let book_pk = alloc.alloc("Collectible")?; // Book partage le compteur de l'entité racine.
    sql.push('\n');
    sql.push_str(&format!(
        "INSERT INTO ZCOLLECTIBLE (\n\
         \x20 Z_PK, Z_ENT, Z_OPT, ZCOLLECTION, ZCOLLECTIONSTATUS, ZQUANTITY,\n\
         \x20 ZTITLE, ZTITLESORT, ZSUBTITLE, ZISBN,\n\
         \x20 ZPUBLISHER, ZFORMAT, ZLANGUAGE, ZCOUNTRY, ZLOCATION,\n\
         \x20 ZNUMBEROFPAGES, ZPUBLICATIONYEAR, ZPUBLICATIONMONTH, ZPUBLICATIONDAY\n\
         ) VALUES (\n\
         \x20 {book_pk}, {book_ent}, 1, {collection_pk}, 2, {quantity},\n\
         \x20 {title}, {title_sort}, {subtitle}, {isbn},\n\
         \x20 {publisher_fk}, {format_fk}, {language_fk}, {country_fk}, {location_fk},\n\
         \x20 {pages}, {year}, {month}, {day}\n\
         );\n",
        collection_pk = book.collection.existing_pk.context("sous-collection non résolue")?,
        quantity = book.quantity,
        title = sql_quote(&book.title),
        title_sort = sql_quote(&book.title), // simplification — voir note en bas de sortie
        subtitle = sql_opt_str(&book.subtitle),
        isbn = sql_opt_str(&book.isbn),
        pages = sql_opt_i64(book.number_of_pages),
        year = sql_opt_str(&book.publication_year),
        month = sql_opt_str(&book.publication_month),
        day = sql_opt_str(&book.publication_day),
    ));

    // 4) Auteurs : lignes de jointure ZAUTHOR(ZBOOK, ZAUTHORPERSON, ZRANK).
    let author_ent = entity_ent_id(&conn, "Author")?;
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

    // 6) Compteurs Z_PRIMARYKEY à mettre à jour pour chaque entité touchée.
    sql.push('\n');
    for (entity, new_max) in alloc.updates() {
        sql.push_str(&format!(
            "UPDATE Z_PRIMARYKEY SET Z_MAX = {new_max} WHERE Z_NAME = {};\n",
            sql_quote(&entity)
        ));
    }

    sql.push_str("\nCOMMIT;\n");

    println!();
    println!("{sql}");

    println!("--- Non couvert par ce plan (connu, documenté) ---");
    println!("- ZSEARCHSTRING : une ligne compagnon semble exister pour chaque fiche");
    println!("  (son rôle exact n'a pas encore été étudié) — absente de ce plan.");
    println!("- ZTITLESORT : copié tel quel depuis le titre (l'app retire probablement");
    println!("  les articles \"Le/La/Les/The\" — pas reproduit ici).");
    println!("- ZCLZID / ZCONNECTHASH (sync CLZ Cloud) : laissés NULL, comme un livre");
    println!("  jamais synchronisé.");
    println!("- ZINDEX, ZDATEADDED, ZMYRATING, ZREADIT : non renseignés dans ce plan.");
    println!("- Auteurs : nom entier mis dans ZDISPLAYNAME/ZSORTNAME ; pas de découpage");
    println!("  prénom/nom (ZFIRSTNAME/ZLASTNAME).");

    if let Some(path) = &output {
        std::fs::write(path, &sql).with_context(|| format!("écriture de {}", path.display()))?;
        println!();
        println!("SQL également écrit dans: {}", path.display());
    }

    println!();
    println!(
        "(Aucune écriture dans {} — ce SQL n'a pas été exécuté.)",
        db_path.display()
    );

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Resolve { db, input, output } => cmd_resolve(db, input, output),
        Command::Plan { db, resolved, output } => cmd_plan(db, resolved, output),
    }
}
