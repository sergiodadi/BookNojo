//! bc_add — préparation de l'ajout d'un nouveau livre à Book Collector,
//! par étapes (voir MANUAL.md § 7).
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
//!
//! - `add` : enchaîne resolve+plan+commit en une seule commande contre une
//!   base, et peut répéter tout le pipeline contre une seconde base
//!   (`--also-db`, ex. la vraie collection tant qu'elle existe encore) —
//!   résolue et planifiée indépendamment pour chaque base, jamais une
//!   simple recopie du même SQL (les clés primaires et valeurs déjà
//!   connues divergent dès que deux bases évoluent séparément).
//!
//! Le cœur résolution/plan/écriture (`resolve_book`, `plan_book_sql`,
//! `write_plan_sql`, ...) vit dans `book_collector_export` (lib.rs) — partagé
//! avec l'app BookNojo, qui l'appelle directement en IPC Tauri au lieu de ce
//! détour par fichiers. Les garde-fous CLI (`--force`, "OUI" tapé au
//! clavier) restent spécifiques à ce binaire : l'app BookNojo n'écrit jamais sur
//! un chemin qui ressemble à la vraie collection, sans possibilité de forcer.

use anyhow::{bail, Context, Result};
use book_collector_export::{
    backup_sqlite_store, entity_ent_id, is_book_collector_running, looks_like_real_collection,
    plan_book_sql, resolve_book, resolve_db_path, strip_plan_wrapper, write_plan_sql, NewBookInput,
    ResolvedBook, ResolvedLookup,
};
use clap::{Parser, Subcommand};
use rusqlite::{Connection, OpenFlags};
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
    /// Langue des messages ("fr" ou "en"). Pour l'instant, seule la
    /// sous-commande `import` en tient compte — point de départ d'une
    /// internationalisation étendue progressivement au reste de l'outil.
    #[arg(long, global = true, default_value = "fr")]
    lang: String,
}

/// Minimal i18n seed: a handful of message functions, picked by `Lang`,
/// rather than a full localization framework — deliberately small so it's
/// easy to extend command-by-command instead of retrofitting ~150 existing
/// French `println!` calls in one pass. English is "en", anything else
/// falls back to French.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Fr,
    En,
}

