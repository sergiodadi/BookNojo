use std::path::Path;

use serde::Serialize;
use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Only collections produced by this exact Book Collector release are known
/// to match the schema this project's SQL targets.
const SUPPORTED_BKCP_VERSION: &str = "20.3.1";

/// The version shown to the user (header badge, both "About BookNojo"
/// surfaces), deliberately *not* `CARGO_PKG_VERSION` — `Cargo.toml`'s
/// version bumps on every single internal commit, while this tracks the
/// latest published `.dmg` release tag instead (what the user actually
/// downloaded). MusicNojo/VideoNojo both already have this; BookNojo never
/// did — found 15/11/2026 by directly comparing the three apps' code, not
/// from BookNojo's own MANUAL.md, which doesn't mention the gap. **Update
/// this by hand at every future release**, before building that release's
/// `.dmg`, not after.
const DISPLAY_VERSION: &str = "1.0.0b10";

/// Loads a collection (a `.bkcp`/`.col` directory, or its `Book_data` file
/// directly) read-only and returns every book, across all sous-collections,
/// as the same `PageBook` shape `bc_export`'s static HTML export already
/// uses — so `templates/app.js` renders it identically either way. Errors
/// are returned as plain strings (Tauri's IPC boundary can't carry
/// `anyhow::Error`), with the original context preserved in the message.
///
/// Also grants the asset protocol (used for `convertFileSrc`-loaded cover
/// images) access to this collection's own folder — see
/// `grant_asset_scope_for_collection` for why this needs to happen
/// dynamically rather than via a static list in `tauri.conf.json`.
#[tauri::command]
fn load_collection(app: tauri::AppHandle, path: String) -> Result<Vec<book_collector_export::PageBook>, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    grant_asset_scope_for_collection(&app, &db_path);
    book_collector_export::load_page_books(&db_path).map_err(|e| format!("{e:#}"))
}

/// Allows the asset protocol to serve files (cover images) from whichever
/// collection folder was just opened. The WebView's security sandbox blocks
/// raw `file://` loads, so cover images go through Tauri's asset protocol
/// instead (`convertFileSrc` in `templates/app.js`), which only serves paths
/// explicitly in scope. A static scope list in `tauri.conf.json` would have
/// to name every possible collection location in advance — fine on the
/// developer's own machine (where the test fixtures live at fixed paths),
/// but broken for anyone else who clones this project and opens a
/// collection from wherever theirs happens to live. Granting the scope here,
/// at the moment a collection is actually opened, works for any location
/// without needing to know it ahead of time. Best-effort: a failure here
/// only means cover images won't load, not that the collection itself
/// failed to open, so it's never surfaced as an error.
fn grant_asset_scope_for_collection<R: tauri::Runtime>(app: &tauri::AppHandle<R>, db_path: &Path) {
    if let Some(collection_dir) = db_path.parent() {
        let _ = app.asset_protocol_scope().allow_directory(collection_dir, true);
    }
}

/// The BookNojo app's HTML shell (`app/src/index.template.html`) is copied as-is
/// at dev-data-sync time, with no build step to burn a version string into
/// it the way `bc_export`'s static export does (`src/main.rs`'s
/// `__VERSION__` substitution) — `templates/app.js` asks for it over IPC
/// instead, same as everything else.
#[tauri::command]
fn app_version() -> String {
    DISPLAY_VERSION.to_string()
}

/// Refuses any write-bound operation against what looks like the real Book
/// Collector collection — unlike `bc_add`'s CLI, there is no `--force`/"OUI"
/// override reachable from the GUI at all. The standing project rule is
/// that new code never touches the real collection directly; this is where
/// that rule is enforced for the "Ajouter un livre" form.
fn refuse_if_real_collection(db_path: &Path) -> Result<(), String> {
    if book_collector_export::looks_like_real_collection(db_path).is_some() {
        return Err(
            "BookNojo n'écrit jamais directement dans la vraie collection Book Collector — \
             cette opération n'est possible qu'en ligne de commande (bc_add)."
                .to_string(),
        );
    }
    Ok(())
}

