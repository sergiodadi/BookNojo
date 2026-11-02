use std::path::Path;

use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Emitter;
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
#[tauri::command]
fn load_collection(path: String) -> Result<Vec<book_collector_export::PageBook>, String> {
    let db_path = book_collector_export::resolve_db_path(Path::new(&path)).map_err(|e| format!("{e:#}"))?;
    book_collector_export::load_page_books(&db_path).map_err(|e| format!("{e:#}"))
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
/// targets that one release's schema and other versions may
/// use a different schema. The chosen path is sent to the frontend as an
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
    let app_menu = SubmenuBuilder::new(app, "NBC")
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
    // behavior) — "NBC Help" sidesteps it while staying unambiguous.
    let help_menu = SubmenuBuilder::new(app, "NBC Help")
        .text("about_nbc", "About NBC")
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
        .invoke_handler(tauri::generate_handler![load_collection])
        .setup(|app| {
            let menu = build_menu(app.handle())?;
            app.set_menu(menu)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "import_collection" => open_import_dialog(app),
            "about_nbc" => {
                app.dialog()
                    .message(format!("NBC (nom provisoire)\nVersion {}", env!("CARGO_PKG_VERSION")))
                    .title("About NBC")
                    .show(|_| {});
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