impl Lang {
    fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("en") {
            Lang::En
        } else {
            Lang::Fr
        }
    }
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
    /// Enchaîne resolve+plan+commit en une seule commande — l'usage courant,
    /// une fois les 4 étapes validées séparément. Peut aussi répéter tout le
    /// pipeline contre une seconde base (--also-db), ex. la vraie collection
    /// Book Collector tant qu'elle existe encore.
    Add {
        /// Base principale.
        #[arg(long)]
        db: PathBuf,
        /// Fichier JSON décrivant le nouveau livre (voir MANUAL.md § 7).
        #[arg(long)]
        input: PathBuf,
        /// Seconde base à alimenter aussi, en plus de --db (ex. la vraie
        /// collection) — le livre y est résolu et planifié séparément, pas
        /// une simple recopie du plan de la base principale.
        #[arg(long)]
        also_db: Option<PathBuf>,
        /// Dossier de sauvegarde (par défaut : "backups" à côté de chaque base).
        #[arg(long)]
        backup_dir: Option<PathBuf>,
        /// Dossier où écrire les fichiers resolved/plan intermédiaires de
        /// chaque base (par défaut : bc_add_runs/<horodatage>/).
        #[arg(long)]
        runs_dir: Option<PathBuf>,
        /// Autorise l'écriture sur --db et/ou --also-db si l'un ressemble à
        /// la vraie collection — la confirmation tapée (voir `commit`)
        /// s'applique quand même à chaque base concernée.
        #[arg(long)]
        force: bool,
    },
    /// Importe une collection existante (BC, ou une base BookNojo déjà créée)
    /// vers un nouvel emplacement BookNojo indépendant — l'opération de
    /// transfert "BC vers BookNojo". Copie simple de fichiers (base + images),
    /// jamais écrasée si la destination existe déjà : ne sert qu'une fois,
    /// pour amorcer une base qui vivra ensuite sa propre vie.
    Import {
        /// Chemin vers la collection source (dossier .bkcp/.col, ou
        /// directement son fichier Book_data).
        #[arg(long)]
        source: PathBuf,
        /// Dossier de destination (nouvelle extension .col). Par défaut :
        /// "collection/<nom de la source>.col" dans le répertoire courant.
        #[arg(long)]
        dest: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------- Resolve

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

    let resolved = resolve_book(&conn, book)?;

    println!();
    println!("Titre: \"{}\"", resolved.title);
    if let Some(s) = &resolved.subtitle {
        println!("  Sous-titre: \"{s}\"");
    }
    if let Some(i) = &resolved.isbn {
        println!("  ISBN: {i}");
    }
    println!("Auteurs:");
    if resolved.authors.is_empty() {
        println!("  (aucun)");
    }
    for a in &resolved.authors {
        print_lookup("Auteur", a);
    }
    if let Some(r) = &resolved.publisher {
        print_lookup("Éditeur", r);
    }
    if let Some(r) = &resolved.format {
        print_lookup("Format", r);
    }
    if let Some(r) = &resolved.language {
        print_lookup("Langue", r);
    }
    if let Some(r) = &resolved.country {
        print_lookup("Pays", r);
    }
    if let Some(r) = &resolved.location {
        print_lookup("Emplacement", r);
    }
    for g in &resolved.genres {
        print_lookup("Genre", g);
    }
    for s in &resolved.subjects {
        print_lookup("Sujet", s);
    }
    println!(
        "Sous-collection: \"{}\" -> Z_PK={}",
        resolved.collection.value,
        resolved.collection.existing_pk.context("sous-collection non résolue")?
    );

    if let Some(r) = &resolved.edition {
        print_lookup("Édition", r);
    }
    if let Some(r) = &resolved.printed_by {
        print_lookup("Imprimé par", r);
    }
    if let Some(r) = &resolved.series {
        print_lookup("Série", r);
    }
    if let Some(r) = &resolved.volume {
        print_lookup("Volume", r);
    }
    if let Some(r) = &resolved.paper_type {
        print_lookup("Type de papier", r);
    }
    if let Some(r) = &resolved.original_publisher {
        print_lookup("Éditeur original", r);
    }
    if let Some(r) = &resolved.original_country {
        print_lookup("Pays original", r);
    }
    if let Some(r) = &resolved.original_language {
        print_lookup("Langue originale", r);
    }
    if let Some(r) = &resolved.condition {
        print_lookup("État", r);
    }
    if let Some(r) = &resolved.owner {
        print_lookup("Propriétaire", r);
    }
    if let Some(r) = &resolved.store {
        print_lookup("Magasin", r);
    }
    for c in &resolved.credits {
        print_lookup("Crédit", &c.person);
    }
    for c in &resolved.characters {
        print_lookup("Personnage", &c.person);
    }
    for (i, c) in resolved.contents.iter().enumerate() {
        println!("Contenu {}: \"{}\" ({} auteur(s))", i + 1, c.title, c.authors.len());
        for a in &c.authors {
            print_lookup("  Auteur du contenu", a);
        }
    }
    for l in &resolved.links {
        println!("Lien: {} ({})", l.url, l.description.as_deref().unwrap_or("sans description"));
    }

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

fn cmd_plan(db: PathBuf, resolved_path: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let db_path = resolve_db_path(&db)?;
    println!("Base lue (lecture seule): {}", db_path.display());

    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture en lecture seule de {}", db_path.display()))?;

    let raw = std::fs::read_to_string(&resolved_path)
        .with_context(|| format!("lecture de {}", resolved_path.display()))?;
    let book: ResolvedBook =
        serde_json::from_str(&raw).with_context(|| format!("parsing JSON de {}", resolved_path.display()))?;

    let sql = plan_book_sql(&conn, &book)?;

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
    println!("- Crédits/Personnages/Contenus (ZCREATOR, ZCHARACTER, ZSTORY) : ces tables");
    println!("  sont VIDES dans la collection de test — ce plan suit par analogie le motif");
    println!("  vérifié ailleurs (ZAUTHOR pour les jointures, ZLINK pour le compagnon");
    println!("  ZITEMDELEGATE), mais n'a pas pu être recoupé avec des lignes réelles.");
    println!("  Contrairement à ZLINK, aucune ligne ZITEMDELEGATE compagnon n'est créée");
    println!("  pour les crédits/personnages : ZAUTHOR fonctionne sans (vérifié étape 4),");
    println!("  pris comme motif par défaut le plus sûr en l'absence de preuve contraire.");
    println!("- Liens : ZTYPEID=4 repris tel quel (100% des 832 liens existants dans la");
    println!("  collection de test utilisent cette valeur) — sens exact non confirmé.");
    println!();
    println!("--- Field audit against the full ZCOLLECTIBLE/ZSTORY schema (not implemented) ---");
    println!("- ZROLEID (Credits): no corresponding label exists anywhere in Book_data itself —");
    println!("  left NULL, not resolved. A credit is just a name here, same as a character.");
    println!("- ZUSERLOOKUP1/2 (Personal tab) are now resolved as ordinary lookup fields — same");
    println!("  pattern as every other resolved lookup here, confirmed against real");
    println!("  UserLookup1/UserLookup2 entities in Z_PRIMARYKEY.");
    println!("- ZUSERTEXT1/2 (Personal tab) are now covered as plain text.");
    println!("- ZUSERDEFINEDVALUESXML: always an empty string in every row of testdata/ regardless of");
    println!("  whether ZUSERTEXT1/2 or ZUSERLOOKUP1/2 are set — left NULL/untouched, no evidence it");
    println!("  needs to be written for these fields to work correctly.");
    println!("- ZSTORY (Contenus): ZSTARTPAGE is now covered. ZSERIES, ZPLOT, ZNOTES, ZMYRATING, and");
    println!("  the whole ORIGINAL* group (title/subtitle/publisher/country/language/publication");
    println!("  date) exist on ZSTORY too, mirroring the book-level fields — but ZSTORY has 0 rows");
    println!("  in testdata/ (same as ZCREATOR/ZCHARACTER), so this is unverified by analogy only,");
    println!("  and a 10+-field addition to each content entry's form felt disproportionate without");
    println!("  real data to confirm it. Not implemented — flag if you actually use these.");
    println!("- ZITEMDELEGATE on ZCOLLECTIBLE itself: populated on every real book row, but not a");
    println!("  user-facing field in Add Book — inferred to be the same internal delegate-wrapper");
    println!("  pattern already confirmed for ZLINK's companion row, not something to write to.");
    println!("- ZLASTLOAN, ZSYNCFRONTCOVER/ZSYNCBACKCOVER, ZCLZLASTREVISION, ZLASTEDITDATE,");
    println!("  ZLASTSUBMISSIONDATE: loan-tracking/sync metadata, set by other app actions — not");
    println!("  part of Add Book, out of scope here.");

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

    let raw_sql = std::fs::read_to_string(&plan).with_context(|| format!("lecture de {}", plan.display()))?;
    let statement_count = strip_plan_wrapper(&raw_sql).lines().filter(|l| !l.trim().is_empty()).count();
    println!("Exécution de {statement_count} instruction(s) dans une transaction unique...");

    let backup_path = write_plan_sql(&db_path, &backup_dir, &raw_sql)?;
    println!("✅ Sauvegarde créée: {}", backup_path.display());
    println!("✅ Commit réussi — tout ou rien, comme prévu.");
    println!();
    println!("Vérification recommandée : ouvrez {} dans Book Collector", db_path.display());
    println!("et confirmez que le livre apparaît correctement (titre, auteurs, couverture...).");

    Ok(())
}

// --------------------------------------------------------------------- Add

/// Runs resolve → plan → commit against a single `db`, writing the two
/// intermediate files under `runs_dir` (named after `label`, so running
/// this twice for `--db`/`--also-db` never collides) — reuses the exact
/// same `cmd_resolve`/`cmd_plan`/`cmd_commit` already exercised on their
/// own, rather than duplicating any of their logic.
fn run_one_leg(
    db: &std::path::Path,
    input: &std::path::Path,
    runs_dir: &std::path::Path,
    label: &str,
    backup_dir: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let resolved_path = runs_dir.join(format!("{label}.resolved.json"));
    let plan_path = runs_dir.join(format!("{label}.plan.sql"));
    cmd_resolve(db.to_path_buf(), input.to_path_buf(), resolved_path.clone())?;
    cmd_plan(db.to_path_buf(), resolved_path, Some(plan_path.clone()))?;
    cmd_commit(db.to_path_buf(), plan_path, backup_dir, force)
}

fn cmd_add(
    db: PathBuf,
    input: PathBuf,
    also_db: Option<PathBuf>,
    backup_dir: Option<PathBuf>,
    runs_dir: Option<PathBuf>,
    force: bool,
) -> Result<()> {
    let ts_output = std::process::Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .context("obtention de l'horodatage pour le dossier de run")?;
    let timestamp = String::from_utf8_lossy(&ts_output.stdout).trim().to_string();
    let runs_dir = runs_dir.unwrap_or_else(|| PathBuf::from("bc_add_runs").join(&timestamp));
    std::fs::create_dir_all(&runs_dir)
        .with_context(|| format!("création de {}", runs_dir.display()))?;
    println!("Fichiers intermédiaires (resolved/plan) dans : {}", runs_dir.display());

    println!();
    println!("=== Base principale : {} ===", db.display());
    run_one_leg(&db, &input, &runs_dir, "principale", backup_dir.clone(), force)
        .with_context(|| format!("échec sur la base principale ({})", db.display()))?;
    println!("✅ Livre ajouté à la base principale ({}).", db.display());

    if let Some(also) = also_db {
        println!();
        println!("=== Seconde base : {} ===", also.display());
        match run_one_leg(&also, &input, &runs_dir, "aussi", backup_dir, force) {
            Ok(()) => println!("✅ Livre ajouté aussi à : {}", also.display()),
            Err(e) => {
                println!("❌ Échec sur la seconde base ({}) : {e}", also.display());
                println!("   La base principale, elle, a bien été mise à jour. Vous pouvez");
                println!("   retenter cette seconde écriture plus tard — rien n'a été perdu,");
                println!("   les fichiers resolved/plan de cette tentative restent dans");
                println!("   {} pour être rejoués avec `bc_add commit`.", runs_dir.display());
            }
        }
    }

    Ok(())
}

// --------------------------------------------------------------- Import

/// Default destination when `--dest` is omitted: `collection/<source
/// basename>.col` in the current directory — `.col` is the chosen
/// extension for the new, independent "BookNojo" database going forward.
fn default_import_dest(source: &std::path::Path) -> PathBuf {
    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "MyCollection".to_string());
    PathBuf::from("collection").join(format!("{name}.col"))
}