/// Native picker for a cover image, same async-callback shape as the
/// import dialog (a blocking pick from inside a `#[tauri::command]`
/// deadlocks on macOS). `set_cover` didn't exist anywhere in this project
/// before 2026-11-15 — found missing by comparing against MusicNojo's/
/// VideoNojo's own `pick_and_set_cover`, which this mirrors. Entry point is
/// the stats panel's "no cover" list (front cover only, same as the other
/// two apps — there's no equivalent one-click action for the back cover).
/// Result goes back as an event, not the command's return value, since the
/// picker itself is asynchronous.
#[tauri::command]
fn pick_and_set_cover(app: tauri::AppHandle, path: String, book_id: i64) {
    let db_path = match book_collector_export::resolve_db_path(Path::new(&path)) {
        Ok(p) => p,
        Err(e) => {
            let _ = app.emit("cover-set-error", format!("{e:#}"));
            return;
        }
    };
    if refuse_if_real_collection(&db_path).is_err() {
        let _ = app.emit(
            "cover-set-error",
            "BookNojo n'écrit jamais directement dans la vraie collection Book Collector.".to_string(),
        );
        return;
    }
    app.dialog()
        .file()
        .set_title("Choisir une image de jaquette")
        .add_filter("Image", &["jpg", "jpeg", "png", "gif", "bmp"])
        .pick_file(move |file| {
            let Some(file) = file else { return };
            let Some(local_path) = file.as_path() else { return };
            match book_collector_export::is_book_collector_running() {
                Ok(true) => {
                    let _ = app.emit(
                        "cover-set-error",
                        "Book Collector est actuellement lancé — fermez l'application avant d'enregistrer.".to_string(),
                    );
                    return;
                }
                Err(e) => {
                    let _ = app.emit("cover-set-error", format!("{e:#}"));
                    return;
                }
                Ok(false) => {}
            }
            let backup_dir = db_path
                .parent()
                .map(|p| p.join("backups"))
                .unwrap_or_else(|| std::path::PathBuf::from("backups"));
            match book_collector_export::set_cover(&db_path, &backup_dir, book_id, Some(local_path), None) {
                Ok(_) => {
                    let _ = app.emit("cover-set", book_id);
                }
                Err(e) => {
                    let _ = app.emit("cover-set-error", format!("{e:#}"));
                }
            }
        });
}

/// Native picker for a cover image to attach to a *brand-new* book, fired
/// from the Add-book form's "Choisir une couverture" button — before the
/// book exists yet, so this only picks a file and hands its path back via
/// an event; `commit_add_book` does the actual `set_cover()` write once the
/// new book's `Z_PK` is known. Same async-callback shape as
/// `pick_and_set_cover` (a blocking pick deadlocks on macOS). No error
/// event: the only way this doesn't emit is the user cancelling the picker,
/// which isn't an error.
#[tauri::command]
fn pick_cover_for_new_book(app: tauri::AppHandle) {
    app.dialog()
        .file()
        .set_title("Choisir une image de jaquette")
        .add_filter("Image", &["jpg", "jpeg", "png", "gif", "bmp"])
        .pick_file(move |file| {
            let Some(file) = file else { return };
            let Some(local_path) = file.as_path() else { return };
            let _ = app.emit("add-cover-picked", local_path.to_string_lossy().to_string());
        });
}

/// "Recherche en ligne" (étape 3 du plan) — interroge les 3 sources
/// gratuites (Open Library, Google Books, BnF) **en parallèle**, un thread
/// par source : `reqwest::blocking` est volontairement synchrone (voir
/// `project/src/online_search.rs`), donc le parallélisme vient d'ici, pas
/// d'async/tokio. Chaque source est indépendante — un échec réseau, un
/// timeout ou même un panic d'une seule source (`JoinHandle::join`
/// renvoyant `Err`) n'empêche jamais les 2 autres de répondre ; ses
/// résultats sont simplement absents. Les résultats des 3 sources sont
/// concaténés (Open Library, puis Google Books, puis BnF), chacun portant
/// déjà son propre champ `source` — pas de fusion, le frontend groupe par
/// source à l'affichage.
#[tauri::command]
fn search_book_online(query: String) -> Vec<book_collector_export::online_search::BookSearchResult> {
    let open_library = std::thread::spawn({
        let query = query.clone();
        move || book_collector_export::online_search::search_open_library(&query)
    });
    let google_books = std::thread::spawn({
        let query = query.clone();
        move || book_collector_export::online_search::search_google_books(&query)
    });
    let bnf = std::thread::spawn(move || book_collector_export::online_search::search_bnf(&query));

    let mut results = Vec::new();
    for handle in [open_library, google_books, bnf] {
        if let Ok(Ok(mut source_results)) = handle.join() {
            results.append(&mut source_results);
        }
    }
    results
}

