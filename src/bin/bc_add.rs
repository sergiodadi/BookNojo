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
//! - `guard` (étape 3) : vérifie que Book Collector n'est pas lancé et
//!   produit une sauvegarde horodatée de la base. Ne modifie toujours rien
//!   dans `Book_data` lui-même (seule une copie est créée ailleurs).
//! - `commit` (étape 4) : exécute le SQL produit par `plan`, dans une seule
//!   transaction (tout ou rien), après les mêmes vérifications que `guard`
//!   (refuse si Book Collector tourne, sauvegarde d'abord). C'est la
//!   SEULE sous-commande qui écrit réellement — et elle refuse de le faire
//!   sur un chemin qui ressemble à la vraie collection
//!   (`~/Documents/Collectorz/...`) sauf `--force` explicite.
//!
//! Étape 5 : `resolve`/`plan` couvrent maintenant aussi les onglets Détails,
//! Personnel, Crédits & Personnages, Contenus, Intrigue/Notes et Liens (pas
//! Couvertures — gestion de fichiers, hors périmètre). Crédits, Personnages
//! et Contenus utilisent des tables (`ZCREATOR`, `ZCHARACTER`, `ZSTORY`) qui
//! sont VIDES dans la collection de test : leur plan SQL est construit par
//! analogie avec des motifs vérifiés ailleurs (ZAUTHOR, ZLINK — ce dernier
//! avec 832 lignes réelles), mais n'a pas pu être recoupé avec des lignes
//! réelles existantes. À vérifier avec un soin particulier après `commit`.
//!
//! Étape 6 : `commit --force` reste la seule façon d'écrire sur un chemin
//! qui ressemble à la vraie collection — mais ce n'est plus suffisant à lui
//! seul : un second garde-fou (`confirm_real_write`) exige en plus de taper
//! "OUI" de façon interactive sur stdin avant toute écriture réelle. Deux
//! barrières indépendantes (un flag ET une confirmation tapée), pour qu'une
//! vraie écriture ne puisse jamais être accidentelle ou silencieuse — y
//! compris si cette commande est un jour appelée depuis un script.

