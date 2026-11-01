//! bc_add — préparation d'un nouveau livre pour Book Collector (ÉTAPE 1
//! d'un plan en plusieurs étapes : voir MANUAL.md § 9).
//!
//! Ce binaire n'écrit JAMAIS dans la base. Il ouvre `Book_data` en lecture
//! seule (`SQLITE_OPEN_READ_ONLY`), comme `bc_export`, et se contente de
//! résoudre les champs "liste" d'un livre à ajouter (éditeur, genres,
//! auteurs, ...) contre les tables flyweight existantes
//! (ZLOOKUPITEMDELEGATE / ZPERSONDELEGATE), pour déterminer ce qui peut
//! être réutilisé tel quel et ce qui serait une nouvelle valeur.
//!
//! Étapes suivantes (pas encore implémentées) : calcul et affichage du SQL
//! d'insertion sans l'exécuter ("dry-run"), puis — seulement après des
//! garde-fous explicites (sauvegarde automatique, app fermée, transaction
//! unique) — l'écriture réelle sur une copie inscriptible dédiée.

use anyhow::{Context, Result};
use book_collector_export::{
    resolve_collection_pk, resolve_db_path, resolve_lookup_pk, resolve_person_pk,
};
use clap::Parser;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bc_add",
    about = "Prépare (sans jamais écrire) l'ajout d'un livre à une collection Book Collector",
    version
)]
struct Args {
    /// Chemin vers le fichier Book_data, ou vers le dossier .bkcp qui le contient.
    #[arg(long)]
    db: PathBuf,

    /// Fichier JSON décrivant le nouveau livre (voir MANUAL.md § 9 pour le format).
    #[arg(long)]
    input: PathBuf,

    /// Fichier JSON de sortie, avec chaque champ "liste" résolu.
    #[arg(long, default_value = "resolved-book.json")]
    output: PathBuf,
}

/// Les champs correspondent à l'onglet "Main" de la fenêtre "Add Book" de
/// Book Collector — seuls ceux dont on connaît déjà la colonne/relation en
/// base (voir le tableau envoyé dans la conversation) sont repris ici.
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

#[derive(Debug, Serialize)]
struct ResolvedLookup {
    value: String,
    /// `Some(pk)` = une ligne existante sera réutilisée (clé primaire de
    /// l'entité "liste", ex. ZPUBLISHER.Z_PK — pas celle du delegate).
    /// `None` = cette valeur n'existe pas encore et serait créée.
    existing_pk: Option<i64>,
}

#[derive(Debug, Serialize)]
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

fn main() -> Result<()> {
    let args = Args::parse();

    let db_path = resolve_db_path(&args.db)?;
    println!("Base lue (lecture seule): {}", db_path.display());

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;

    let raw = std::fs::read_to_string(&args.input)
        .with_context(|| format!("lecture de {}", args.input.display()))?;
    let input: NewBookInput = serde_json::from_str(&raw)
        .with_context(|| format!("parsing JSON de {}", args.input.display()))?;

    // La sous-collection DOIT déjà exister — contrairement aux autres
    // champs, on ne sait pas en créer une nouvelle ici (et ce n'est pas le
    // but : on ajoute un livre à une collection existante).
    let collection_pk = resolve_collection_pk(&conn, &input.collection)
        .context("sous-collection cible")?;

    let authors: Vec<ResolvedLookup> = input
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

    let publisher = input
        .publisher
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZPUBLISHER", v))
        .transpose()?;
    let format = input
        .format
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZFORMAT", v))
        .transpose()?;
    let language = input
        .language
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZLANGUAGE", v))
        .transpose()?;
    let country = input
        .country
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZCOUNTRY", v))
        .transpose()?;
    let location = input
        .location
        .as_deref()
        .map(|v| resolve_lookup(&conn, "ZLOCATION", v))
        .transpose()?;
    let genres = resolve_lookup_list(&conn, "ZGENRE", &input.genres)?;
    let subjects = resolve_lookup_list(&conn, "ZSUBJECT", &input.subjects)?;

    println!();
    println!("Titre: \"{}\"", input.title);
    if let Some(s) = &input.subtitle {
        println!("  Sous-titre: \"{s}\"");
    }
    if let Some(i) = &input.isbn {
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
    println!(
        "Sous-collection: \"{}\" -> Z_PK={collection_pk}",
        input.collection
    );

    let resolved = ResolvedBook {
        title: input.title,
        subtitle: input.subtitle,
        isbn: input.isbn,
        authors,
        publisher,
        format,
        language,
        country,
        number_of_pages: input.number_of_pages,
        publication_year: input.publication_year,
        publication_month: input.publication_month,
        publication_day: input.publication_day,
        genres,
        subjects,
        location,
        quantity: input.quantity,
        collection: ResolvedLookup {
            value: input.collection,
            existing_pk: Some(collection_pk),
        },
    };

    let out = serde_json::to_string_pretty(&resolved)?;
    std::fs::write(&args.output, out)
        .with_context(|| format!("écriture de {}", args.output.display()))?;

    println!();
    println!("Résultat écrit dans: {}", args.output.display());
    println!(
        "(Aucune écriture dans {} — cette commande ne modifie jamais la base.)",
        db_path.display()
    );

    Ok(())
}