/// Étape 5 du plan "Recherche en ligne" — thin wrapper around
/// `online_search::download_cover_to_temp_file` (see its own doc comment
/// for why the download+write logic lives in the shared crate rather than
/// here: `reqwest` is already a dependency there, not of this crate).
#[tauri::command]
fn download_online_cover(url: String) -> Result<String, String> {
    book_collector_export::online_search::download_cover_to_temp_file(&url)
}

/// Read-only half of "Ajouter un livre" (étape 1-5 du formulaire) — résout
/// les champs du nouveau livre contre la collection actuellement ouverte et
/// renvoie un résumé à faire valider par l'utilisateur avant toute écriture.
#[tauri::command]
fn prepare_add_book(
    path: String,
    input: book_collector_export::NewBookInput,
) -> Result<book_collector_export::AddBookPreview, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    refuse_if_real_collection(&db_path)?;
    book_collector_export::prepare_add_book(&db_path, input).map_err(|e| format!("{e:#}"))
}

/// Write half of "Ajouter un livre" — appelé seulement après que
/// l'utilisateur a validé l'aperçu renvoyé par `prepare_add_book`. Refuse la
/// vraie collection (voir `refuse_if_real_collection`) et refuse si Book
/// Collector est actuellement lancé, avant toute sauvegarde/écriture.
///
/// `cover_path` (optionnel) vient du bouton "Choisir une couverture" du
/// formulaire d'ajout (voir `pick_cover_for_new_book`) : une fois le livre
/// créé et son `Z_PK` connu (renvoyé par `commit_add_book` côté lib), la
/// jaquette est posée via `set_cover` — une 2e écriture indépendante, pas
/// la même transaction que l'ajout. Si elle échoue, le livre reste quand
/// même ajouté (rattrapable ensuite via la liste "sans couverture" du
/// panneau Statistiques) ; le message renvoyé le signale.
#[tauri::command]
fn commit_add_book(
    path: String,
    input: book_collector_export::NewBookInput,
    cover_path: Option<String>,
) -> Result<String, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    refuse_if_real_collection(&db_path)?;
    if book_collector_export::is_book_collector_running().map_err(|e| format!("{e:#}"))? {
        return Err("Book Collector est actuellement lancé — fermez l'application avant d'enregistrer.".to_string());
    }
    let backup_dir = db_path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("backups"));
    let (backup, book_pk) = book_collector_export::commit_add_book(&db_path, &backup_dir, input)
        .map_err(|e| format!("{e:#}"))?;
    let Some(cover_path) = cover_path else {
        return Ok(format!("Livre ajouté. Sauvegarde : {}", backup.display()));
    };
    match book_collector_export::set_cover(&db_path, &backup_dir, book_pk, Some(Path::new(&cover_path)), None) {
        Ok(_) => Ok(format!("Livre ajouté avec sa couverture. Sauvegarde : {}", backup.display())),
        Err(e) => Ok(format!(
            "Livre ajouté, mais la couverture n'a pas pu être enregistrée ({e:#}). Sauvegarde : {}",
            backup.display()
        )),
    }
}

/// Supprime un livre (et uniquement ses propres lignes — crédits,
/// personnages, contenus, liens, jointures genres/sujets/tags — jamais les
/// valeurs partagées comme l'éditeur ou un genre). Mêmes garde-fous que
/// `commit_add_book` : jamais la vraie collection, jamais si Book Collector
/// est lancé. La confirmation ("es-tu sûr ?") est côté interface
/// (`window.confirm`), pas ici — cette commande exécute dès qu'elle est
/// appelée.
#[tauri::command]
fn delete_book(path: String, book_id: i64) -> Result<String, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    refuse_if_real_collection(&db_path)?;
    if book_collector_export::is_book_collector_running().map_err(|e| format!("{e:#}"))? {
        return Err("Book Collector est actuellement lancé — fermez l'application avant de supprimer.".to_string());
    }
    let backup_dir = db_path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("backups"));
    book_collector_export::delete_book(&db_path, &backup_dir, book_id)
        .map(|backup| format!("Livre supprimé. Sauvegarde : {}", backup.display()))
        .map_err(|e| format!("{e:#}"))
}

