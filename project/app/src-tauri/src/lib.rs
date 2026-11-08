use std::path::Path;

use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Only collections produced by this exact Book Collector release are known
/// to match the schema this project's SQL targets.
const SUPPORTED_BKCP_VERSION: &str = "20.3.1";

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
    env!("CARGO_PKG_VERSION").to_string()
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
#[tauri::command]
fn commit_add_book(path: String, input: book_collector_export::NewBookInput) -> Result<String, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    refuse_if_real_collection(&db_path)?;
    if book_collector_export::is_book_collector_running().map_err(|e| format!("{e:#}"))? {
        return Err("Book Collector est actuellement lancé — fermez l'application avant d'enregistrer.".to_string());
    }
    let backup_dir = db_path
        .parent()
        .map(|p| p.join("backups"))
        .unwrap_or_else(|| std::path::PathBuf::from("backups"));
    book_collector_export::commit_add_book(&db_path, &backup_dir, input)
        .map(|backup| format!("Livre ajouté. Sauvegarde : {}", backup.display()))
        .map_err(|e| format!("{e:#}"))
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

/// "File > Import a collection..." — opens the native macOS file picker. A
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
/// picker itself is asynchronous/callback-based.
fn open_import_dialog(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.dialog()
        .file()
        .set_title("Importer une collection")
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

fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "BookNojo")
        .about(None)
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .text("import_collection", "Import a collection…")
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
    let help_menu = SubmenuBuilder::new(app, "BookNojo Help")
        .text("about_booknojo", "About BookNojo")
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
            app_version
        ])
        .setup(|app| {
            let menu = build_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "import_collection" => open_import_dialog(app),
            "about_booknojo" => {
                app.dialog()
                    .message(format!("BookNojo\nVersion {}", env!("CARGO_PKG_VERSION")))
                    .title("About BookNojo")
                    .show(|_| {});
            }
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
}