fn cmd_import(source: PathBuf, dest: Option<PathBuf>, lang: Lang) -> Result<()> {
    let source_db = resolve_db_path(&source)?;
    let source_pkg = source_db
        .parent()
        .context("dossier source introuvable (attendu : un .bkcp/.col contenant Book_data)")?;

    let dest = dest.unwrap_or_else(|| default_import_dest(&source));
    if dest.exists() {
        bail!(match lang {
            Lang::Fr => format!(
                "{} existe déjà — l'import ne sert qu'à amorcer une nouvelle base, une seule \
                 fois, pour ne jamais en écraser une existante. Choisissez un autre chemin \
                 avec --dest, ou supprimez-le d'abord si vous êtes sûr.",
                dest.display()
            ),
            Lang::En => format!(
                "{} already exists — import only ever bootstraps a brand-new database, once, \
                 so an existing one is never overwritten. Pick a different path with --dest, \
                 or remove it first if you're sure.",
                dest.display()
            ),
        });
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("création de {}", parent.display()))?;
    }

    match lang {
        Lang::Fr => println!("Import de {} vers {}...", source_pkg.display(), dest.display()),
        Lang::En => println!("Importing {} to {}...", source_pkg.display(), dest.display()),
    }
    let status = std::process::Command::new("cp")
        .args(["-R", &source_pkg.to_string_lossy(), &dest.to_string_lossy()])
        .status()
        .context("exécution de cp -R")?;
    if !status.success() {
        bail!(match lang {
            Lang::Fr => "échec de la copie (cp -R)".to_string(),
            Lang::En => "copy failed (cp -R)".to_string(),
        });
    }
    std::process::Command::new("chmod")
        .args(["-R", "u+w", &dest.to_string_lossy()])
        .status()
        .context("exécution de chmod -R u+w")?;

    let dest_book_data = dest.join("Book_data");
    let conn = Connection::open_with_flags(&dest_book_data, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("ouverture de {}", dest_book_data.display()))?;
    let book_ent = entity_ent_id(&conn, "Book")?;
    let book_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ZCOLLECTIBLE WHERE Z_ENT = ?1",
        [book_ent],
        |row| row.get(0),
    )?;

    println!();
    match lang {
        Lang::Fr => {
            println!("✅ Import terminé : {book_count} fiche(s) dans {}.", dest.display());
            println!("   Cette base est désormais indépendante : plus aucune synchronisation");
            println!("   automatique avec la source. Pour continuer à alimenter aussi la vraie");
            println!("   collection pendant la transition, utilisez `bc_add add --also-db`.");
        }
        Lang::En => {
            println!("✅ Import complete: {book_count} record(s) in {}.", dest.display());
            println!("   This database is now independent: no further automatic sync with");
            println!("   the source. To keep feeding the real collection too during the");
            println!("   transition, use `bc_add add --also-db`.");
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let lang = Lang::parse(&cli.lang);
    match cli.command {
        Command::Resolve { db, input, output } => cmd_resolve(db, input, output),
        Command::Plan { db, resolved, output } => cmd_plan(db, resolved, output),
        Command::Guard { db, backup_dir } => cmd_guard(db, backup_dir),
        Command::Commit { db, plan, backup_dir, force } => cmd_commit(db, plan, backup_dir, force),
        Command::Add { db, input, also_db, backup_dir, runs_dir, force } => {
            cmd_add(db, input, also_db, backup_dir, runs_dir, force)
        }
        Command::Import { source, dest } => cmd_import(source, dest, lang),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn default_import_dest_uses_the_col_extension_under_collection() {
        assert_eq!(
            default_import_dest(Path::new("/Users/x/Documents/Collectorz/Book Collector/MyCollection.bkcp")),
            PathBuf::from("collection/MyCollection.col")
        );
        assert_eq!(
            default_import_dest(Path::new("Already.col")),
            PathBuf::from("collection/Already.col")
        );
    }

    #[test]
    fn lang_parse_defaults_to_french_for_anything_but_en() {
        assert_eq!(Lang::parse("en"), Lang::En);
        assert_eq!(Lang::parse("EN"), Lang::En);
        assert_eq!(Lang::parse("fr"), Lang::Fr);
        assert_eq!(Lang::parse("anything-else"), Lang::Fr);
    }

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