/// Charge les valeurs actuelles d'un livre pour pré-remplir le formulaire
/// "Ajouter un livre" en mode édition — lecture seule.
#[tauri::command]
fn load_book_for_edit(path: String, book_id: i64) -> Result<book_collector_export::NewBookInput, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    book_collector_export::load_book_for_edit_at(&db_path, book_id).map_err(|e| format!("{e:#}"))
}

/// Write half of "Modifier ce livre" — appelé seulement après que
/// l'utilisateur a validé l'aperçu renvoyé par `prepare_add_book` (la même
/// commande que pour l'ajout : la résolution des champs ne dépend pas de
/// la fiche existante). Mêmes garde-fous que `commit_add_book`.
#[tauri::command]
fn commit_update_book(path: String, book_id: i64, input: book_collector_export::NewBookInput) -> Result<String, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    refuse_if_real_collection(&db_path)?;
    if book_collector_export::is_book_collector_running().map_err(|e| format!("{e:#}"))? {
        return Err("Book Collector est actuellement lancé — fermez l'application avant d'enregistrer.".to_string());
    }
    let backup_dir = db_path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("backups"));
    book_collector_export::commit_update_book(&db_path, &backup_dir, book_id, input)
        .map(|backup| format!("Livre modifié. Sauvegarde : {}", backup.display()))
        .map_err(|e| format!("{e:#}"))
}

/// Reads `kPreferencesApplicationVersion` out of a `.bkcp`'s
/// `preferences.plist` (binary plist — shelled out to `plutil`, since this
/// is macOS-only code anyway and avoids a new plist-parsing dependency for
/// a single field).
fn bkcp_version(bkcp_dir: &Path) -> Option<String> {
    let output = std::process::Command::new("plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(bkcp_dir.join("preferences.plist"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("kPreferencesApplicationVersion")?.as_str().map(str::to_string)
}

/// Header button "Open Collection…" (only entry point left as of this
/// step — the native File menu now has "Import Collection…" instead, see
/// `open_import_collection_dialog`). Opens the native macOS file picker. A
/// `.bkcp`/`.col` is a directory on disk, but macOS/LaunchServices registers
/// `.bkcp` (via the app that created it) as a package — NSOpenPanel treats a
/// recognized package as a *file*, not a folder, so a folder-only picker
/// (`canChooseFiles: false`) grays it out entirely. Using `pick_file` with
/// an extension filter instead lets the package be selected as a single
/// atomic item, matching how Finder/Open dialogs normally handle package-
/// style bundles. A picked `.bkcp` is additionally checked against
/// `SUPPORTED_BKCP_VERSION` before being accepted, since this project's SQL
/// targets that one release's schema and other versions may use a
/// different schema. The chosen path is sent to the frontend as an
/// `import-collection` event rather than returned directly, since the
/// picker itself is asynchronous/callback-based. (The event name stays
/// `import-collection` — it's an internal implementation detail, not shown
/// to the user; the user-visible label is "Open Collection…" everywhere.)
///
/// **Still accepts a `.bkcp` directly for now** (opened in place, exactly
/// like today) — restricting this picker to `.col` only, now that a real
/// "Import Collection…" exists to bootstrap one, is step 4 of this plan,
/// not yet done.
fn open_import_dialog(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.dialog()
        .file()
        .set_title("Open a Book Collector collection (.bkcp)")
        .add_filter("Collection", &["bkcp", "col"])
        .pick_file(move |file| {
            let Some(path) = file else { return };
            let Some(local_path) = path.as_path() else {
                let _ = app_handle.emit("import-collection", path.to_string());
                return;
            };

            let is_bkcp = local_path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("bkcp"));

            if is_bkcp {
                match bkcp_version(local_path) {
                    Some(version) if version == SUPPORTED_BKCP_VERSION => {}
                    Some(version) => {
                        app_handle
                            .dialog()
                            .message(format!(
                                "Cette collection a été créée avec Book Collector {version}. \
                                 Seule la version {SUPPORTED_BKCP_VERSION} est prise en charge \
                                 pour l'import — import annulé."
                            ))
                            .title("Version non prise en charge")
                            .kind(MessageDialogKind::Error)
                            .show(|_| {});
                        return;
                    }
                    None => {
                        app_handle
                            .dialog()
                            .message(
                                "Impossible de déterminer la version de Book Collector utilisée \
                                 pour cette collection — import annulé par précaution.",
                            )
                            .title("Version non vérifiable")
                            .kind(MessageDialogKind::Error)
                            .show(|_| {});
                        return;
                    }
                }
            }

            let _ = app_handle.emit("import-collection", path.to_string());
        });
}

