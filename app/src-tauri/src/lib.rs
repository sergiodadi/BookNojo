use std::path::Path;

use tauri::menu::{MenuBuilder, SubmenuBuilder};
use tauri::Emitter;
use tauri_plugin_dialog::DialogExt;

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

/// "Files > Import a file..." — opens the native macOS file picker scoped to
/// folders (a `.bkcp`/`.col` is a directory; macOS treats it as a single
/// selectable package rather than something to navigate into, since Book
/// Collector registers that extension as a package type). The chosen path
/// is sent to the frontend as an `import-collection` event rather than
/// returned directly, since the picker itself is asynchronous/callback-based.
fn open_import_dialog(app: &tauri::AppHandle) {
    let app_handle = app.clone();
    app.dialog()
        .file()
        .set_title("Importer une collection")
        .pick_folder(move |folder| {
            let Some(path) = folder else { return };
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
        .text("import_file", "Import a file…")
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
            "import_file" => open_import_dialog(app),
            "about_nbc" => {
                app.dialog()
                    .message(format!(
                        "NBC — successeur de Book Collector (nom provisoire)\nVersion {}",
                        env!("CARGO_PKG_VERSION")
                    ))
                    .title("About NBC")
                    .show(|_| {});
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