use anyhow::{bail, Context, Result};
use book_collector_export::{
    backup_sqlite_store, entity_ent_id, entity_max_pk, is_book_collector_running,
    resolve_collection_pk, resolve_db_path, resolve_lookup_pk, resolve_person_pk,
    run_write_transaction,
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
    /// Étape 3 : vérifie qu'il est sûr d'écrire (app fermée) et sauvegarde la base.
    Guard {
        /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
        #[arg(long)]
        db: PathBuf,
        /// Dossier de sauvegarde (par défaut : "backups" à côté de la base).
        #[arg(long)]
        backup_dir: Option<PathBuf>,
    },
    /// Étape 4 : exécute le plan — la SEULE sous-commande qui écrit réellement.
    Commit {
        /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
        /// Doit être une copie de test (ex. testdata-rw/) — refusé sur un chemin
        /// qui ressemble à la vraie collection, sauf --force.
        #[arg(long)]
        db: PathBuf,
        /// Fichier .sql produit par `bc_add plan`.
        #[arg(long)]
        plan: PathBuf,
        /// Dossier de sauvegarde (par défaut : "backups" à côté de la base).
        #[arg(long)]
        backup_dir: Option<PathBuf>,
        /// Autorise l'écriture même si le chemin ressemble à la vraie collection.
        #[arg(long)]
        force: bool,
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

    // ---- Onglet "Détails" ----
    #[serde(default)]
    edition: Option<String>,
    #[serde(default)]
    printed_by: Option<String>,
    #[serde(default)]
    series: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    paper_type: Option<String>,
    #[serde(default)]
    height: Option<String>,
    #[serde(default)]
    width: Option<String>,
    #[serde(default)]
    dewey: Option<String>,
    #[serde(default)]
    lccn: Option<String>,
    #[serde(default)]
    dedication: Option<String>,
    #[serde(default)]
    first_edition: Option<bool>,
    #[serde(default)]
    printing: Option<i64>,
    #[serde(default)]
    abridged: Option<bool>,
    #[serde(default)]
    audio_length_minutes: Option<i64>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    original_subtitle: Option<String>,
    #[serde(default)]
    original_publisher: Option<String>,
    #[serde(default)]
    original_country: Option<String>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    original_publication_year: Option<String>,
    #[serde(default)]
    original_publication_month: Option<String>,
    #[serde(default)]
    original_publication_day: Option<String>,

    // ---- Onglet "Personnel" ----
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    store: Option<String>,
    #[serde(default)]
    current_value: Option<f64>,
    #[serde(default)]
    purchase_price: Option<f64>,
    #[serde(default)]
    cover_price: Option<f64>,
    #[serde(default)]
    purchase_year: Option<String>,
    #[serde(default)]
    purchase_month: Option<String>,
    #[serde(default)]
    purchase_day: Option<String>,
    #[serde(default)]
    rare: Option<bool>,
    #[serde(default)]
    read_it: Option<bool>,
    #[serde(default)]
    read_times: Option<i64>,
    #[serde(default)]
    read_year: Option<String>,
    #[serde(default)]
    read_month: Option<String>,
    #[serde(default)]
    read_day: Option<String>,
    #[serde(default)]
    my_rating: Option<i64>,

    // ---- Onglet "Intrigue & Notes" ----
    #[serde(default)]
    plot: Option<String>,
    #[serde(default)]
    notes: Option<String>,

    // ---- Onglet "Crédits & Personnages" ----
    #[serde(default)]
    credits: Vec<String>,
    #[serde(default)]
    characters: Vec<CharacterInput>,

    // ---- Onglet "Contenus" ----
    #[serde(default)]
    contents: Vec<ContentInput>,

    // ---- Onglet "Liens" ----
    #[serde(default)]
    links: Vec<LinkInput>,
}

fn default_quantity() -> i64 {
    1
}

#[derive(Debug, Deserialize)]
struct CharacterInput {
    name: String,
    #[serde(default)]
    alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContentInput {
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    #[serde(default)]
    number_of_pages: Option<i64>,
    #[serde(default)]
    authors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LinkInput {
    url: String,
    #[serde(default)]
    description: Option<String>,
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

    // ---- Détails ----
    edition: Option<ResolvedLookup>,
    printed_by: Option<ResolvedLookup>,
    series: Option<ResolvedLookup>,
    volume: Option<ResolvedLookup>,
    paper_type: Option<ResolvedLookup>,
    height: Option<String>,
    width: Option<String>,
    dewey: Option<String>,
    lccn: Option<String>,
    dedication: Option<String>,
    first_edition: Option<bool>,
    printing: Option<i64>,
    abridged: Option<bool>,
    audio_length_minutes: Option<i64>,
    original_title: Option<String>,
    original_subtitle: Option<String>,
    original_publisher: Option<ResolvedLookup>,
    original_country: Option<ResolvedLookup>,
    original_language: Option<ResolvedLookup>,
    original_publication_year: Option<String>,
    original_publication_month: Option<String>,
    original_publication_day: Option<String>,

    // ---- Personnel ----
    condition: Option<ResolvedLookup>,
    owner: Option<ResolvedLookup>,
    store: Option<ResolvedLookup>,
    current_value: Option<f64>,
    purchase_price: Option<f64>,
    cover_price: Option<f64>,
    purchase_year: Option<String>,
    purchase_month: Option<String>,
    purchase_day: Option<String>,
    rare: Option<bool>,
    read_it: Option<bool>,
    read_times: Option<i64>,
    read_year: Option<String>,
    read_month: Option<String>,
    read_day: Option<String>,
    my_rating: Option<i64>,

    // ---- Intrigue & Notes ----
    plot: Option<String>,
    notes: Option<String>,

    // ---- Crédits & Personnages ----
    credits: Vec<ResolvedLookup>,
    characters: Vec<ResolvedCharacter>,

    // ---- Contenus ----
    contents: Vec<ResolvedContent>,

    // ---- Liens ----
    links: Vec<LinkInput>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResolvedCharacter {
    person: ResolvedLookup,
    alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ResolvedContent {
    title: String,
    subtitle: Option<String>,
    number_of_pages: Option<i64>,
    authors: Vec<ResolvedLookup>,
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

    let authors = resolve_person_list(&conn, "ZAUTHORPERSON", &book.authors)?;

    let publisher = book.publisher.as_deref().map(|v| resolve_lookup(&conn, "ZPUBLISHER", v)).transpose()?;
    let format = book.format.as_deref().map(|v| resolve_lookup(&conn, "ZFORMAT", v)).transpose()?;
    let language = book.language.as_deref().map(|v| resolve_lookup(&conn, "ZLANGUAGE", v)).transpose()?;
    let country = book.country.as_deref().map(|v| resolve_lookup(&conn, "ZCOUNTRY", v)).transpose()?;
    let location = book.location.as_deref().map(|v| resolve_lookup(&conn, "ZLOCATION", v)).transpose()?;
    let genres = resolve_lookup_list(&conn, "ZGENRE", &book.genres)?;
    let subjects = resolve_lookup_list(&conn, "ZSUBJECT", &book.subjects)?;

    // ---- Détails ----
    let edition = book.edition.as_deref().map(|v| resolve_lookup(&conn, "ZEDITION", v)).transpose()?;
    let printed_by = book.printed_by.as_deref().map(|v| resolve_lookup(&conn, "ZPRINTEDBY", v)).transpose()?;
    let series = book.series.as_deref().map(|v| resolve_lookup(&conn, "ZSERIES", v)).transpose()?;
    let volume = book.volume.as_deref().map(|v| resolve_lookup(&conn, "ZVOLUME", v)).transpose()?;
    let paper_type = book.paper_type.as_deref().map(|v| resolve_lookup(&conn, "ZPAPERTYPE", v)).transpose()?;
    let original_publisher = book
        .original_publisher
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZPUBLISHER", v))
        .transpose()?;
    let original_country = book
        .original_country
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZCOUNTRY", v))
        .transpose()?;
    let original_language = book
        .original_language
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZLANGUAGE", v))
        .transpose()?;

    // ---- Personnel ----
    let condition = book.condition.as_deref().map(|v| resolve_lookup(&conn, "ZCONDITION", v)).transpose()?;
    let owner = book.owner.as_deref().map(|v| resolve_person(&conn, "ZOWNER", v)).transpose()?;
    let store = book.store.as_deref().map(|v| resolve_lookup(&conn, "ZSTORE", v)).transpose()?;

    // ---- Crédits & Personnages ----
    let credits = resolve_person_list(&conn, "ZCREDITPERSON", &book.credits)?;
    let characters: Vec<ResolvedCharacter> = book
        .characters
        .iter()
        .map(|c| {
            Ok::<_, anyhow::Error>(ResolvedCharacter {
                person: resolve_person(&conn, "ZCHARACTERPERSON", &c.name)?,
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
                authors: resolve_person_list(&conn, "ZAUTHORPERSON", &c.authors)?,
            })
        })
        .collect::<Result<_>>()?;

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

    if let Some(r) = &edition {
        print_lookup("Édition", r);
    }
    if let Some(r) = &printed_by {
        print_lookup("Imprimé par", r);
    }
    if let Some(r) = &series {
        print_lookup("Série", r);
    }
    if let Some(r) = &volume {
        print_lookup("Volume", r);
    }
    if let Some(r) = &paper_type {
        print_lookup("Type de papier", r);
    }
    if let Some(r) = &original_publisher {
        print_lookup("Éditeur original", r);
    }
    if let Some(r) = &original_country {
        print_lookup("Pays original", r);
    }
    if let Some(r) = &original_language {
        print_lookup("Langue originale", r);
    }
    if let Some(r) = &condition {
        print_lookup("État", r);
    }
    if let Some(r) = &owner {
        print_lookup("Propriétaire", r);
    }
    if let Some(r) = &store {
        print_lookup("Magasin", r);
    }
    for c in &credits {
        print_lookup("Crédit", c);
    }
    for c in &characters {
        print_lookup("Personnage", &c.person);
    }
    for (i, c) in contents.iter().enumerate() {
        println!("Contenu {}: \"{}\" ({} auteur(s))", i + 1, c.title, c.authors.len());
        for a in &c.authors {
            print_lookup("  Auteur du contenu", a);
        }
    }
    for l in &book.links {
        println!("Lien: {} ({})", l.url, l.description.as_deref().unwrap_or("sans description"));
    }

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
        plot: book.plot,
        notes: book.notes,
        credits,
        characters,
        contents,
        links: book.links,
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

    // 2) Nouveaux auteurs (ZPERSONDELEGATE + ZAUTHORPERSON), avant la fiche
    //    livre elle-même puisque ZAUTHOR a besoin du Z_PK du livre.
    let author_person_pks = plan_person_list(&mut alloc, &AUTHOR_PERSON, &book.authors, &mut sql)?;

    // 3) La fiche livre elle-même (ZCOLLECTIBLE, Z_ENT="Book").
    let book_ent = entity_ent_id(&conn, "Book")?;
    let book_pk = alloc.alloc("Collectible")?; // Book partage le compteur de l'entité racine.
    sql.push('\n');
    sql.push_str(&format!(
        "INSERT INTO ZCOLLECTIBLE (\n\
         \x20 Z_PK, Z_ENT, Z_OPT, ZCOLLECTION, ZCOLLECTIONSTATUS, ZQUANTITY,\n\
         \x20 ZTITLE, ZTITLESORT, ZSUBTITLE, ZISBN,\n\
         \x20 ZPUBLISHER, ZFORMAT, ZLANGUAGE, ZCOUNTRY, ZLOCATION,\n\
         \x20 ZNUMBEROFPAGES, ZPUBLICATIONYEAR, ZPUBLICATIONMONTH, ZPUBLICATIONDAY,\n\
         \x20 ZEDITION, ZPRINTEDBY, ZSERIES, ZVOLUME, ZPAPERTYPE,\n\
         \x20 ZHEIGHT, ZWIDTH, ZDEWEY, ZLCCN, ZDEDICATION,\n\
         \x20 ZFIRSTEDITION, ZPRINTING, ZABRIDGED, ZAUDIOLENGTHMINUTES,\n\
         \x20 ZORIGINALTITLE, ZORIGINALSUBTITLE, ZORIGINALPUBLISHER, ZORIGINALCOUNTRY, ZORIGINALLANGUAGE,\n\
         \x20 ZORIGINALPUBLICATIONYEAR, ZORIGINALPUBLICATIONMONTH, ZORIGINALPUBLICATIONDAY,\n\
         \x20 ZCONDITION, ZOWNER, ZSTORE, ZCURRENTVALUE, ZPURCHASEPRICE, ZCOVERPRICE,\n\
         \x20 ZPURCHASEYEAR, ZPURCHASEMONTH, ZPURCHASEDAY,\n\
         \x20 ZRARE, ZREADIT, ZREADTIMES, ZREADYEAR, ZREADMONTH, ZREADDAY, ZMYRATING,\n\
         \x20 ZPLOT, ZNOTES\n\
         ) VALUES (\n\
         \x20 {book_pk}, {book_ent}, 1, {collection_pk}, 2, {quantity},\n\
         \x20 {title}, {title_sort}, {subtitle}, {isbn},\n\
         \x20 {publisher_fk}, {format_fk}, {language_fk}, {country_fk}, {location_fk},\n\
         \x20 {pages}, {year}, {month}, {day},\n\
         \x20 {edition_fk}, {printed_by_fk}, {series_fk}, {volume_fk}, {paper_type_fk},\n\
         \x20 {height}, {width}, {dewey}, {lccn}, {dedication},\n\
         \x20 {first_edition}, {printing}, {abridged}, {audio_length_minutes},\n\
         \x20 {original_title}, {original_subtitle}, {original_publisher_fk}, {original_country_fk}, {original_language_fk},\n\
         \x20 {original_year}, {original_month}, {original_day},\n\
         \x20 {condition_fk}, {owner_fk}, {store_fk}, {current_value}, {purchase_price}, {cover_price},\n\
         \x20 {purchase_year}, {purchase_month}, {purchase_day},\n\
         \x20 {rare}, {read_it}, {read_times}, {read_year}, {read_month}, {read_day}, {my_rating},\n\
         \x20 {plot}, {notes}\n\
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
        height = sql_opt_str(&book.height),
        width = sql_opt_str(&book.width),
        dewey = sql_opt_str(&book.dewey),
        lccn = sql_opt_str(&book.lccn),
        dedication = sql_opt_str(&book.dedication),
        first_edition = sql_opt_bool(book.first_edition),
        printing = sql_opt_i64(book.printing),
        abridged = sql_opt_bool(book.abridged),
        audio_length_minutes = sql_opt_i64(book.audio_length_minutes),
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
        plot = sql_opt_str(&book.plot),
        notes = sql_opt_str(&book.notes),
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

    // 6) Crédits : ZCREATOR, rattachés directement au livre (ZBASEBOOKCOLLECTIBLE
    //    + sa colonne jumelle Z7_BASEBOOKCOLLECTIBLE, qui porte le Z_ENT de la
    //    cible — motif confirmé empiriquement sur ZLINK, § non couvert plus bas
    //    pour ce qui reste incertain ici). Le rôle (ZROLEID) n'est pas renseigné.
    let credit_person_pks = plan_person_list(&mut alloc, &CREDIT_PERSON, &book.credits, &mut sql)?;
    let creator_ent = entity_ent_id(&conn, "Creator")?;
    for (i, person_pk) in credit_person_pks.iter().enumerate() {
        let creator_pk = alloc.alloc("Creator")?;
        sql.push_str(&format!(
            "INSERT INTO ZCREATOR (Z_PK, Z_ENT, Z_OPT, ZRANK, ZCREDITPERSON, ZBASEBOOKCOLLECTIBLE, Z7_BASEBOOKCOLLECTIBLE) \
             VALUES ({creator_pk}, {creator_ent}, 1, {rank}, {person_pk}, {book_pk}, {book_ent});\n",
            rank = i + 1
        ));
    }

    // 7) Personnages : ZCHARACTER, même motif que les crédits. ZALIAS est
    //    porté par ZCHARACTERPERSON lui-même (valeur partagée) : il n'est
    //    donc écrit que lors de la création d'un nouveau personnage, jamais
    //    en réutilisant un personnage existant.
    let character_ent = entity_ent_id(&conn, "Character")?;
    let character_person_ent = entity_ent_id(&conn, CHARACTER_PERSON.entity)?;
    for (i, c) in book.characters.iter().enumerate() {
        let person_pk = match c.person.existing_pk {
            Some(pk) => pk,
            None => {
                let delegate_ent = entity_ent_id(&conn, "PersonDelegate")?;
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
    let story_ent = entity_ent_id(&conn, "Story")?;
    for (ci, c) in book.contents.iter().enumerate() {
        let story_pk = alloc.alloc("Story")?;
        sql.push_str(&format!(
            "INSERT INTO ZSTORY (Z_PK, Z_ENT, Z_OPT, ZRANK, ZBOOK, ZNUMBEROFPAGES, ZTITLE, ZTITLESORT, ZSUBTITLE) \
             VALUES ({story_pk}, {story_ent}, 1, {rank}, {book_pk}, {pages}, {title}, {title_sort}, {subtitle});\n",
            rank = ci + 1,
            pages = sql_opt_i64(c.number_of_pages),
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
    let link_ent = entity_ent_id(&conn, "Link")?;
    let itemdelegate_ent = entity_ent_id(&conn, "ItemDelegate")?;
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

    println!();
    println!("{sql}");

    println!("--- Non couvert par ce plan (connu, documenté) ---");
    println!("- ZSEARCHSTRING : une ligne compagnon semble exister pour chaque fiche");
    println!("  (son rôle exact n'a pas encore été étudié) — absente de ce plan.");
    println!("- ZTITLESORT (livre et contenus) : copié tel quel depuis le titre (l'app");
    println!("  retire probablement les articles \"Le/La/Les/The\" — pas reproduit ici).");
    println!("- ZCLZID / ZCONNECTHASH (sync CLZ Cloud) : laissés NULL, comme un livre");
    println!("  jamais synchronisé.");
    println!("- ZINDEX, ZDATEADDED : non renseignés dans ce plan.");
    println!("- Personnes (auteurs, crédits, personnages, propriétaire) : nom entier mis");
    println!("  dans ZDISPLAYNAME/ZSORTNAME ; pas de découpage prénom/nom.");
    println!("- Crédits : ZROLEID (rôle — Illustrateur, Éditeur, Traducteur...) n'est pas");
    println!("  renseigné (laissé NULL) — la correspondance nom de rôle <-> identifiant");
    println!("  numérique interne n'a pas été établie (table de rôles non trouvée en base).");
    println!("- Crédits/Personnages/Contenus (ZCREATOR, ZCHARACTER, ZSTORY) : ces tables");
    println!("  sont VIDES dans la collection de test — ce plan suit par analogie le motif");
    println!("  vérifié ailleurs (ZAUTHOR pour les jointures, ZLINK pour le compagnon");
    println!("  ZITEMDELEGATE), mais n'a pas pu être recoupé avec des lignes réelles.");
    println!("  Contrairement à ZLINK, aucune ligne ZITEMDELEGATE compagnon n'est créée");
    println!("  pour les crédits/personnages : ZAUTHOR fonctionne sans (vérifié étape 4),");
    println!("  pris comme motif par défaut le plus sûr en l'absence de preuve contraire.");
    println!("- Liens : ZTYPEID=4 repris tel quel (100% des 832 liens existants dans la");
    println!("  collection de test utilisent cette valeur) — sens exact non confirmé.");

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

// ------------------------------------------------------------------ Guard

fn cmd_guard(db: PathBuf, backup_dir: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(&db)?;
    println!("Base: {}", db_path.display());
    println!();

    let running = is_book_collector_running()?;
    if running {
        println!("❌ Book Collector est actuellement lancé.");
        println!("   Écrire maintenant risquerait un fichier WAL incohérent, ou que l'app");
        println!("   écrase nos changements à sa prochaine sauvegarde. Fermez l'app d'abord.");
    } else {
        println!("✅ Book Collector n'est pas lancé.");
    }

    let backup_dir = backup_dir.unwrap_or_else(|| {
        db_path
            .parent()
            .map(|p| p.join("backups"))
            .unwrap_or_else(|| PathBuf::from("backups"))
    });
    let backup_path = backup_sqlite_store(&db_path, &backup_dir)
        .context("création de la sauvegarde avant écriture")?;
    let size = std::fs::metadata(&backup_path)
        .with_context(|| format!("lecture de la taille de {}", backup_path.display()))?
        .len();
    println!("✅ Sauvegarde créée: {} ({size} octets)", backup_path.display());

    println!();
    if running {
        println!("=> NE PAS écrire tant que Book Collector est ouvert.");
        println!("   Fermez l'app, puis relancez cette vérification.");
    } else {
        println!("=> Conditions réunies pour une écriture : app fermée, sauvegarde faite.");
        println!("   (`bc_add commit` refait ces deux vérifications lui-même avant d'écrire.)");
    }

    Ok(())
}

// ----------------------------------------------------------------- Commit

/// Refuses paths under `~/Documents/Collectorz/...` — the known location
/// of the user's real collection — unless `force` is set. Cheap defense in
/// depth on top of the standing rule to only ever write to a dedicated
/// test copy.
///
/// `db_path` is always canonicalized by `resolve_db_path` before reaching
/// here, so `real_prefix` must be canonicalized the same way before the
/// comparison — otherwise a symlink anywhere in `$HOME` (e.g. macOS's
/// `/var` -> `/private/var`) silently defeats the whole check: the two
/// paths would represent the same real location on disk but never compare
/// equal as plain strings. Falls back to the raw (non-canonicalized) prefix
/// if canonicalization fails (e.g. the directory doesn't exist yet) rather
/// than disabling the check entirely.
fn looks_like_real_collection(db_path: &std::path::Path) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let real_prefix = PathBuf::from(home).join("Documents").join("Collectorz");
    let canonical_prefix = real_prefix.canonicalize().unwrap_or_else(|_| real_prefix.clone());
    db_path.starts_with(&canonical_prefix).then_some(real_prefix)
}

/// Strips `bc_add plan`'s own transaction wrapper (`BEGIN TRANSACTION;`
/// ... `COMMIT;`) and comments from its SQL output, since
/// `run_write_transaction` opens its own transaction — keeps just the
/// INSERT/UPDATE statements to execute.
fn strip_plan_wrapper(raw_sql: &str) -> String {
    raw_sql
        .lines()
        .filter(|line| {
            let t = line.trim();
            !(t.is_empty() || t.starts_with("--") || t == "BEGIN TRANSACTION;" || t == "COMMIT;")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Second, independent gate on top of `--force`: even with the flag set,
/// a real write to what looks like the real collection requires typing
/// "OUI" interactively. Takes `input`/`out` rather than touching stdin/
/// stdout directly so the prompt logic itself is unit-testable without any
/// real terminal or file I/O.
fn confirm_real_write<R: std::io::BufRead, W: std::io::Write>(
    input: &mut R,
    out: &mut W,
    db_path: &std::path::Path,
    real_prefix: &std::path::Path,
) -> Result<()> {
    writeln!(
        out,
        "⚠️  ATTENTION : {} se trouve sous {} — ceci est la VRAIE collection Book Collector, pas une copie de test.",
        db_path.display(),
        real_prefix.display()
    )?;
    writeln!(out, "   Une sauvegarde horodatée sera prise avant d'écrire, mais cette opération")?;
    writeln!(out, "   modifie directement vos données réelles.")?;
    write!(out, "   Tapez exactement OUI pour continuer (autre chose annule) : ")?;
    out.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    if answer.trim() == "OUI" {
        Ok(())
    } else {
        bail!("Confirmation refusée ou incorrecte (réponse attendue : OUI, exactement) — aucune écriture effectuée.");
    }
}

fn cmd_commit(db: PathBuf, plan: PathBuf, backup_dir: Option<PathBuf>, force: bool) -> Result<()> {
    let db_path = resolve_db_path(&db)?;
    println!("Base: {}", db_path.display());

    match (force, looks_like_real_collection(&db_path)) {
        (false, Some(real_prefix)) => {
            bail!(
                "{} se trouve sous {} — ça ressemble à la vraie collection.\n\
                 Cette commande écrit réellement dans la base : utilisez une copie dédiée\n\
                 (ex. testdata-rw/, via ./refresh-testdata-rw.sh), ou passez --force si\n\
                 vous savez vraiment ce que vous faites.",
                db_path.display(),
                real_prefix.display()
            );
        }
        (true, Some(real_prefix)) => {
            let stdin = std::io::stdin();
            let mut locked = stdin.lock();
            let mut stdout = std::io::stdout();
            confirm_real_write(&mut locked, &mut stdout, &db_path, &real_prefix)?;
        }
        (_, None) => {}
    }

    if is_book_collector_running()? {
        bail!("Book Collector est actuellement lancé — fermez l'app avant d'écrire (voir `bc_add guard`).");
    }
    println!("✅ Book Collector n'est pas lancé.");

    let backup_dir = backup_dir.unwrap_or_else(|| {
        db_path
            .parent()
            .map(|p| p.join("backups"))
            .unwrap_or_else(|| PathBuf::from("backups"))
    });
    let backup_path =
        backup_sqlite_store(&db_path, &backup_dir).context("création de la sauvegarde avant écriture")?;
    println!("✅ Sauvegarde créée: {}", backup_path.display());

    let raw_sql =
        std::fs::read_to_string(&plan).with_context(|| format!("lecture de {}", plan.display()))?;
    let statements = strip_plan_wrapper(&raw_sql);
    let statement_count = statements.lines().filter(|l| !l.trim().is_empty()).count();
    println!("Exécution de {statement_count} instruction(s) dans une transaction unique...");

    run_write_transaction(&db_path, |conn| {
        conn.execute_batch(&statements).context("exécution du plan SQL")
    })?;

    println!("✅ Commit réussi — tout ou rien, comme prévu.");
    println!();
    println!("Vérification recommandée : ouvrez {} dans Book Collector", db_path.display());
    println!("et confirmez que le livre apparaît correctement (titre, auteurs, couverture...).");

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Resolve { db, input, output } => cmd_resolve(db, input, output),
        Command::Plan { db, resolved, output } => cmd_plan(db, resolved, output),
        Command::Guard { db, backup_dir } => cmd_guard(db, backup_dir),
        Command::Commit { db, plan, backup_dir, force } => cmd_commit(db, plan, backup_dir, force),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::Path;

    fn confirm(answer: &str) -> Result<()> {
        let mut input = Cursor::new(answer.as_bytes());
        let mut out = Vec::new();
        confirm_real_write(&mut input, &mut out, Path::new("/x/Documents/Collectorz/y"), Path::new("/x/Documents/Collectorz"))
    }

    #[test]
    fn confirm_real_write_accepts_only_an_exact_oui() {
        assert!(confirm("OUI\n").is_ok());
        // Trailing whitespace from the terminal is forgiven...
        assert!(confirm("OUI \n").is_ok());
        // ...but anything else — including a casual lowercase or a plain
        // Enter — must refuse. This is the step-6 safety gate: it must
        // never succeed by accident.
        assert!(confirm("oui\n").is_err());
        assert!(confirm("yes\n").is_err());
        assert!(confirm("\n").is_err());
        assert!(confirm("").is_err());
    }

    #[test]
    fn looks_like_real_collection_matches_only_under_documents_collectorz() {
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", "/Users/test");

        assert!(looks_like_real_collection(Path::new(
            "/Users/test/Documents/Collectorz/Book Collector/MyCollection.bkcp/Book_data"
        ))
        .is_some());
        assert!(looks_like_real_collection(Path::new("/Users/test/Developer/book_collector_export/testdata-rw/MyCollection.bkcp/Book_data")).is_none());

        match original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Regression test for a real bug found while testing step 6: on macOS,
    /// `/var` is a symlink to `/private/var`. `resolve_db_path` always
    /// canonicalizes (so a path under a `/var/...` $HOME arrives here as
    /// `/private/var/...`), but `looks_like_real_collection` used to build
    /// `real_prefix` from the raw $HOME without canonicalizing it — so the
    /// `starts_with` check silently failed and the whole real-collection
    /// gate (including the OUI prompt) was bypassed, for a path that *was*
    /// genuinely the real collection. Caught with a fake $HOME under
    /// std::env::temp_dir(), which on macOS is itself under `/var/...`.
    #[test]
    fn looks_like_real_collection_survives_a_symlinked_home_like_macos_var() {
        let fake_home = std::env::temp_dir().join(format!(
            "bc_add_test_home_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let collectorz_dir = fake_home.join("Documents").join("Collectorz");
        std::fs::create_dir_all(&collectorz_dir).unwrap();

        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", &fake_home);

        // The path a real `--db` run would see after resolve_db_path's own
        // canonicalize() — i.e. with any $HOME symlink already resolved.
        let canonical_db_path = collectorz_dir.canonicalize().unwrap().join("MyCollection.bkcp").join("Book_data");
        assert!(
            looks_like_real_collection(&canonical_db_path).is_some(),
            "a canonicalized path under a symlinked $HOME must still be recognized as the real collection"
        );

        std::fs::remove_dir_all(&fake_home).ok();
        match original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }
}