/// IPC wrapper so the header button's "Open Collection…" can call the same
/// `open_import_dialog` the function above documents (a dialog callback
/// can't be invoked directly from the frontend, only a `#[tauri::command]`).
#[tauri::command]
fn pick_collection(app: tauri::AppHandle) {
    open_import_dialog(&app);
}

/// Payload for the `collection-imported` event — a brand-new `.col` was
/// just bootstrapped from a `.bkcp` (see `open_import_collection_dialog`).
/// `book_count` lets the frontend show an import summary without a second
/// round trip to count anything itself.
#[derive(Serialize, Clone)]
struct CollectionImported {
    path: String,
    book_count: i64,
}

/// Small one-button error dialog, factored out because
/// `open_import_collection_dialog` below has three distinct failure points
/// (unsupported/unverifiable Book Collector version, destination already
/// existing, the copy itself failing) that would otherwise repeat the same
/// `app.dialog()....show(|_| {})` boilerplate three times.
fn show_error_dialog(app: &tauri::AppHandle, title: &str, message: impl Into<String>) {
    app.dialog()
        .message(message.into())
        .title(title)
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

/// "File > Import Collection…" — the native macOS menu's own entry point
/// (deliberately *not* in the app's own header/menu: imports are rare,
/// a native-menu-only place suits them, per Serge). Distinct from "Open
/// Collection…" (the header button, `open_import_dialog`): Import only
/// ever takes a real Book Collector `.bkcp` as source — the picker is
/// filtered accordingly (no `.col` here; re-importing an already-BookNojo
/// database makes no sense) — checks its Book Collector version exactly
/// like the combined picker used to, then bootstraps a brand-new,
/// independent `.col` at the fixed location `default_col_destination`
/// computes (`~/Documents/BookNojo/<name>.col`, étape 2 of this plan) via
/// the shared `import_collection_to` (étape 1). The source `.bkcp` is only
/// ever read — this is a one-shot copy, never a live link (see
/// `import_collection_to`'s own doc comment).
///
/// On success, emits `collection-imported` (new path + book count) rather
/// than reusing `import-collection` — the frontend needs to react
/// differently here (show an import summary and *then* open the new
/// collection, not just silently reload); wiring that frontend listener is
/// étape 5 of this plan, not yet done.
fn open_import_collection_dialog(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.dialog()
        .file()
        .set_title("Import a Book Collector collection (.bkcp)")
        .add_filter("Book Collector collection", &["bkcp"])
        .pick_file(move |file| {
            let Some(path) = file else { return };
            let Some(source) = path.as_path() else { return };

            match bkcp_version(source) {
                Some(version) if version == SUPPORTED_BKCP_VERSION => {}
                Some(version) => {
                    show_error_dialog(
                        &app_handle,
                        "Version non prise en charge",
                        format!(
                            "Cette collection a été créée avec Book Collector {version}. \
                             Seule la version {SUPPORTED_BKCP_VERSION} est prise en charge \
                             pour l'import — import annulé."
                        ),
                    );
                    return;
                }
                None => {
                    show_error_dialog(
                        &app_handle,
                        "Version non vérifiable",
                        "Impossible de déterminer la version de Book Collector utilisée \
                         pour cette collection — import annulé par précaution.",
                    );
                    return;
                }
            }

            let dest = match book_collector_export::default_col_destination(source) {
                Ok(dest) => dest,
                Err(e) => {
                    show_error_dialog(&app_handle, "Import impossible", format!("{e:#}"));
                    return;
                }
            };

            match book_collector_export::import_collection_to(source, &dest) {
                Ok(outcome) => {
                    let _ = app_handle.emit(
                        "collection-imported",
                        CollectionImported {
                            path: outcome.dest.to_string_lossy().to_string(),
                            book_count: outcome.book_count,
                        },
                    );
                }
                Err(e) => {
                    show_error_dialog(&app_handle, "Import impossible", format!("{e:#}"));
                }
            }
        });
}

/// "BookNojo Help > User Guide" — opens a real help window (a bundled,
/// bilingual, self-contained `help.html`: no Tauri API calls from inside
/// it, so no capability grant needed) instead of the plain name+version
/// dialog this used to show, which only duplicated the real native About
/// panel (the app_menu's own `.about(...)`, a separate surface already
/// covering name+version). Reuses the existing window if already open
/// instead of opening a second one.
fn show_help_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("help") {
        let _ = win.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(app, "help", tauri::WebviewUrl::App("help.html".into()))
        .title("BookNojo Help")
        .inner_size(680.0, 820.0)
        .resizable(true)
        .build();
}

fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "BookNojo")
        .about(Some(tauri::menu::AboutMetadata {
            // Overrides the bundle's own CFBundleShortVersionString/
            // CFBundleVersion (still CARGO_PKG_VERSION/Cargo.toml's
            // per-commit dev version) with DISPLAY_VERSION — the native
            // About panel is a separate surface from the custom dialog
            // below, driven by the bundle's own Info.plist fields rather
            // than any Rust command, so both need fixing.
            version: Some(DISPLAY_VERSION.to_string()),
            short_version: Some(DISPLAY_VERSION.to_string()),
            ..Default::default()
        }))
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .text("import_collection", "Import Collection…")
        .separator()
        .close_window()
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .separator()
        .close_window()
        .build()?;

    // A submenu titled exactly "Help" is swallowed entirely by macOS/Tauri
    // in this Tauri version (confirmed by testing: renaming it made it
    // reappear; matches known upstream Tauri issues on this exact
    // behavior) — "BookNojo Help" sidesteps it while staying unambiguous.
    // "User Guide" used to be "About BookNojo", showing a bare
    // name+version dialog that just duplicated the real native About
    // panel (built from the app_menu's own `.about(...)` above) — found
    // by Serge comparing the two. Now opens a real help window instead
    // (see show_help_window).
    let help_menu = SubmenuBuilder::new(app, "BookNojo Help")
        .text("show_help", "User Guide")
        .build()?;

    let mut builder = MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    {
        builder = builder.item(&app_menu);
    }
    builder
        .item(&file_menu)
        .item(&edit_menu)
        .item(&window_menu)
        .item(&help_menu)
        .build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_collection,
            prepare_add_book,
            commit_add_book,
            delete_book,
            load_book_for_edit,
            commit_update_book,
            pick_and_set_cover,
            pick_cover_for_new_book,
            pick_collection,
            search_book_online,
            download_online_cover,
            app_version
        ])
        .setup(|app| {
            let menu = build_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "import_collection" => open_import_collection_dialog(app),
            "show_help" => show_help_window(app),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_asset_scope_for_collection_allows_only_the_opened_collections_own_folder() {
        let app = tauri::test::mock_app();
        let collection_dir = std::env::temp_dir().join("booknojo_asset_scope_test_collection");
        let db_path = collection_dir.join("Book_data");
        let unrelated_dir = std::env::temp_dir().join("booknojo_asset_scope_test_unrelated");
        let unrelated_file = unrelated_dir.join("Images/cover.jpg");

        let scope = app.asset_protocol_scope();
        assert!(
            !scope.is_allowed(&unrelated_file),
            "scope must deny everything by default, before any collection is opened"
        );

        grant_asset_scope_for_collection(app.handle(), &db_path);

        assert!(
            scope.is_allowed(collection_dir.join("Images/cover.jpg")),
            "opening a collection must allow loading cover images from its own folder, \
             wherever on disk that folder happens to be"
        );
        assert!(
            !scope.is_allowed(&unrelated_file),
            "granting access to one collection's folder must not widen access to unrelated folders"
        );
    }

    /// Real network call, not a mock — same spirit as `online_search`'s own
    /// live tests. Only asserts on Open Library's contribution (confirmed
    /// reachable/deterministic from this environment); Google Books can
    /// legitimately return nothing (shared keyless quota) and BnF is
    /// unreachable from this dev environment entirely (see
    /// `online_search::search_bnf`'s doc comment) — the point of this test
    /// is confirming the 3-way parallel join itself works and a slow/failing
    /// source doesn't drop the others, not that every source answers.
    #[test]
    fn search_book_online_returns_open_librarys_result_even_if_other_sources_fail() {
        let results = search_book_online("978-0-14-032872-1".to_string());
        assert!(
            results.iter().any(|r| r.source == "Open Library" && r.title == "Fantastic Mr. Fox"),
            "expected Open Library's result among {results:?}"
        );
    }
}
