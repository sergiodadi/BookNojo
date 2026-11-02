use std::path::Path;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![load_collection])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
